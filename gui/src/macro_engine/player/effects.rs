//! effect commands: `TypeText`, `SetClipboard`, `GetClipboard`, `OpenFile` and `RunCommand`.

use log::{error, info};
use std::collections::HashMap;
use std::process::Stdio;
use std::time::{Duration, Instant};
use wmacro_core_types::{Operand, Value};

use crate::macro_engine::player::dispatch::execute_type_text;
use crate::macro_engine::player::models::{ClipboardBackend, FlowControl};
use crate::macro_engine::player::variables::{interpolate_variables, resolve_value};

pub(super) fn execute_type_text_cmd(text: &str, variables: &HashMap<String, Value>) -> FlowControl {
    let resolved = interpolate_variables(text, variables);
    if let Err(e) = execute_type_text(&resolved) {
        error!("TypeText error: {}", e);
    }
    FlowControl::Continue
}

/// sets the clipboard to the operand's text form; no-ops without a backend.
pub(super) fn execute_set_clipboard(
    text: &Operand,
    variables: &HashMap<String, Value>,
    clipboard: Option<&dyn ClipboardBackend>,
) {
    let Some(clipboard) = clipboard else {
        log::warn!("SetClipboard: no clipboard backend available, skipping");
        return;
    };
    let value = resolve_value(text, variables);
    clipboard.set_text(&value.as_text());
    log::debug!("SetClipboard: clipboard set to '{}'", value.as_text());
}

/// reads the current clipboard text into a variable; empty or unavailable reads as `""`.
pub(super) fn execute_get_clipboard(
    target: &str,
    variables: &mut HashMap<String, Value>,
    clipboard: Option<&dyn ClipboardBackend>,
) {
    let Some(clipboard) = clipboard else {
        log::warn!("GetClipboard: no clipboard backend available, skipping");
        return;
    };
    let text = clipboard.get_text().unwrap_or_default();
    variables.insert(target.to_string(), Value::Text(text.clone()));
    log::debug!("GetClipboard: {} = '{}'", target, text);
}

pub(super) fn execute_open_file(path: &str, args: &str, run_as_admin: bool) {
    let parsed_args = parse_args(args);

    let resolved_executable = which::which(path);

    let result = if let Ok(exec_path) = resolved_executable {
        if run_as_admin {
            let mut cmd = std::process::Command::new("pkexec");
            cmd.arg(exec_path);
            cmd.args(&parsed_args);
            cmd.spawn()
        } else {
            let mut cmd = std::process::Command::new(exec_path);
            cmd.args(&parsed_args);
            cmd.spawn()
        }
    } else {
        let path_buf = std::path::Path::new(path);

        if !path_buf.exists() {
            error!(
                "OpenFile: command not found in PATH and path does not exist: {}",
                path
            );
            return;
        }

        std::process::Command::new("xdg-open").arg(path).spawn()
    };

    match result {
        Ok(_) => {
            info!("OpenFile: launched '{}'", path);
        }
        Err(e) => {
            error!("OpenFile: failed to launch '{}': {}", path, e);
        }
    }
}

/// variable-name captures for a `RunCommand`; `None` fields are not stored.
pub(crate) struct RunCommandCaptures<'a> {
    pub stdout: Option<&'a str>,
    pub stderr: Option<&'a str>,
    pub exit_code: Option<&'a str>,
    pub pid: Option<&'a str>,
}

