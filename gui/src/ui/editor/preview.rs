use crate::macro_engine::script::format_operand;
use crate::ui::components::format_coord_display;
use crate::ui::modals::variable::format_duration_string;
use wmacro_core_types::{MacroCommand, MacroEvent};

pub fn row_preview(cmd: &MacroCommand) -> String {
    // TODO: keep a per-command cached preview; this runs on every hover frame.
    match cmd {
        MacroCommand::Action(ev) => action_preview(ev),
        MacroCommand::IfPixelColor {
            x,
            y,
            r,
            g,
            b,
            tolerance,
        } => {
            let mut lines = vec![format!(
                "If the pixel at ({}, {}) is #{:02X}{:02X}{:02X}",
                format_coord_display(x),
                format_coord_display(y),
                r,
                g,
                b
            )];
            if *tolerance > 0 {
                lines.push(format!("tolerance {}%", tolerance));
            }
            lines.push("then run the block below, else jump to the matching End If.".to_string());
            lines.join("\n")
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
            let mut lines = vec![format!(
                "If the image {} is {}found{}",
                target_image_path,
                if *trigger_if_not_found { "NOT " } else { "" },
                match region {
                    Some((l, t, w, h)) => format!(" within region {},{} {}x{}", l, t, w, h),
                    None => " on the whole screen".to_string(),
                }
            )];
            lines.push(format!("minimum similarity {:.2}", similarity_threshold));
            match (store_x, store_y) {
                (Some(sx), Some(sy)) => {
                    lines.push(format!("on match, saves position to ${} and ${}", sx, sy))
                }
                (Some(sx), None) => lines.push(format!("on match, saves x to ${}", sx)),
                (None, Some(sy)) => lines.push(format!("on match, saves y to ${}", sy)),
                (None, None) => {}
            }
            if !*trigger_if_not_found && *move_cursor_if_found {
                lines.push("on match, moves the cursor to the image".to_string());
            }
            lines.push("then run the block below, else jump to the matching End If.".to_string());
            lines.join("\n")
        }
        MacroCommand::IfColorFound {
            region,
            r,
            g,
            b,
            tolerance,
            store_x,
            store_y,
            store_w,
            store_h,
            ..
        } => {
            let mut lines = vec![format!(
                "If the color #{:02X}{:02X}{:02X} is found{}",
                r,
                g,
                b,
                match region {
                    Some((l, t, w, h)) => format!(" within region {},{} {}x{}", l, t, w, h),
                    None => format!(" on the whole screen (tolerance {}%)", tolerance),
                }
            )];
            if let (Some(sx), Some(sy)) = (store_x, store_y) {
                let mut saves = format!("on match, saves to ${} and ${}", sx, sy);
                if let Some(sw) = store_w {
                    saves.push_str(&format!(", ${}", sw));
                }
                if let Some(sh) = store_h {
                    saves.push_str(&format!(", ${}", sh));
                }
                lines.push(saves);
            }
            lines.push("then run the block below, else jump to the matching End If.".to_string());
            lines.join("\n")
        }
        MacroCommand::Else => "Runs when the surrounding If condition was not met.".to_string(),
        MacroCommand::EndIf => {
            "End of the If block; execution continues here after the block.".to_string()
        }
        MacroCommand::Loop { count } => {
            format!("Repeats the block below {} times.", format_operand(count))
        }
        MacroCommand::EndLoop => "End of the loop; jumps back to its matching Loop.".to_string(),
        MacroCommand::PlayMacro(path) => format!("Runs the macro at:\n{}", path),
        MacroCommand::Label(name) => format!("Defines a jump target named \"{}\".", name),
        MacroCommand::Goto(target) => format!("Jumps to the label \"{}\".", target),
        MacroCommand::TypeText(text) => format!("Types the text:\n{}", text),
        MacroCommand::OpenFile {
            path,
            args,
            run_as_admin,
        } => {
            let mut lines = vec![format!("Opens:\n{}", path)];
            if !args.trim().is_empty() {
                lines.push(format!("with arguments: {}", args));
            }
            if *run_as_admin {
                lines.push("with administrator privileges".to_string());
            }
            lines.join("\n")
        }
        MacroCommand::SetVariable { target, value } => format!(
            "Stores the value {} into the variable ${}.",
            format_operand(value),
            target
        ),
        MacroCommand::Calculate { target, expression } => {
            format!(
                "Calculates {} and stores the result in ${}.",
                expression, target
            )
        }
        MacroCommand::IfCompare { left, op, right } => format!(
            "If {} {} {}\nthen run the block below, else jump to the matching End If.",
            format_operand(left),
            op.symbol(),
            format_operand(right)
        ),
        MacroCommand::Delay { duration_ms } => format!(
            "Waits {} before the next command.",
            format_duration_string(match duration_ms {
                wmacro_core_types::Operand::Literal(wmacro_core_types::Value::Number(ms)) => {
                    *ms as u64
                }
                _ => 0,
            })
        ),
        MacroCommand::SetClipboard { text } => {
            format!("Copies {} to the system clipboard.", format_operand(text))
        }
        MacroCommand::GetClipboard { target } => {
            format!("Reads the system clipboard into ${}.", target)
        }
        MacroCommand::Comment(text) => format!("Note:\n{}", text),
    }
}

