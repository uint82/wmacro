//! the Run Command / Shell Command modal: execute a program or shell line and capture output.

use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wmacro_core_types::{MacroCommand, Value};

use super::modal_trait::ModalWidget;
use super::types::ModalOutcome;
use super::variable::auto_focus;

/// dry-run helper for the modal Test button: same executor as playback, no
/// abort flag, captures into a scratch map so results can be displayed.
#[allow(clippy::too_many_arguments)]
fn execute_run_command_test(
    command: &str,
    args: &str,
    use_shell: bool,
    working_dir: &str,
    timeout_ms: Option<u64>,
    variables: &mut HashMap<String, Value>,
    wait: bool,
    stdin_text: Option<&str>,
    env_vars: &[(String, String)],
) {
    crate::macro_engine::player::effects::execute_run_command(
        command,
        args,
        use_shell,
        working_dir,
        crate::macro_engine::player::effects::RunCommandCaptures {
            stdout: Some("__test_out"),
            stderr: Some("__test_err"),
            exit_code: Some("__test_code"),
            pid: None,
        },
        timeout_ms,
        variables,
        wait,
        stdin_text,
        env_vars,
        None,
    );
}

/// renders the Test result box content from the scratch variable map.
fn format_test_summary(
    vars: &HashMap<String, Value>,
    elapsed: std::time::Duration,
) -> Result<String, String> {
    let code = vars.get("__test_code").map(Value::as_i64).unwrap_or(-1);
    let out = vars
        .get("__test_out")
        .map(Value::as_text)
        .unwrap_or_default();
    let err = vars
        .get("__test_err")
        .map(Value::as_text)
        .unwrap_or_default();

    let truncate = |s: &str| -> String {
        if s.len() > 300 {
            format!("{}…", &s[..300])
        } else if s.is_empty() {
            "(empty)".to_string()
        } else {
            s.to_string()
        }
    };

    let mut lines = vec![format!(
        "exit {}  in {:.2}s",
        code,
        elapsed.as_secs_f32()
    )];
    lines.push(format!("stdout: {}", truncate(&out)));
    lines.push(format!("stderr: {}", truncate(&err)));

    if code == -1 && out.is_empty() && !err.contains("aborted") && err.starts_with("timed out") {
        return Err(err);
    }
    Ok(lines.join("\n"))
}

pub struct RunCommandModal {
    pub command: String,
    pub args: String,
    pub use_shell: bool,
    pub working_dir: String,
    pub store_stdout: String,
    pub store_stderr: String,
    pub store_exit_code: String,
    pub store_pid: String,
    pub timeout_text: String,
    pub wait: bool,
    pub stdin_text: String,
    pub env_text: String,
    /// result of the modal's Test run; `None` until Test is clicked. Shared
    /// with a worker thread like the other modals' async results.
    pub test_result: Arc<Mutex<Option<Result<String, String>>>>,
    pub edit_idx: Option<usize>,
}

impl ModalWidget for RunCommandModal {
    fn title(&self) -> String {
        format!("{} Run Command", egui_phosphor::regular::TERMINAL)
    }

    fn edit_idx(&self) -> Option<usize> {
        self.edit_idx
    }

