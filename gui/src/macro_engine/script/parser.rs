use log::warn;
use std::collections::{HashMap, HashSet};
use wmacro_core_types::{
    CompareOp, Coord, Macro, MacroButton, MacroCommand, MacroEvent, MousePosition, Operand, Value,
};

const DEFAULT_VERSION: u8 = 3;

/// parses a `.wmr` script text into a macro; header lines set the name and
/// format version, the remaining lines are parsed as commands.
pub fn deserialize(script: &str) -> Result<Macro, String> {
    let mut name = String::from("untitled");
    let mut version: u8 = 3;
    let mut commands = Vec::new();

    for (line_idx, line) in script.lines().enumerate() {
        let Some(line) = parse_line(line) else {
            continue;
        };

        match line.cmd.as_str() {
            "version" => version = parse_version(&line.raw, version),
            "name" => name = parse_quoted_name(&line.raw).unwrap_or(name),
            _ => {
                if let Some(command) = parse_command(&line) {
                    commands.push(command);
                } else {
                    // TODO: surface unrecognized-command warnings in the GUI instead of only the log.
                    warn!("Unknown command on line {}: {}", line_idx + 1, line.cmd);
                }
            }
        }
    }

    let mut m = Macro::new(name);
    m.version = version;
    m.commands = commands;
    Ok(m)
}

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

/// strips a matching pair of double or single quotes, collapsing doubled
/// pairs (`""` / `''`) into single quotes; returns `None` when `s` is not a
/// quoted literal. shared with the UI operand fields for consistent syntax.
pub fn strip_quotes(s: &str) -> Option<String> {
    let quote = s.chars().next()?;
    if (quote != '"' && quote != '\'') || s.len() < 2 || !s.ends_with(quote) {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    if !inner.contains(quote) {
        return Some(inner.to_string());
    }
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == quote && chars.peek() == Some(&quote) {
            chars.next();
        }
    }
    Some(out)
}

/// a parsed script line; `quoted` holds args written in double quotes
/// (quoted text is a string literal, unquoted text a number).
struct ParsedLine {
    raw: String,
    cmd: String,
    args: HashMap<String, String>,
    quoted: HashSet<String>,
}

/// tokenizes one script line into a command name and `key=value` args.
fn parse_line(line: &str) -> Option<ParsedLine> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
    let cmd = cmd.to_string();

    let mut args = HashMap::new();
    let mut quoted = HashSet::new();
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
            warn!(
                "Skipping malformed token in command '{}': missing '=' after key '{}'",
                cmd, key
            );
            continue;
        }

        let mut val = String::new();
        if let Some(&quote) = chars.peek().filter(|&&c| c == '"' || c == '\'') {
            chars.next();
            while let Some(&c) = chars.peek() {
                if c == quote {
                    chars.next();
                    // a doubled delimiter is an escaped literal (e.g. `""` means one `"`), not the end of the value.
                    if chars.peek() == Some(&quote) {
                        chars.next();
                        val.push(quote);
                    } else {
                        break;
                    }
                } else {
                    val.push(c);
                    chars.next();
                }
            }
            quoted.insert(key.clone());
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

    Some(ParsedLine {
        raw: line.to_string(),
        cmd,
        args,
        quoted,
    })
}

