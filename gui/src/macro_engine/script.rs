use wmacro_core_types::{Macro, MacroButton, MacroCommand, MacroEvent, MousePosition};
use log::warn;
use std::collections::HashMap;
use std::fmt::Write;

pub fn serialize(m: &Macro) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "# wmacro script");
    let _ = writeln!(out, "version 3");
    let _ = writeln!(out, "name \"{}\"\n", m.name);

    for cmd in &m.commands {
        serialize_command(&mut out, cmd);
    }
    out
}

fn serialize_command(out: &mut String, cmd: &MacroCommand) {
    match cmd {
        MacroCommand::Action(ev) => serialize_event(out, ev),
        MacroCommand::IfPixelColor { x, y, r, g, b, tolerance } => {
            let _ = writeln!(out, "IfPixelColor x={} y={} r={} g={} b={} tol={}", x, y, r, g, b, tolerance);
        }
        MacroCommand::IfImageFound { target_image_path, similarity_threshold, move_cursor_if_found, trigger_if_not_found, region } => {
            let _ = write!(out, "IfImageFound target=\"{}\" tol={} move={} not_found={}", target_image_path, similarity_threshold, move_cursor_if_found, trigger_if_not_found);
            if let Some((l, t, w, h)) = region {
                let _ = writeln!(out, " left={} top={} width={} height={}", l, t, w, h);
            } else {
                let _ = writeln!(out, "");
            }
        }
        MacroCommand::Else => {
            let _ = writeln!(out, "Else");
        }
        MacroCommand::EndIf => {
            let _ = writeln!(out, "EndIf");
        }
        MacroCommand::Loop { count } => {
            let _ = writeln!(out, "Loop count={}", count);
        }
        MacroCommand::EndLoop => {
            let _ = writeln!(out, "EndLoop");
        }
        MacroCommand::PlayMacro(path) => {
            let _ = writeln!(out, "PlayMacro path=\"{}\"", path);
        }
        MacroCommand::Label(name) => {
            let _ = writeln!(out, "Label name=\"{}\"", name);
        }
        MacroCommand::Goto(target) => {
            let _ = writeln!(out, "Goto target=\"{}\"", target);
        }
        MacroCommand::TypeText(text) => {
            let _ = writeln!(out, "TypeText text=\"{}\"", text);
        }
    }
}

fn serialize_event(out: &mut String, ev: &MacroEvent) {
    match ev {
        MacroEvent::Delay(us) => {
            let _ = writeln!(out, "Delay ms={}", us / 1000);
        }
        MacroEvent::MouseMove { x, y } => {
            let _ = writeln!(out, "MouseMove x={} y={}", x, y);
        }
        MacroEvent::Click { position, button, jitter, hold_time_ms } => {
            let _ = write!(out, "Click ");
            write_position(out, position);
            let _ = write!(out, " btn={}", format_button(button));

            if *jitter > 0 {
                let _ = write!(out, " jitter={}", jitter);
            }
            let _ = writeln!(out, " hold={}", hold_time_ms);
        }
        MacroEvent::MouseDown { position, button, jitter } => {
            let _ = write!(out, "MouseDown ");
            write_position(out, position);
            let _ = write!(out, " btn={}", format_button(button));

            if *jitter > 0 {
                let _ = write!(out, " jitter={}", jitter);
            }
            let _ = writeln!(out);
        }
        MacroEvent::MouseUp { position, button, jitter } => {
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
            let _ = writeln!(out, "KeyDown key=\"{}\" code={}", key, code);
        }
        MacroEvent::KeyUp { key, code } => {
            let _ = writeln!(out, "KeyUp key=\"{}\" code={}", key, code);
        }
        MacroEvent::KeyPress { key, code, hold_time_ms } => {
            let _ = writeln!(out, "KeyPress key=\"{}\" code={} hold_time_ms={}", key, code, hold_time_ms);
        }
    }
}

