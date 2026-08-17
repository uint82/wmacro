// TODO: add whole-word and regex match options to find & replace.

use wmacro_core_types::MacroCommand;

/// replaces every case-insensitive occurrence of `from_lower` (already lowercased by the caller) in `field` with `to`.
pub fn replace_in_field(field: &mut String, from_lower: &str, to: &str) -> usize {
    let haystack = field.to_lowercase();
    let count = haystack.matches(from_lower).count();
    if count == 0 {
        return 0;
    }

    let mut out = String::with_capacity(field.len());
    let mut rest = field.as_str();
    let mut search = haystack.as_str();
    // walk both strings in lockstep so slicing the original uses byte offsets from the lowercased copy.
    while let Some(pos) = search.find(from_lower) {
        out.push_str(&rest[..pos]);
        out.push_str(to);
        rest = &rest[pos + from_lower.len()..];
        search = &search[pos + from_lower.len()..];
    }
    out.push_str(rest);
    *field = out;
    count
}

/// replaces in the raw string fields of a single command; returns the count (0 when none match).
pub fn replace_in_command(cmd: &mut MacroCommand, from_lower: &str, to: &str) -> usize {
    let mut count = 0;
    let mut apply = |field: &mut String| {
        count += replace_in_field(field, from_lower, to);
    };

    match cmd {
        MacroCommand::TypeText(text) => apply(text),
        MacroCommand::Label(name) => apply(name),
        MacroCommand::Goto(target) => apply(target),
        MacroCommand::OpenFile { path, args, .. } => {
            apply(path);
            apply(args);
        }
        MacroCommand::PlayMacro(path) => apply(path),
        MacroCommand::Calculate { target, expression } => {
            apply(target);
            apply(expression);
        }
        MacroCommand::GetClipboard { target } => apply(target),
        MacroCommand::Comment(text) => apply(text),
        _ => {}
    }
    count
}

pub fn replace_in_row(
    commands: &mut [MacroCommand],
    row: usize,
    from_lower: &str,
    to: &str,
) -> usize {
    let Some(cmd) = commands.get_mut(row) else {
        return 0;
    };
    replace_in_command(cmd, from_lower, to)
}

pub fn replace_all_in(commands: &mut [MacroCommand], from_lower: &str, to: &str) -> usize {
    commands
        .iter_mut()
        .map(|cmd| replace_in_command(cmd, from_lower, to))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmacro_core_types::{MacroCommand, MacroEvent};

    #[test]
    fn replace_in_field_is_case_insensitive() {
        let mut field = "Hello WORLD hello".to_string();
        let count = replace_in_field(&mut field, "hello", "bye");
        assert_eq!(count, 2);
        assert_eq!(field, "bye WORLD bye");
    }

    #[test]
    fn replace_in_field_absent_leaves_untouched() {
        let mut field = "nothing here".to_string();
        let count = replace_in_field(&mut field, "nope", "x");
        assert_eq!(count, 0);
        assert_eq!(field, "nothing here");
    }

    #[test]
    fn replace_in_command_touches_text_fields_only() {
        let mut cmd = MacroCommand::TypeText("click to continue".to_string());
        let count = replace_in_command(&mut cmd, "click", "press");
        assert_eq!(count, 1);
        assert_eq!(cmd, MacroCommand::TypeText("press to continue".to_string()));

        let mut delay = MacroCommand::Action(MacroEvent::Delay(1000));
        let count = replace_in_command(&mut delay, "delay", "x");
        assert_eq!(count, 0);
        assert_eq!(delay, MacroCommand::Action(MacroEvent::Delay(1000)));
    }

    #[test]
    fn replace_all_in_totals_across_rows() {
        let mut commands = vec![
            MacroCommand::TypeText("say hello".to_string()),
            MacroCommand::Goto("hello_world".to_string()),
            MacroCommand::Action(MacroEvent::Click {
                position: wmacro_core_types::MousePosition::Current,
                button: wmacro_core_types::MacroButton::Left,
                jitter: 0,
                hold_time_ms: 30,
            }),
        ];
        let count = replace_all_in(&mut commands, "hello", "hi");
        assert_eq!(count, 2);
        assert_eq!(commands[0], MacroCommand::TypeText("say hi".to_string()));
        assert_eq!(commands[1], MacroCommand::Goto("hi_world".to_string()));
    }
}