fn parse_command(line: &ParsedLine) -> Option<MacroCommand> {
    let args = &line.args;
    match line.cmd.as_str() {
        "Delay" => {
            // delay lines: manual `duration_ms=<var|500>`, recorded `us=<us>`, legacy `ms=<ms>`.
            if args.contains_key("duration_ms") {
                Some(MacroCommand::Delay {
                    duration_ms: parse_operand(args, &line.quoted, "duration_ms"),
                })
            } else {
                Some(MacroCommand::Action(MacroEvent::Delay(
                    parse_recorded_delay_us(args),
                )))
            }
        }
        "MouseMove" => Some(MacroCommand::Action(MacroEvent::MouseMove {
            x: parse_coord(args, "x"),
            y: parse_coord(args, "y"),
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
            x: parse_coord(args, "x"),
            y: parse_coord(args, "y"),
            r: arg_or(args, "r", 0),
            g: arg_or(args, "g", 0),
            b: arg_or(args, "b", 0),
            tolerance: arg_or(args, "tol", 0),
        }),
        "IfImageFound" => Some(MacroCommand::IfImageFound {
            target_image_path: args.get("target").cloned().unwrap_or_default(),
            similarity_threshold: arg_or(args, "tol", 0.0),
            move_cursor_if_found: arg_or(args, "move", false),
            trigger_if_not_found: arg_or(args, "not_found", false),
            region: parse_region(args),
            store_x: parse_store_var(args, "store_x"),
            store_y: parse_store_var(args, "store_y"),
        }),
        "IfColorFound" => {
            let (r, g, b) = parse_hex_color(args.get("color").map(String::as_str));
            Some(MacroCommand::IfColorFound {
                region: parse_region(args),
                r,
                g,
                b,
                tolerance: arg_or(args, "tol", 0),
                min_width: arg_or(args, "min_width", 1),
                min_height: arg_or(args, "min_height", 1),
                move_cursor_if_found: arg_or(args, "move", false),
                store_x: parse_store_var(args, "store_x"),
                store_y: parse_store_var(args, "store_y"),
                store_w: parse_store_var(args, "store_w"),
                store_h: parse_store_var(args, "store_h"),
            })
        }
        "EndIf" => Some(MacroCommand::EndIf),
        "Loop" => Some(MacroCommand::Loop {
            count: parse_operand(args, &line.quoted, "count"),
        }),
        "Else" => Some(MacroCommand::Else),
        "EndLoop" => Some(MacroCommand::EndLoop),
        "PlayMacro" => args.get("path").cloned().map(MacroCommand::PlayMacro),
        "Label" => args.get("name").cloned().map(MacroCommand::Label),
        "Goto" => args.get("target").cloned().map(MacroCommand::Goto),
        "TypeText" => args.get("text").cloned().map(MacroCommand::TypeText),
        "OpenFile" => {
            let path = args.get("path").cloned().unwrap_or_default();
            let args_str = args.get("args").cloned().unwrap_or_default();
            let run_as_admin = arg_or(args, "admin", false);
            Some(MacroCommand::OpenFile {
                path,
                args: args_str,
                run_as_admin,
            })
        }
        "SetVariable" => Some(MacroCommand::SetVariable {
            target: args.get("target").cloned().unwrap_or_default(),
            value: parse_operand(args, &line.quoted, "value"),
        }),
        "Calculate" => Some(MacroCommand::Calculate {
            target: args.get("target").cloned().unwrap_or_default(),
            expression: args
                .get("expr")
                .cloned()
                .or_else(|| legacy_calculate_expression(args))
                .unwrap_or_default(),
        }),
        "IfCompare" => Some(MacroCommand::IfCompare {
            left: parse_operand(args, &line.quoted, "left"),
            op: parse_compare_op(args),
            right: parse_operand(args, &line.quoted, "right"),
        }),
        "SetClipboard" => Some(MacroCommand::SetClipboard {
            text: parse_operand(args, &line.quoted, "text"),
        }),
        "GetClipboard" => Some(MacroCommand::GetClipboard {
            target: args.get("target").cloned().unwrap_or_default(),
        }),
        "Comment" => args.get("text").cloned().map(MacroCommand::Comment),
        _ => None,
    }
}

/// converts the legacy `Calculate` line (`left op right`) into formula text so old macros still load.
fn legacy_calculate_expression(args: &HashMap<String, String>) -> Option<String> {
    let left = args.get("left")?;
    let right = args.get("right")?;
    let op = args.get("op").map(String::as_str).unwrap_or("+");
    Some(format!("{} {} {}", left, op, right))
}

/// parses an operand: a quoted literal (never interpreted, even when it
/// starts with `$`; delimiters are doubled, e.g. `"a""b"` is `a"b`), a `$`
/// variable reference, or a number; anything unparseable reads as 0.
fn parse_operand(args: &HashMap<String, String>, quoted: &HashSet<String>, key: &str) -> Operand {
    match args.get(key) {
        Some(raw) if quoted.contains(key) => Operand::Literal(Value::Text(raw.clone())),
        Some(raw) if raw.starts_with('$') => Operand::Var(raw[1..].to_string()),
        Some(raw) => raw
            .parse::<i64>()
            .map(|n| Operand::Literal(Value::Number(n)))
            .unwrap_or(Operand::Literal(Value::Number(0))),
        None => Operand::Literal(Value::Number(0)),
    }
}

fn parse_coord(args: &HashMap<String, String>, key: &str) -> Coord {
    match args.get(key) {
        Some(raw) if raw.starts_with('$') => Coord::Var(raw[1..].to_string()),
        Some(raw) => raw
            .parse::<i32>()
            .map(Coord::Const)
            .unwrap_or(Coord::Const(0)),
        None => Coord::Const(0),
    }
}

fn parse_region(args: &HashMap<String, String>) -> Option<(i32, i32, i32, i32)> {
    if !["left", "top", "width", "height"]
        .iter()
        .all(|key| args.contains_key(*key))
    {
        return None;
    }
    Some((
        arg_or(args, "left", 0),
        arg_or(args, "top", 0),
        arg_or(args, "width", 0),
        arg_or(args, "height", 0),
    ))
}

/// a `store_x="name"`-style arg; empty names are treated as absent.
fn parse_store_var(args: &HashMap<String, String>, key: &str) -> Option<String> {
    args.get(key).cloned().filter(|s| !s.is_empty())
}

fn parse_hex_color(raw: Option<&str>) -> (u8, u8, u8) {
    let Some(hex) = raw
        .and_then(|s| s.strip_prefix('#').or(Some(s)))
        .filter(|s| s.len() == 6)
    else {
        return (0, 0, 0);
    };
    let ok = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    match (ok(0), ok(2), ok(4)) {
        (Some(r), Some(g), Some(b)) => (r, g, b),
        _ => (0, 0, 0),
    }
}

fn parse_compare_op(args: &HashMap<String, String>) -> CompareOp {
    match args.get("op").map(String::as_str) {
        Some("!=") => CompareOp::Ne,
        Some("<") => CompareOp::Lt,
        Some("<=") => CompareOp::Le,
        Some(">") => CompareOp::Gt,
        Some(">=") => CompareOp::Ge,
        Some("contains") => CompareOp::Contains,
        _ => CompareOp::Eq,
    }
}

fn parse_position(args: &HashMap<String, String>) -> MousePosition {
    if args.get("pos").map(String::as_str) == Some("current") {
        return MousePosition::Current;
    }
    MousePosition::Absolute {
        x: parse_coord(args, "x"),
        y: parse_coord(args, "y"),
    }
}

fn parse_button(args: &HashMap<String, String>) -> MacroButton {
    parse_btn(args.get("btn").map(String::as_str).unwrap_or("Left"))
}

fn arg_or<T: std::str::FromStr>(args: &HashMap<String, String>, key: &str, default: T) -> T {
    args.get(key)
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// parses the wait of a recorded delay line: the current format stores
/// microseconds (`Delay us=...`), legacy files store milliseconds.
fn parse_recorded_delay_us(args: &HashMap<String, String>) -> u64 {
    if args.contains_key("us") {
        arg_or(args, "us", 0u64)
    } else {
        arg_or(args, "ms", 0u64) * 1000
    }
}

fn parse_key_or_unknown(args: &HashMap<String, String>) -> String {
    args.get("key")
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string())
}

// TODO: unit-test the parser against golden files.
fn parse_btn(s: &str) -> MacroButton {
    match s.to_lowercase().as_str() {
        "right" => MacroButton::Right,
        "middle" => MacroButton::Middle,
        _ => MacroButton::Left,
    }
}
