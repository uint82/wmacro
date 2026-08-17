//! effect commands: `TypeText`, `SetClipboard`, `GetClipboard` and `OpenFile`.

use log::{error, info};
use std::collections::HashMap;
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
}
