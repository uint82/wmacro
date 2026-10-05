//! serializes a macro to `.wmr` script text.

use std::fmt::Write;
use wmacro_core_types::{
    Coord, Macro, MacroButton, MacroCommand, MacroEvent, MousePosition, Operand, Value,
};

// TODO: round-trip test: serialize every command variant and parse it back.
// TODO: escape newlines and control characters in quoted values so text cannot break the line-based format.

/// serializes a macro to `.wmr` script text.
pub fn serialize(m: &Macro) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "# wmacro script");
    let _ = writeln!(out, "version {}", wmacro_core_types::CURRENT_FORMAT_VERSION);
    let _ = writeln!(out, "name \"{}\"\n", m.name);

    for cmd in &m.commands {
        serialize_command(&mut out, cmd);
    }
    out
}

/// escapes a quoted value: `"` doubles (AHK-style, the format's original
/// hatch) and control characters use backslash sequences so values never
/// break the line-based format. `\\` is escaped first-order so `\n` from an
/// older file cannot be confused with a real newline written by this version.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\"\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

/// formats an operand: a `$` variable, a literal number, or quoted text with doubled quotes.
pub fn format_operand(op: &Operand) -> String {
    match op {
        Operand::Var(name) => format!("${}", name),
        Operand::Literal(Value::Number(n)) => n.to_string(),
        Operand::Literal(Value::Float(f)) => f.to_string(),
        Operand::Literal(Value::Text(s)) => format!("\"{}\"", esc(s)),
    }
}

fn format_coord(c: &Coord) -> String {
    match c {
        Coord::Const(v) => v.to_string(),
        Coord::Var(name) => format!("${}", name),
    }
}

fn serialize_command(out: &mut String, cmd: &MacroCommand) {
    match cmd {
        MacroCommand::Action(ev) => serialize_event(out, ev),
        MacroCommand::IfPixelColor {
            x,
            y,
            r,
            g,
            b,
            tolerance,
        } => {
            let _ = writeln!(
                out,
                "IfPixelColor x={} y={} r={} g={} b={} tol={}",
                format_coord(x),
                format_coord(y),
                r,
                g,
                b,
                tolerance
            );
        }
        MacroCommand::IfImageFound {
            target_image_path,
            similarity_threshold,
            move_cursor_if_found,
            trigger_if_not_found,
            region,
            store_x,
            store_y,
        } => {
            let _ = write!(
                out,
                "IfImageFound target=\"{}\" tol={} move={} not_found={}",
                esc(target_image_path),
                similarity_threshold,
                move_cursor_if_found,
                trigger_if_not_found
            );
            write_region(out, region);
            write_store_var(out, "store_x", store_x);
            write_store_var(out, "store_y", store_y);
            let _ = writeln!(out);
        }
        MacroCommand::IfColorFound {
            region,
            r,
            g,
            b,
            tolerance,
            min_width,
            min_height,
            move_cursor_if_found,
            store_x,
            store_y,
            store_w,
            store_h,
        } => {
            let _ = write!(
                out,
                "IfColorFound color=#{:02X}{:02X}{:02X} tol={} min_width={} min_height={}",
                r, g, b, tolerance, min_width, min_height
            );
            if *move_cursor_if_found {
                let _ = write!(out, " move=true");
            }
            write_region(out, region);
            write_store_var(out, "store_x", store_x);
            write_store_var(out, "store_y", store_y);
            write_store_var(out, "store_w", store_w);
            write_store_var(out, "store_h", store_h);
            let _ = writeln!(out);
        }
        MacroCommand::Else => {
            let _ = writeln!(out, "Else");
        }
        MacroCommand::EndIf => {
            let _ = writeln!(out, "EndIf");
        }
        MacroCommand::Loop { count } => {
            let _ = writeln!(out, "Loop count={}", format_operand(count));
        }
        MacroCommand::Delay { duration_ms } => {
            let _ = writeln!(out, "Delay duration_ms={}", format_operand(duration_ms));
        }
        MacroCommand::EndLoop => {
            let _ = writeln!(out, "EndLoop");
        }
        MacroCommand::PlayMacro(path) => {
            let _ = writeln!(out, "PlayMacro path=\"{}\"", esc(path));
        }
        MacroCommand::Label(name) => {
            let _ = writeln!(out, "Label name=\"{}\"", esc(name));
        }
        MacroCommand::Goto(target) => {
            let _ = writeln!(out, "Goto target=\"{}\"", esc(target));
        }
        MacroCommand::TypeText(text) => {
            let _ = writeln!(out, "TypeText text=\"{}\"", esc(text));
        }
        MacroCommand::OpenFile {
            path,
            args,
            run_as_admin,
        } => {
            let _ = writeln!(
                out,
                "OpenFile path=\"{}\" args=\"{}\" admin={}",
                esc(path),
                esc(args),
                run_as_admin
            );
        }
        MacroCommand::SetVariable { target, value } => {
            let _ = writeln!(
                out,
                "SetVariable target=\"{}\" value={}",
                esc(target),
                format_operand(value)
            );
        }
        MacroCommand::Calculate { target, expression } => {
            let _ = writeln!(
                out,
                "Calculate target=\"{}\" expr=\"{}\"",
                esc(target),
                esc(expression)
            );
        }
        MacroCommand::IfCompare { left, op, right } => {
            let _ = writeln!(
                out,
                "IfCompare left={} op=\"{}\" right={}",
                format_operand(left),
                op.symbol(),
                format_operand(right)
            );
        }
        MacroCommand::SetClipboard { text } => {
            let _ = writeln!(out, "SetClipboard text={}", format_operand(text));
        }
        MacroCommand::GetClipboard { target } => {
            let _ = writeln!(out, "GetClipboard target=\"{}\"", esc(target));
        }
        MacroCommand::Comment(text) => {
            let _ = writeln!(out, "Comment text=\"{}\"", esc(text));
        }
        MacroCommand::RunCommand {
            command,
            args,
            use_shell,
            working_dir,
            store_stdout,
            store_stderr,
            store_exit_code,
            store_pid,
            timeout_ms,
            wait,
            stdin_text,
            env_vars,
        } => {
            let _ = write!(
                out,
                "RunCommand command=\"{}\" args=\"{}\"",
                esc(command),
                esc(args)
            );
            if *use_shell {
                let _ = write!(out, " shell=true");
            }
            if !working_dir.is_empty() {
                let _ = write!(out, " workdir=\"{}\"", esc(working_dir));
            }
            write_store_var(out, "store_stdout", store_stdout);
            write_store_var(out, "store_stderr", store_stderr);
            write_store_var(out, "store_exit_code", store_exit_code);
            write_store_var(out, "store_pid", store_pid);
            if let Some(t) = timeout_ms {
                let _ = write!(out, " timeout_ms={}", t);
            }
            // wait=true is the default (AHK RunWait); only the fire-and-forget case is written.
            if !*wait {
                let _ = write!(out, " wait=false");
            }
            if let Some(text) = stdin_text {
                let _ = write!(out, " stdin=\"{}\"", esc(text));
            }
            if !env_vars.is_empty() {
                let joined = env_vars
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, quote_env_value(v)))
                    .collect::<Vec<_>>()
                    .join(" ");
                let _ = write!(out, " env=\"{}\"", esc(&joined));
            }
            let _ = writeln!(out);
        }
    }
}