/// executes a `RunCommand`. Visible crate-wide so the editor's modal Test
/// button can dry-run a command with the same code path as playback.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_run_command(
    command: &str,
    args: &str,
    use_shell: bool,
    working_dir: &str,
    captures: RunCommandCaptures<'_>,
    timeout_ms: Option<u64>,
    variables: &mut HashMap<String, Value>,
    wait: bool,
    stdin_text: Option<&str>,
    env_vars: &[(String, String)],
    abort: Option<&std::sync::atomic::AtomicBool>,
) {
    let RunCommandCaptures {
        stdout: store_stdout,
        stderr: store_stderr,
        exit_code: store_exit_code,
        pid: store_pid,
    } = captures;
    let resolved_command = interpolate_variables(command, variables);
    let resolved_args = interpolate_variables(args, variables);
    let resolved_workdir = interpolate_variables(working_dir, variables);

    if resolved_command.trim().is_empty() {
        error!("RunCommand: empty command, skipping");
        if let Some(var) = store_exit_code {
            variables.insert(var.to_string(), Value::Number(-1));
        }
        return;
    }

    // fire-and-forget launch (AutoHotkey `Run`): no pipes, no wait; captures are not meaningful.
    if !wait {
        if store_stdout.is_some() || store_stderr.is_some() || store_exit_code.is_some() {
            log::warn!("RunCommand: wait=false ignores output captures (process keeps running)");
        }
        let mut cmd = build_command(&resolved_command, &resolved_args, use_shell);
        apply_workdir_and_env(&mut cmd, &resolved_workdir, env_vars, variables);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match cmd.spawn() {
            Ok(child) => {
                if let Some(var) = store_pid {
                    variables.insert(var.to_string(), Value::Number(child.id() as i64));
                }
                info!(
                    "RunCommand: launched detached '{}' (pid {})",
                    resolved_command,
                    child.id()
                );
            }
            Err(e) => {
                error!("RunCommand: failed to launch '{}': {}", resolved_command, e);
                if let Some(var) = store_pid {
                    variables.insert(var.to_string(), Value::Number(-1));
                }
            }
        }
        return;
    }

    let mut cmd = build_command(&resolved_command, &resolved_args, use_shell);

    apply_workdir_and_env(&mut cmd, &resolved_workdir, env_vars, variables);

    let has_stdin = stdin_text.is_some();
    if has_stdin {
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    info!(
        "RunCommand: executing '{}' args='{}' shell={} workdir='{}' stdin={} env={}",
        resolved_command,
        resolved_args,
        use_shell,
        resolved_workdir,
        has_stdin,
        env_vars.len()
    );

    let stdin_data: Vec<u8> = stdin_text
        .map(|t| interpolate_variables(t, variables).into_bytes())
        .unwrap_or_default();

    // like Python's subprocess.communicate(): pipes are drained on dedicated
    // threads so a chatty child can never deadlock on a full pipe buffer while
    // we poll for exit, timeout or user abort. The poll loop keeps F10 abort
    // responsive even mid-command, matching AHK's hotkeys-during-RunWait model.
    let output_result = spawn_and_collect(
        cmd,
        has_stdin.then_some(stdin_data.as_slice()),
        timeout_ms.map(Duration::from_millis),
        abort,
        store_pid,
        variables,
    );

    match output_result {
        Ok(output) => {
            // industry standard: bash `$(...)` strips the trailing newline so
            // captured values are immediately reusable as arguments/variables.
            let stdout_raw = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr_raw = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout = stdout_raw.trim_end_matches(['\n', '\r']).to_string();
            let stderr = stderr_raw.trim_end_matches(['\n', '\r']).to_string();
            let code = exit_code_of(output.status);

            if let Some(var) = store_stdout {
                variables.insert(var.to_string(), Value::Text(stdout.clone()));
            }
            if let Some(var) = store_stderr {
                variables.insert(var.to_string(), Value::Text(stderr.clone()));
            }
            if let Some(var) = store_exit_code {
                variables.insert(var.to_string(), Value::Number(code));
            }

            info!(
                "RunCommand: exit={} stdout_len={} stderr_len={}",
                code,
                stdout.len(),
                stderr.len()
            );
            if !output.status.success() && !stderr.is_empty() {
                log::warn!("RunCommand stderr: {}", stderr.trim_end());
            }
        }
        Err(e) => {
            error!("RunCommand failed: {}", e);
            if let Some(var) = store_stderr {
                variables.insert(var.to_string(), Value::Text(e.clone()));
            }
            if let Some(var) = store_exit_code {
                variables.insert(var.to_string(), Value::Number(-1));
            }
            if let Some(var) = store_stdout
                && !variables.contains_key(var)
            {
                variables.insert(var.to_string(), Value::Text(String::new()));
            }
        }
    }
}

/// maps a process status to a conventional integer code; signal deaths follow
/// the POSIX `128+n` convention (bash `$?`), which is what users compare against.
fn exit_code_of(status: std::process::ExitStatus) -> i64 {
    match status.code() {
        Some(c) => c as i64,
        None => {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                if let Some(sig) = status.signal() {
                    return 128 + sig as i64;
                }
                if let Some(sig) = status.stopped_signal() {
                    return 128 + sig as i64;
                }
            }
            -1
        }
    }
}