fn write_position(out: &mut String, position: &MousePosition) {
    match position {
        MousePosition::Absolute { x, y } => {
            let _ = write!(out, "x={} y={}", x, y);
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

pub fn deserialize(script: &str) -> Result<Macro, String> {
    let mut name = String::from("untitled");
    let mut version: u8 = 3;
    let mut commands = Vec::new();

    for (line_idx, line) in script.lines().enumerate() {
        let Some((cmd, args)) = parse_line(line) else {
            continue;
        };

        match cmd.as_str() {
            "version" => version = parse_version(line, version),
            "name" => name = parse_quoted_name(line).unwrap_or(name),
            _ => {
                if let Some(command) = parse_command(&cmd, &args) {
                    commands.push(command);
                } else {
                    // TODO: unrecognized commands are logged and dropped for now.
                    warn!("Unknown command on line {}: {}", line_idx + 1, cmd);
                }
            }
        }
    }

    let mut m = Macro::new(name);
    m.version = version;
    m.commands = commands;
    Ok(m)
}

const DEFAULT_VERSION: u8 = 3;

fn parse_version(line: &str, current: u8) -> u8 {
    match line.split_whitespace().nth(1) {
        Some(token) => token.parse().unwrap_or(DEFAULT_VERSION),
        None => current,
    }
}

fn parse_quoted_name(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let end = line.rfind('"')?;
    if start == end {
        return None;
    }
    Some(line[start + 1..end].to_string())
}

/// builds the `MacroCommand` for a known command name. returns `None` for
/// unrecognized commands, caller will log and skip them.
fn parse_command(cmd: &str, args: &HashMap<String, String>) -> Option<MacroCommand> {
    match cmd {
        "Delay" => Some(MacroCommand::Action(MacroEvent::Delay(arg_or(args, "ms", 0) * 1000))),
        "MouseMove" => Some(MacroCommand::Action(MacroEvent::MouseMove {
            x: arg_or(args, "x", 0),
            y: arg_or(args, "y", 0),
        })),
        "Click" => Some(MacroCommand::Action(MacroEvent::Click {
            position: parse_position(args),
            button: parse_button(args),
            jitter: arg_or(args, "jitter", 0),
            hold_time_ms: arg_or(args, "hold", 30),
        })),
        "MouseDown" => Some(MacroCommand::Action(MacroEvent::MouseDown {
            position: parse_position(args),
            button: parse_button(args),
            jitter: arg_or(args, "jitter", 0),
        })),
        "MouseUp" => Some(MacroCommand::Action(MacroEvent::MouseUp {
            position: parse_position(args),
            button: parse_button(args),
            jitter: arg_or(args, "jitter", 0),
        })),
        "Scroll" => Some(MacroCommand::Action(MacroEvent::Scroll {
            dx: arg_or(args, "dx", 0),
            dy: arg_or(args, "dy", 0),
        })),
        "KeyDown" => Some(MacroCommand::Action(MacroEvent::KeyDown {
            key: parse_key_or_unknown(args),
            code: arg_or(args, "code", 0),
        })),
        "KeyUp" => Some(MacroCommand::Action(MacroEvent::KeyUp {
            key: parse_key_or_unknown(args),
            code: arg_or(args, "code", 0),
        })),
        "KeyPress" => Some(MacroCommand::Action(MacroEvent::KeyPress {
            key: parse_key_or_unknown(args),
            code: arg_or(args, "code", 0),
            hold_time_ms: arg_or(args, "hold_time_ms", 30),
        })),
        "IfPixelColor" => Some(MacroCommand::IfPixelColor {
            x: arg_or(args, "x", 0),
            y: arg_or(args, "y", 0),
            r: arg_or(args, "r", 0),
            g: arg_or(args, "g", 0),
            b: arg_or(args, "b", 0),
            tolerance: arg_or(args, "tol", 0),
        }),
        "IfImageFound" => {
            let target_image_path = args.get("target").cloned().unwrap_or_default();
            let similarity_threshold = arg_or(args, "tol", 0.0);
            let move_cursor_if_found = arg_or(args, "move", false);
            let trigger_if_not_found = arg_or(args, "not_found", false);
            let region = if args.contains_key("left") && args.contains_key("top") && args.contains_key("width") && args.contains_key("height") {
                Some((
                    arg_or(args, "left", 0),
                    arg_or(args, "top", 0),
                    arg_or(args, "width", 0),
                    arg_or(args, "height", 0)
                ))
            } else {
                None
            };
            Some(MacroCommand::IfImageFound {
                target_image_path,
                similarity_threshold,
                move_cursor_if_found,
                trigger_if_not_found,
                region
            })
        },
        "EndIf" => Some(MacroCommand::EndIf),
        "Loop" => Some(MacroCommand::Loop { count: arg_or(args, "count", 1) }),
        "Else" => Some(MacroCommand::Else),
        "EndLoop" => Some(MacroCommand::EndLoop),
        "PlayMacro" => args.get("path").cloned().map(MacroCommand::PlayMacro),
        "Label" => args.get("name").cloned().map(MacroCommand::Label),
        "Goto" => args.get("target").cloned().map(MacroCommand::Goto),
        "TypeText" => args.get("text").cloned().map(MacroCommand::TypeText),
        _ => None,
    }
}

fn parse_position(args: &HashMap<String, String>) -> MousePosition {
    if args.get("pos").map(String::as_str) == Some("current") {
        return MousePosition::Current;
    }
    MousePosition::Absolute {
        x: arg_or(args, "x", 0),
        y: arg_or(args, "y", 0),
    }
}

fn parse_button(args: &HashMap<String, String>) -> MacroButton {
    parse_btn(args.get("btn").map(String::as_str).unwrap_or("Left"))
}

fn arg_or<T: std::str::FromStr>(args: &HashMap<String, String>, key: &str, default: T) -> T {
    args.get(key).and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn parse_key_or_unknown(args: &HashMap<String, String>) -> String {
    args.get("key").cloned().unwrap_or_else(|| "Unknown".to_string())
}

fn parse_btn(s: &str) -> MacroButton {
    match s.to_lowercase().as_str() {
        "right" => MacroButton::Right,
        "middle" => MacroButton::Middle,
        _ => MacroButton::Left,
    }
}

fn parse_line(line: &str) -> Option<(String, HashMap<String, String>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
    let cmd = cmd.to_string();

    let mut args = HashMap::new();
    let mut chars = rest.chars().peekable();

    while chars.peek().is_some() {
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek().is_none() {
            break;
        }

        let mut key = String::new();
        while let Some(&c) = chars.peek() {
            if c == '=' || c.is_whitespace() {
                break;
            }
            key.push(c);
            chars.next();
        }

        if chars.peek() == Some(&'=') {
            chars.next();
        } else {
            warn!("Skipping malformed token in command '{}': missing '=' after key '{}'", cmd, key);
            continue;
        }

        let mut val = String::new();
        if chars.peek() == Some(&'"') {
            chars.next();
            while let Some(&c) = chars.peek() {
                if c == '"' {
                    chars.next();
                    break;
                }
                val.push(c);
                chars.next();
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                val.push(c);
                chars.next();
            }
        }
        args.insert(key, val);
    }

    Some((cmd, args))
}