fn write_region(out: &mut String, region: &Option<(i32, i32, i32, i32)>) {
    if let Some((l, t, w, h)) = region {
        let _ = write!(out, " left={} top={} width={} height={}", l, t, w, h);
    }
}

fn write_store_var(out: &mut String, key: &str, name: &Option<String>) {
    if let Some(name) = name {
        let _ = write!(out, " {}=\"{}\"", key, esc(name));
    }
}

/// quotes an env value when it contains spaces so `env="K=v with space"` re-parses losslessly.
fn quote_env_value(v: &str) -> String {
    if v.is_empty() || v.chars().any(|c| c.is_whitespace()) || v.contains('"') || v.contains('\'') {
        format!("\"{}\"", esc(v))
    } else {
        v.to_string()
    }
}

fn serialize_event(out: &mut String, ev: &MacroEvent) {
    match ev {
        MacroEvent::Delay(us) => {
            let _ = writeln!(out, "Delay us={}", us);
        }
        MacroEvent::MouseMove { x, y } => {
            let _ = writeln!(out, "MouseMove x={} y={}", format_coord(x), format_coord(y));
        }
        MacroEvent::Click {
            position,
            button,
            jitter,
            hold_time_ms,
        } => {
            let _ = write!(out, "Click ");
            write_position(out, position);
            let _ = write!(out, " btn={}", format_button(button));

            if *jitter > 0 {
                let _ = write!(out, " jitter={}", jitter);
            }
            let _ = writeln!(out, " hold={}", hold_time_ms);
        }
        MacroEvent::MouseDown {
            position,
            button,
            jitter,
        } => {
            let _ = write!(out, "MouseDown ");
            write_position(out, position);
            let _ = write!(out, " btn={}", format_button(button));

            if *jitter > 0 {
                let _ = write!(out, " jitter={}", jitter);
            }
            let _ = writeln!(out);
        }
        MacroEvent::MouseUp {
            position,
            button,
            jitter,
        } => {
            let _ = write!(out, "MouseUp ");
            write_position(out, position);
            let _ = write!(out, " btn={}", format_button(button));

            if *jitter > 0 {
                let _ = write!(out, " jitter={}", jitter);
            }
            let _ = writeln!(out);
        }
        MacroEvent::Scroll { dx, dy } => {
            let _ = writeln!(out, "Scroll dx={} dy={}", dx, dy);
        }
        MacroEvent::KeyDown { key, code } => {
            let _ = writeln!(out, "KeyDown key=\"{}\" code={}", esc(key), code);
        }
        MacroEvent::KeyUp { key, code } => {
            let _ = writeln!(out, "KeyUp key=\"{}\" code={}", esc(key), code);
        }
        MacroEvent::KeyPress {
            key,
            code,
            hold_time_ms,
        } => {
            let _ = writeln!(
                out,
                "KeyPress key=\"{}\" code={} hold_time_ms={}",
                esc(key),
                code,
                hold_time_ms
            );
        }
    }
}

fn write_position(out: &mut String, position: &MousePosition) {
    match position {
        MousePosition::Absolute { x, y } => {
            let _ = write!(out, "x={} y={}", format_coord(x), format_coord(y));
        }
        MousePosition::Current => {
            let _ = write!(out, "pos=current");
        }
    }
}

fn format_button(button: &MacroButton) -> &'static str {
    match button {
        MacroButton::Left => "Left",
        MacroButton::Right => "Right",
        MacroButton::Middle => "Middle",
    }
}