fn apply_workdir_and_env(
    cmd: &mut std::process::Command,
    resolved_workdir: &str,
    env_vars: &[(String, String)],
    variables: &HashMap<String, Value>,
) {
    // expand a leading `~` like every shell does; Command does not do it for us.
    let dir = expand_tilde(resolved_workdir.trim());
    if !dir.is_empty() {
        cmd.current_dir(dir);
    }
    for (k, v) in env_vars {
        cmd.env(k, interpolate_variables(v, variables));
    }
}

/// builds the base command: a single shell line via `sh -c`, or an executable
/// with quote-aware argument splitting (mirrors AHK Run's target handling).
fn build_command(resolved_command: &str, resolved_args: &str, use_shell: bool) -> std::process::Command {
    if use_shell {
        let shell_line = if resolved_args.trim().is_empty() {
            resolved_command.to_string()
        } else {
            format!("{} {}", resolved_command, resolved_args)
        };
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(shell_line);
        c
    } else {
        let mut c = std::process::Command::new(resolved_command);
        c.args(parse_args(resolved_args));
        c
    }
}

/// expands a leading `~` or `~/...` to $HOME; other paths pass through untouched.
fn expand_tilde(path: &str) -> String {
    if path == "~" {
        return std::env::var("HOME").unwrap_or_else(|_| path.to_string());
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{}/{}", home.trim_end_matches('/'), rest);
    }
    path.to_string()
}