    fn autofocus_ids(&self) -> &[&'static str] {
        &["run_cmd_command", "run_cmd_args", "run_cmd_stdout"]
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        _state: &SharedState,
        palette: &ThemePalette,
    ) -> ModalOutcome {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Command:")
                .color(palette.text_primary)
                .size(12.0)
                .strong(),
        )
        .on_hover_text("Executable name or full shell line. Supports $variable interpolation.");
        ui.add_space(2.0);
        let command_resp = ui.add(
            egui::TextEdit::singleline(&mut self.command)
                .id(egui::Id::new("run_cmd_command"))
                .desired_width(f32::INFINITY)
                .hint_text("e.g., curl, python3, echo, hyprctl"),
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new("Tip: use $var inside command/args, e.g., curl $url")
                .color(palette.text_muted)
                .size(10.0),
        );

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Arguments:")
                .color(palette.text_primary)
                .size(12.0)
                .strong(),
        )
        .on_hover_text("Arguments passed to the command. In shell mode this is appended as 'command args' and run via sh -c.");
        ui.add_space(2.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.args)
                .id(egui::Id::new("run_cmd_args"))
                .desired_width(f32::INFINITY)
                .hint_text("-s https://example.com  or  \"hello $name\" | grep hi"),
        );

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.use_shell, "");
            ui.label(
                egui::RichText::new("Run via shell (sh -c)")
                    .color(palette.text_primary)
                    .size(12.0),
            )
            .on_hover_text("When enabled, runs as: sh -c \"<command> <args>\". Enables pipes, redirects, &&, ||, $(). When disabled, runs command directly.");
        });
        if self.use_shell {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("Shell mode: pipes (|), redirects (>), &&, || allowed.")
                    .color(palette.text_muted)
                    .size(10.0),
            );
        }

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Working directory (optional):")
                .color(palette.text_primary)
                .size(11.0),
        );
        ui.add_space(2.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.working_dir)
                .id(egui::Id::new("run_cmd_workdir"))
                .desired_width(f32::INFINITY)
                .hint_text("/tmp, ~/projects  or  $project_dir"),
        );

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.wait, "");
            ui.label(
                egui::RichText::new("Wait for completion")
                    .color(palette.text_primary)
                    .size(12.0),
            )
            .on_hover_text("When checked (default), the macro blocks until the command exits and can capture output/exit code (like AutoHotkey RunWait). Unchecked = fire-and-forget launch (like AHK Run); output capture is ignored.");
        });
        if !self.wait {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("Fire-and-forget: stdout/stderr/exit captures are skipped.")
                    .color(palette.text_muted)
                    .size(10.0),
            );
        }

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Environment variables (optional):")
                .color(palette.text_primary)
                .size(11.0),
        );
        ui.add_space(2.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.env_text)
                .id(egui::Id::new("run_cmd_env"))
                .desired_width(f32::INFINITY)
                .hint_text("API_KEY=$my_key DEBUG=1 MSG=\"hello world\""),
        );

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Stdin input (optional):")
                .color(palette.text_primary)
                .size(11.0),
        );
        ui.add_space(2.0);
        ui.add(
            egui::TextEdit::multiline(&mut self.stdin_text)
                .id(egui::Id::new("run_cmd_stdin"))
                .desired_width(f32::INFINITY)
                .desired_rows(2)
                .hint_text("$clipboard_text  or any text piped to the command's stdin"),
        );

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Capture output into variables (optional):")
                .color(palette.text_primary)
                .size(11.0)
                .strong(),
        );
        ui.add_space(6.0);

        egui::Grid::new("run_cmd_capture_grid")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Stdout -> $").color(palette.text_muted).size(11.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.store_stdout)
                        .id(egui::Id::new("run_cmd_stdout"))
                        .hint_text("my_stdout")
                        .desired_width(140.0),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Stderr -> $").color(palette.text_muted).size(11.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.store_stderr)
                        .id(egui::Id::new("run_cmd_stderr"))
                        .hint_text("my_stderr")
                        .desired_width(140.0),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Exit code -> $").color(palette.text_muted).size(11.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.store_exit_code)
                        .id(egui::Id::new("run_cmd_exit"))
                        .hint_text("exit_code")
                        .desired_width(140.0),
                );
                ui.end_row();

                ui.label(egui::RichText::new("PID -> $").color(palette.text_muted).size(11.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.store_pid)
                        .id(egui::Id::new("run_cmd_pid"))
                        .hint_text("launched_pid")
                        .desired_width(140.0),
                );
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Timeout (ms):").color(palette.text_muted).size(11.0));
            ui.add(
                egui::TextEdit::singleline(&mut self.timeout_text)
                    .id(egui::Id::new("run_cmd_timeout"))
                    .hint_text("5000")
                    .desired_width(80.0),
            );
            ui.label(egui::RichText::new("empty = no timeout").color(palette.text_muted).size(10.0));
        });
        if !self.timeout_text.trim().is_empty() && parse_timeout(&self.timeout_text).is_none() {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("Timeout must be a number in milliseconds")
                    .color(palette.accent_danger)
                    .size(11.0),
            );
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        auto_focus(ui, "run_cmd_command", &command_resp);

        let can_commit = !self.command.trim().is_empty()
            && (self.timeout_text.trim().is_empty() || parse_timeout(&self.timeout_text).is_some());

        let make_cmd = || MacroCommand::RunCommand {
            command: self.command.trim().to_string(),
            args: self.args.trim().to_string(),
            use_shell: self.use_shell,
            working_dir: self.working_dir.trim().to_string(),
            store_stdout: empty_to_none(&self.store_stdout),
            store_stderr: empty_to_none(&self.store_stderr),
            store_exit_code: empty_to_none(&self.store_exit_code),
            store_pid: empty_to_none(&self.store_pid),
            timeout_ms: parse_timeout(&self.timeout_text),
            wait: self.wait,
            stdin_text: empty_to_none_multiline(&self.stdin_text),
            env_vars: crate::macro_engine::script::parse_env_pairs(&self.env_text),
        };

        // Test run: executes the current fields through the real playback code
        // path with a capped timeout, so users can verify a command before
        // saving. Variables are not resolvable in the editor and stay literal.
        ui.add_space(4.0);
        if ui
            .add_enabled(
                can_commit,
                egui::Button::new(egui::RichText::new(format!("{}  Test", egui_phosphor::regular::PLAY)).size(12.0)),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Runs this command now and shows exit code plus captured output.")
            .clicked()
        {
            let cmd = make_cmd();
            let result_slot = Arc::clone(&self.test_result);
            let ctx_handle = ui.ctx().clone();
            *result_slot.lock().unwrap() = None;
            std::thread::spawn(move || {
                let mut vars = HashMap::new();
                let started = std::time::Instant::now();
                if let MacroCommand::RunCommand {
                    command,
                    args,
                    use_shell,
                    working_dir,
                    timeout_ms,
                    wait,
                    stdin_text,
                    env_vars,
                    ..
                } = &cmd
                {
                    execute_run_command_test(
                        command,
                        args,
                        *use_shell,
                        working_dir,
                        *timeout_ms,
                        &mut vars,
                        *wait,
                        stdin_text.as_deref(),
                        env_vars,
                    );
                }
                let elapsed = started.elapsed();
                let summary = format_test_summary(&vars, elapsed);
                if let Ok(mut slot) = result_slot.lock() {
                    *slot = Some(summary);
                }
                ctx_handle.request_repaint_after(std::time::Duration::from_millis(50));
            });
        }

        // show test result when available
        if let Ok(slot) = self.test_result.lock()
            && let Some(result) = slot.as_ref()
        {
            match result {
                Ok(text) => {
                    ui.add_space(6.0);
                    egui::Frame::NONE
                        .fill(palette.bg_element)
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::same(8))
                        .show(ui, |ui| {
                            ui.set_max_width(ui.available_width());
                            ui.label(
                                egui::RichText::new(text)
                                    .color(palette.accent_success)
                                    .size(11.0)
                                    .monospace(),
                            );
                        });
                }
                Err(err) => {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(format!("Test failed: {}", err))
                            .color(palette.accent_danger)
                            .size(11.0)
                            .monospace(),
                    );
                }
            }
        }

        // handle Enter submission when command field focused
        let command_focused = ui.ctx().memory(|m| m.focused() == Some(egui::Id::new("run_cmd_command")));
        let args_focused = ui.ctx().memory(|m| m.focused() == Some(egui::Id::new("run_cmd_args")));
        if (command_focused || args_focused) && ui.input(|i| i.key_pressed(egui::Key::Enter)) && can_commit {
            return ModalOutcome::Commit(make_cmd());
        }

        let label = if self.edit_idx.is_some() { "Save" } else { "Add" };
        let mut outcome = ModalOutcome::Open;

        super::right_aligned_row(ui, |ui| {
            if ui
                .add_enabled(
                    can_commit,
                    egui::Button::new(egui::RichText::new(label).strong())
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                outcome = ModalOutcome::Commit(make_cmd());
            }

            ui.add_space(8.0);

            if ui
                .add(
                    egui::Button::new(egui::RichText::new("Cancel"))
                        .min_size(egui::vec2(80.0, ui.spacing().interact_size.y * 1.2)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                outcome = ModalOutcome::Cancelled;
            }
        });

        outcome
    }
}

fn empty_to_none(s: &str) -> Option<String> {
    let t = s.trim().to_string();
    if t.is_empty() { None } else { Some(t) }
}

/// keeps interior newlines (stdin is multiline); only all-whitespace input is treated as empty.
fn empty_to_none_multiline(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn parse_timeout(s: &str) -> Option<u64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<u64>().ok()
}