fn action_preview(ev: &MacroEvent) -> String {
    match ev {
        MacroEvent::Delay(us) => {
            format!(
                "Waits {} before the next command.",
                format_duration_string(us / 1000)
            )
        }
        MacroEvent::MouseMove { x, y } => {
            format!(
                "Moves the cursor to ({}, {}).",
                format_coord_display(x),
                format_coord_display(y)
            )
        }
        MacroEvent::Click {
            position,
            button,
            hold_time_ms,
            ..
        } => match position {
            wmacro_core_types::MousePosition::Absolute { x, y } => format!(
                "{} clicks at ({}, {}) for {} ms.",
                button_label(button),
                format_coord_display(x),
                format_coord_display(y),
                hold_time_ms
            ),
            wmacro_core_types::MousePosition::Current => format!(
                "{} clicks at the current cursor position for {} ms.",
                button_label(button),
                hold_time_ms
            ),
        },
        MacroEvent::MouseDown {
            position, button, ..
        } => match position {
            wmacro_core_types::MousePosition::Absolute { x, y } => format!(
                "{} pressed at ({}, {}).",
                button_label(button),
                format_coord_display(x),
                format_coord_display(y)
            ),
            wmacro_core_types::MousePosition::Current => {
                format!(
                    "{} pressed at the current cursor position.",
                    button_label(button)
                )
            }
        },
        MacroEvent::MouseUp {
            position, button, ..
        } => match position {
            wmacro_core_types::MousePosition::Absolute { x, y } => format!(
                "{} released at ({}, {}).",
                button_label(button),
                format_coord_display(x),
                format_coord_display(y)
            ),
            wmacro_core_types::MousePosition::Current => {
                format!(
                    "{} released at the current cursor position.",
                    button_label(button)
                )
            }
        },
        MacroEvent::Scroll { dx, dy } => format!("Scrolls {} and {}.", dx, dy),
        MacroEvent::KeyDown { key, .. } => format!("Holds the {} key down.", key),
        MacroEvent::KeyUp { key, .. } => format!("Releases the {} key.", key),
        MacroEvent::KeyPress {
            key, hold_time_ms, ..
        } => format!("Presses {} (held for {} ms).", key, hold_time_ms),
    }
}

fn button_label(button: &wmacro_core_types::MacroButton) -> &'static str {
    match button {
        wmacro_core_types::MacroButton::Left => "Left",
        wmacro_core_types::MacroButton::Right => "Right",
        wmacro_core_types::MacroButton::Middle => "Middle",
    }
}