/// spawns the command, drains both pipes on threads (never deadlocks), writes
/// stdin when provided, and enforces an optional timeout. `abort` is the
/// playback kill flag: when raised mid-command, the child is terminated and
/// `Err("aborted")` is returned so F10 stays responsive during long commands.
fn spawn_and_collect(
    mut cmd: std::process::Command,
    stdin_data: Option<&[u8]>,
    timeout: Option<Duration>,
    abort: Option<&std::sync::atomic::AtomicBool>,
    store_pid: Option<&str>,
    variables: &mut HashMap<String, Value>,
) -> Result<std::process::Output, String> {
    use std::io::{Read, Write};

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    if let Some(var) = store_pid {
        variables.insert(var.to_string(), Value::Number(child.id() as i64));
    }

    // feed stdin from a thread so a child that never reads it cannot block us either.
    if let Some(data) = stdin_data
        && let Some(mut pipe) = child.stdin.take()
    {
        let data = data.to_vec();
        std::thread::spawn(move || {
            let _ = pipe.write_all(&data);
        });
    }

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let drain_out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = stdout_pipe {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let drain_err = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = stderr_pipe {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });

    let start = Instant::now();
    let deadline = timeout.map(|t| start + t);
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None => {
                let now = Instant::now();
                let timed_out = deadline.is_some_and(|d| now >= d);
                let aborted = abort.is_some_and(|a| a.load(std::sync::atomic::Ordering::Relaxed));

                if timed_out || aborted {
                    terminate_gracefully(&mut child);
                    let _ = drain_out.join();
                    let _ = drain_err.join();
                    return Err(if aborted {
                        "aborted by user".to_string()
                    } else {
                        format!("timed out after {}ms", (now - start).as_millis())
                    });
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };

    let stdout = drain_out.join().map_err(|_| "stdout reader panicked".to_string())?;
    let stderr = drain_err.join().map_err(|_| "stderr reader panicked".to_string())?;

    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

/// asks the child to exit with SIGTERM first, escalating to SIGKILL after a
/// short grace period - the Robot Framework `on_timeout=terminate` model.
/// SIGTERM gives well-behaved daemons/scripts a chance to clean up temp state.
fn terminate_gracefully(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
        let grace_start = Instant::now();
        while Instant::now() - grace_start < Duration::from_millis(500) {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(25)),
                Err(_) => break,
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn parse_args(raw: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '"';

    for ch in raw.chars() {
        match ch {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = ch;
            }
            c if in_quotes && c == quote_char => {
                in_quotes = false;
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    struct FakeClipboard {
        contents: std::sync::Mutex<String>,
    }

    impl FakeClipboard {
        fn new(contents: &str) -> Self {
            Self {
                contents: std::sync::Mutex::new(contents.to_string()),
            }
        }
    }

    impl ClipboardBackend for FakeClipboard {
        fn get_text(&self) -> Option<String> {
            Some(self.contents.lock().unwrap().clone())
        }
        fn set_text(&self, text: &str) {
            *self.contents.lock().unwrap() = text.to_string();
        }
    }

    #[test]
    fn set_clipboard_writes_operand_text() {
        let clipboard = FakeClipboard::new("");
        let mut vars = HashMap::new();
        vars.insert("msg".to_string(), text("hello"));
        execute_set_clipboard(
            &Operand::Literal(Value::Text("plain".into())),
            &vars,
            Some(&clipboard),
        );
        assert_eq!(*clipboard.contents.lock().unwrap(), "plain");
        execute_set_clipboard(&Operand::Var("msg".into()), &vars, Some(&clipboard));
        assert_eq!(*clipboard.contents.lock().unwrap(), "hello");
        execute_set_clipboard(
            &Operand::Literal(Value::Number(42)),
            &vars,
            Some(&clipboard),
        );
        assert_eq!(*clipboard.contents.lock().unwrap(), "42");
    }

    #[test]
    fn get_clipboard_stores_text_variable() {
        let clipboard = FakeClipboard::new("copied text");
        let mut vars = HashMap::new();
        execute_get_clipboard("clip", &mut vars, Some(&clipboard));
        assert_eq!(vars.get("clip"), Some(&text("copied text")));
    }

    #[test]
    fn get_clipboard_empty_reads_empty_string() {
        let clipboard = FakeClipboard::new("");
        let mut vars = HashMap::new();
        execute_get_clipboard("clip", &mut vars, Some(&clipboard));
        assert_eq!(vars.get("clip"), Some(&text("")));
    }

    #[test]
    fn clipboard_commands_noop_without_backend() {
        let mut vars = HashMap::new();
        execute_set_clipboard(&Operand::Literal(Value::Text("x".into())), &vars, None);
        execute_get_clipboard("clip", &mut vars, None);
        assert!(vars.is_empty());
    }

    fn run(args: RunArgs) -> HashMap<String, Value> {
        let mut vars = HashMap::new();
        execute_run_command(
            &args.command,
            args.args.as_deref().unwrap_or(""),
            args.shell,
            args.workdir.as_deref().unwrap_or(""),
            RunCommandCaptures {
                stdout: args.out.as_deref(),
                stderr: args.err.as_deref(),
                exit_code: args.code.as_deref(),
                pid: args.pid.as_deref(),
            },
            args.timeout_ms,
            &mut vars,
            true,
            None,
            &[],
            None,
        );
        vars
    }

    struct RunArgs {
        command: String,
        args: Option<String>,
        shell: bool,
        workdir: Option<String>,
        out: Option<String>,
        err: Option<String>,
        code: Option<String>,
        timeout_ms: Option<u64>,
        pid: Option<String>,
    }

    impl Default for RunArgs {
        fn default() -> Self {
            Self {
                command: String::new(),
                args: None,
                shell: false,
                workdir: None,
                out: Some("out".into()),
                err: Some("err".into()),
                code: Some("code".into()),
                timeout_ms: None,
                pid: None,
            }
        }
    }

    #[test]
    fn run_captures_stdout_without_shell() {
        let vars = run(RunArgs {
            command: "echo".into(),
            args: Some("hello world".into()),
            ..Default::default()
        });
        assert_eq!(vars.get("out"), Some(&text("hello world")));
        assert_eq!(vars.get("code"), Some(&Value::Number(0)));
    }

    #[test]
    fn run_shell_pipe_and_trimmed_newline() {
        let vars = run(RunArgs {
            command: "echo hi there | tr a-z A-Z".into(),
            shell: true,
            ..Default::default()
        });
        assert_eq!(vars.get("out"), Some(&text("HI THERE")));
    }

    #[test]
    fn run_timeout_kills_and_reports_minus_one() {
        let vars = run(RunArgs {
            command: "sleep".into(),
            args: Some("2".into()),
            timeout_ms: Some(300),
            ..Default::default()
        });
        assert_eq!(vars.get("code"), Some(&Value::Number(-1)));
    }

    #[test]
    fn run_big_output_that_exits_is_not_falsely_timed_out() {
        // >64KB of output (several pipe buffers) from a command that exits
        // immediately. Without concurrent draining the child blocks writing
        // forever, never exits, and gets killed by the timeout despite finishing.
        let start = std::time::Instant::now();
        let vars = run(RunArgs {
            command: "seq 1 50000".into(),
            shell: true,
            timeout_ms: Some(10_000),
            ..Default::default()
        });
        assert_eq!(vars.get("code"), Some(&Value::Number(0)));
        let out = vars.get("out").unwrap().as_text();
        assert!(out.lines().count() >= 50_000);
        assert!(start.elapsed() < Duration::from_millis(5_000));
    }

    #[test]
    fn run_stdin_is_piped_to_child() {
        let mut vars = HashMap::new();
        execute_run_command(
            "wc",
            "-c",
            false,
            "",
            RunCommandCaptures {
                stdout: Some("out"),
                stderr: None,
                exit_code: Some("code"),
                pid: None,
            },
            None,
            &mut vars,
            true,
            Some("five!"),
            &[],
            None,
        );
        assert_eq!(vars.get("out"), Some(&text("5")));
    }

    #[test]
    fn run_env_vars_reach_child() {
        let mut vars = HashMap::new();
        execute_run_command(
            "printenv",
            "WMACRO_TEST",
            false,
            "",
            RunCommandCaptures {
                stdout: Some("out"),
                stderr: None,
                exit_code: None,
                pid: None,
            },
            None,
            &mut vars,
            true,
            None,
            &[("WMACRO_TEST".to_string(), "works".to_string())],
            None,
        );
        assert_eq!(vars.get("out"), Some(&text("works")));
    }

    #[test]
    fn run_pid_is_captured() {
        let mut vars = HashMap::new();
        execute_run_command(
            "echo",
            "hi",
            false,
            "",
            RunCommandCaptures {
                stdout: None,
                stderr: None,
                exit_code: None,
                pid: Some("the_pid"),
            },
            None,
            &mut vars,
            true,
            None,
            &[],
            None,
        );
        let pid = vars.get("the_pid").unwrap().as_i64();
        assert!(pid > 0);
    }

    #[test]
    fn run_detached_launch_stores_pid_and_returns_fast() {
        let start = std::time::Instant::now();
        let mut vars = HashMap::new();
        execute_run_command(
            "sleep",
            "3",
            false,
            "",
            RunCommandCaptures {
                stdout: None,
                stderr: None,
                exit_code: None,
                pid: Some("bg_pid"),
            },
            None,
            &mut vars,
            false,
            None,
            &[],
            None,
        );
        assert!(start.elapsed() < Duration::from_millis(500));
        assert!(vars.get("bg_pid").unwrap().as_i64() > 0);
    }

    #[test]
    fn run_abort_terminates_child_promptly() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        let mut vars = HashMap::new();

        let kill = Arc::new(AtomicBool::new(false));
        let killer_kill = Arc::clone(&kill);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            killer_kill.store(true, std::sync::atomic::Ordering::Relaxed);
        });

        let start = std::time::Instant::now();
        execute_run_command(
            "sleep",
            "30",
            false,
            "",
            RunCommandCaptures {
                stdout: None,
                stderr: Some("err"),
                exit_code: Some("code"),
                pid: None,
            },
            None,
            &mut vars,
            true,
            None,
            &[],
            Some(kill.as_ref()),
        );
        // must return well before the 30s sleep finishes
        assert!(start.elapsed() < Duration::from_millis(2_000));
        assert_eq!(vars.get("code"), Some(&Value::Number(-1)));
        assert_eq!(vars.get("err"), Some(&text("aborted by user")));
    }

    #[test]
    fn expand_tilde_uses_home() {
        let home = std::env::var("HOME").unwrap_or_default();
        if home.is_empty() {
            return;
        }
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/sub"), format!("{}/sub", home.trim_end_matches('/')));
        assert_eq!(expand_tilde("/tmp"), "/tmp");
    }

    #[test]
    fn parse_env_pairs_handles_quoted_values() {
        let pairs = crate::macro_engine::script::parse_env_pairs("A=1 MSG=\"hi there\" B=2 C='x y'");
        assert_eq!(
            pairs,
            vec![
                ("A".to_string(), "1".to_string()),
                ("MSG".to_string(), "hi there".to_string()),
                ("B".to_string(), "2".to_string()),
                ("C".to_string(), "x y".to_string()),
            ]
        );
    }
}
