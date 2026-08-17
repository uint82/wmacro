use log::error;

use crate::macro_engine::player::frame::ExecFrame;
use crate::macro_engine::player::models::{FlowControl, PlaybackContext};

/// pushes a nested macro onto the exec stack; the parent frame resumes after it.
pub(super) fn execute_play_macro(
    path: &str,
    ctx: &mut PlaybackContext,
    frame: &mut ExecFrame,
) -> FlowControl {
    if let Ok(nested_macro) = crate::macro_engine::storage::load_wmr(std::path::Path::new(path)) {
        frame.idx += 1;
        ctx.exec_stack.push(frame.clone());
        ctx.exec_stack.push(ExecFrame::new(nested_macro.commands));
        FlowControl::BreakFrame
    } else {
        error!("Failed to load nested macro: {}", path);
        FlowControl::Continue
    }
}

pub(super) fn execute_goto(target: &str, frame: &mut ExecFrame) -> FlowControl {
    if let Some(target_idx) = frame.find_label(target) {
        // jump lands exactly on the label; the Jump flow skips the automatic
        // advance, so no off-by-one arithmetic is needed here.
        frame.idx = target_idx;
        FlowControl::Jump
    } else {
        log::warn!(
            "Warning: Goto target '{}' not found in current macro.",
            target
        );
        FlowControl::Continue
    }
}

pub(super) fn execute_else(frame: &mut ExecFrame) -> FlowControl {
    frame.skip_to_endif();
    FlowControl::Continue
}

pub(super) fn execute_loop_cmd(count: u64, frame: &mut ExecFrame) -> FlowControl {
    if count == 0 {
        frame.skip_to_endloop();
    } else if frame.loop_stack.last().map(|&(i, _)| i) != Some(frame.idx) {
        frame.loop_stack.push((frame.idx, count));
    }
    FlowControl::Continue
}

pub(super) fn execute_end_loop(frame: &mut ExecFrame) -> FlowControl {
    if let Some((start_idx, remaining)) = frame.loop_stack.pop()
        && remaining > 1
    {
        frame.loop_stack.push((start_idx, remaining - 1));
        // jump back to the Loop command; re-running it is a no-op thanks
        // to the loop_stack guard in execute_loop_cmd.
        frame.idx = start_idx;
        return FlowControl::Jump;
    }
    FlowControl::Continue
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use wmacro_core_types::{CompareOp, MacroCommand, Operand, Value};

    use crate::macro_engine::player::variables::{
        execute_calculate, execute_if_compare, resolve_num,
    };

    fn run_commands(commands: Vec<MacroCommand>, vars: &mut HashMap<String, Value>) -> Vec<String> {
        let mut frame = ExecFrame::new(commands);
        let mut seen = Vec::new();
        let mut steps = 0;
        while frame.idx < frame.commands.len() {
            let cmd = frame.commands[frame.idx].clone();
            let flow = match &cmd {
                MacroCommand::Loop { count } => {
                    let n = resolve_num(count, vars).max(0) as u64;
                    execute_loop_cmd(n, &mut frame)
                }
                MacroCommand::EndLoop => execute_end_loop(&mut frame),
                MacroCommand::Goto(target) => execute_goto(target, &mut frame),
                MacroCommand::Calculate { target, expression } => {
                    execute_calculate(target, expression, vars);
                    FlowControl::Continue
                }
                MacroCommand::IfCompare { left, op, right } => {
                    execute_if_compare(left, *op, right, vars, &mut frame)
                }
                MacroCommand::TypeText(t) => {
                    seen.push(t.clone());
                    FlowControl::Continue
                }
                MacroCommand::Label(_) | MacroCommand::Else | MacroCommand::EndIf => {
                    FlowControl::Continue
                }
                _ => FlowControl::Continue,
            };
            match flow {
                FlowControl::Continue => frame.idx += 1,
                FlowControl::Jump => {}
                FlowControl::BreakFrame | FlowControl::Stop => break,
            }
            steps += 1;
            assert!(steps < 200, "runaway loop in test commands");
        }
        seen
    }

    /// regression: a `Loop` at index 0 used to underflow `start_idx - 1`.
    #[test]
    fn end_loop_at_index_zero_iterates_full_count() {
        let seen = run_commands(
            vec![
                MacroCommand::Loop {
                    count: Operand::Literal(Value::Number(3)),
                },
                MacroCommand::TypeText("tick".into()),
                MacroCommand::EndLoop,
                MacroCommand::TypeText("done".into()),
            ],
            &mut HashMap::new(),
        );
        assert_eq!(seen, vec!["tick", "tick", "tick", "done"]);
    }

    #[test]
    fn end_loop_keeps_working_when_loop_not_at_index_zero() {
        let seen = run_commands(
            vec![
                MacroCommand::TypeText("pre".into()),
                MacroCommand::Loop {
                    count: Operand::Literal(Value::Number(2)),
                },
                MacroCommand::TypeText("tick".into()),
                MacroCommand::EndLoop,
            ],
            &mut HashMap::new(),
        );
        assert_eq!(seen, vec!["pre", "tick", "tick"]);
    }

    /// regression: `Goto` to a label at index 0 used to underflow `target_idx - 1`.
    #[test]
    fn goto_to_label_at_index_zero_lands_on_the_label() {
        let mut frame = ExecFrame::new(vec![
            MacroCommand::Label("top".into()),
            MacroCommand::TypeText("body".into()),
            MacroCommand::Goto("top".into()),
        ]);

        frame.idx = 2;
        let flow = execute_goto("top", &mut frame);
        assert!(matches!(flow, FlowControl::Jump));
        assert_eq!(frame.idx, 0, "idx must point at the label itself");
    }

    #[test]
    fn goto_to_mid_macro_label_lands_on_the_label() {
        let mut frame = ExecFrame::new(vec![
            MacroCommand::TypeText("a".into()),
            MacroCommand::Label("mid".into()),
            MacroCommand::TypeText("b".into()),
            MacroCommand::Goto("mid".into()),
        ]);

        frame.idx = 3;
        let flow = execute_goto("mid", &mut frame);
        assert!(matches!(flow, FlowControl::Jump));
        assert_eq!(frame.idx, 1, "idx must point at the label itself");
    }

    #[test]
    fn goto_to_missing_label_warns_and_continues_in_place() {
        let mut frame = ExecFrame::new(vec![
            MacroCommand::TypeText("a".into()),
            MacroCommand::Goto("nope".into()),
        ]);

        frame.idx = 1;
        let flow = execute_goto("nope", &mut frame);
        assert!(matches!(flow, FlowControl::Continue));
        assert_eq!(frame.idx, 1, "a missing target must not move the cursor");
    }

    /// end-to-end replica of the cast/claim pattern: Goto must rewind to index 0.
    #[test]
    fn goto_loop_back_to_index_zero_runs_expected_iterations() {
        let mut vars = HashMap::new();
        vars.insert("n".to_string(), Value::Number(0));

        let seen = run_commands(
            vec![
                MacroCommand::Label("top".into()),
                MacroCommand::Calculate {
                    target: "n".into(),
                    expression: "$n + 1".into(),
                },
                MacroCommand::IfCompare {
                    left: Operand::Var("n".into()),
                    op: CompareOp::Lt,
                    right: Operand::Literal(Value::Number(3)),
                },
                MacroCommand::Goto("top".into()),
                MacroCommand::Else,
                MacroCommand::EndIf,
                MacroCommand::TypeText("done".into()),
            ],
            &mut vars,
        );

        assert_eq!(seen, vec!["done".to_string()]);
        assert_eq!(vars.get("n"), Some(&Value::Number(3)));
    }
}
