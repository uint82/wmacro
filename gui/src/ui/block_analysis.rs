//! structural analysis of a macro's block commands (If/Else/EndIf, Loop/EndLoop): per-row masks for orphan ends, unclosed openers, and fold pairs.

use wmacro_core_types::MacroCommand;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockKind {
    If,
    Loop,
}

/// structural analysis of a macro's block commands (If/Else/EndIf, Loop/EndLoop).
/// the per-row masks are indexed like the `commands` slice they were computed
/// from; `fold_end[i]` is the row a block opener closes at (its matching EndIf/EndLoop, or the last row when never closed).
#[derive(Clone, Debug, Default)]
pub struct BlockAnalysis {
    pub open_ifs: usize,
    pub open_loops: usize,
    pub orphan_end: Vec<bool>,
    pub unclosed_opener: Vec<bool>,
    pub fold_end: Vec<Option<usize>>,
}

pub fn analyze_blocks(commands: &[MacroCommand]) -> BlockAnalysis {
    let mut open_ifs: usize = 0;
    let mut open_loops: usize = 0;
    let mut openers: Vec<(usize, BlockKind)> = Vec::new();
    let mut orphan_end = vec![false; commands.len()];
    let mut unclosed_opener = vec![false; commands.len()];
    let mut fold_end = vec![None; commands.len()];

    for (idx, cmd) in commands.iter().enumerate() {
        match cmd {
            MacroCommand::IfPixelColor { .. }
            | MacroCommand::IfImageFound { .. }
            | MacroCommand::IfColorFound { .. }
            | MacroCommand::IfCompare { .. } => {
                open_ifs += 1;
                openers.push((idx, BlockKind::If));
            }
            MacroCommand::Loop { .. } => {
                open_loops += 1;
                openers.push((idx, BlockKind::Loop));
            }
            MacroCommand::Else => {
                if open_ifs == 0 {
                    orphan_end[idx] = true;
                }
            }
            MacroCommand::EndIf => {
                if open_ifs == 0 {
                    orphan_end[idx] = true;
                } else {
                    open_ifs -= 1;
                    if let Some(pos) = nearest_opener(&openers, BlockKind::If) {
                        let (opener_idx, _) = openers.remove(pos);
                        fold_end[opener_idx] = Some(idx);
                    }
                }
            }
            MacroCommand::EndLoop => {
                if open_loops == 0 {
                    orphan_end[idx] = true;
                } else {
                    open_loops -= 1;
                    if let Some(pos) = nearest_opener(&openers, BlockKind::Loop) {
                        let (opener_idx, _) = openers.remove(pos);
                        fold_end[opener_idx] = Some(idx);
                    }
                }
            }
            _ => {}
        }
    }

    // any opener still on the stack at the end is unclosed; its fold range extends to the last row so folding still works on broken macros.
    let last_row = commands.len().saturating_sub(1);
    for (opener_idx, _) in openers {
        unclosed_opener[opener_idx] = true;
        fold_end[opener_idx] = Some(last_row);
    }

    BlockAnalysis {
        open_ifs,
        open_loops,
        orphan_end,
        unclosed_opener,
        fold_end,
    }
}

fn nearest_opener(openers: &[(usize, BlockKind)], kind: BlockKind) -> Option<usize> {
    openers
        .iter()
        .rposition(|(_, opener_kind)| *opener_kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn if_cmd() -> MacroCommand {
        MacroCommand::IfCompare {
            left: wmacro_core_types::Operand::Literal(wmacro_core_types::Value::Number(1)),
            op: wmacro_core_types::CompareOp::Gt,
            right: wmacro_core_types::Operand::Literal(wmacro_core_types::Value::Number(0)),
        }
    }

    fn loop_cmd() -> MacroCommand {
        MacroCommand::Loop {
            count: wmacro_core_types::Operand::Literal(wmacro_core_types::Value::Number(5)),
        }
    }

    #[test]
    fn empty_macro_has_no_issues() {
        let a = analyze_blocks(&[]);
        assert_eq!(a.open_ifs, 0);
        assert_eq!(a.open_loops, 0);
        assert!(a.orphan_end.is_empty());
        assert!(a.unclosed_opener.is_empty());
    }

    #[test]
    fn balanced_blocks_have_no_issues() {
        let commands = vec![
            if_cmd(),
            MacroCommand::Else,
            MacroCommand::EndIf,
            loop_cmd(),
            MacroCommand::EndLoop,
        ];
        let a = analyze_blocks(&commands);
        assert_eq!(a.open_ifs, 0);
        assert_eq!(a.open_loops, 0);
        assert!(!a.orphan_end.iter().any(|&o| o));
        assert!(!a.unclosed_opener.iter().any(|&u| u));
    }

    #[test]
    fn orphan_close_is_marked() {
        let commands = vec![
            MacroCommand::Else,
            MacroCommand::EndIf,
            MacroCommand::EndLoop,
        ];
        let a = analyze_blocks(&commands);
        assert!(a.orphan_end.iter().all(|&o| o));
        assert_eq!(a.open_ifs, 0);
        assert_eq!(a.open_loops, 0);
    }

    #[test]
    fn unclosed_opener_is_marked() {
        let commands = vec![if_cmd(), loop_cmd()];
        let a = analyze_blocks(&commands);
        assert_eq!(a.open_ifs, 1);
        assert_eq!(a.open_loops, 1);
        assert!(a.unclosed_opener[0]);
        assert!(a.unclosed_opener[1]);
    }

    #[test]
    fn end_matches_nearest_opener_of_same_kind() {
        let commands = vec![
            loop_cmd(),
            if_cmd(),
            MacroCommand::EndIf,
            MacroCommand::EndLoop,
        ];
        let a = analyze_blocks(&commands);
        assert_eq!(a.open_ifs, 0);
        assert_eq!(a.open_loops, 0);
        assert!(!a.orphan_end.iter().any(|&o| o));
        assert!(!a.unclosed_opener.iter().any(|&u| u));
    }

    #[test]
    fn else_does_not_close_if() {
        let commands = vec![if_cmd(), MacroCommand::Else];
        let a = analyze_blocks(&commands);
        assert_eq!(a.open_ifs, 1);
        assert!(!a.orphan_end[1]);
        assert!(a.unclosed_opener[0]);
    }

    #[test]
    fn fold_end_tracks_matching_closer() {
        let commands = vec![
            if_cmd(),
            MacroCommand::Else,
            MacroCommand::EndIf,
            loop_cmd(),
            MacroCommand::EndLoop,
        ];
        let a = analyze_blocks(&commands);
        assert_eq!(a.fold_end[0], Some(2));
        assert_eq!(a.fold_end[3], Some(4));
        assert_eq!(a.fold_end[1], None);
    }

    #[test]
    fn fold_end_of_nested_blocks() {
        let commands = vec![
            loop_cmd(),
            if_cmd(),
            MacroCommand::EndIf,
            MacroCommand::EndLoop,
        ];
        let a = analyze_blocks(&commands);
        assert_eq!(a.fold_end[0], Some(3));
        assert_eq!(a.fold_end[1], Some(2));
    }

    #[test]
    fn fold_end_of_unclosed_opener_reaches_last_row() {
        let commands = vec![if_cmd(), loop_cmd(), MacroCommand::EndLoop];
        let a = analyze_blocks(&commands);
        assert_eq!(a.fold_end[0], Some(2));
        assert_eq!(a.fold_end[1], Some(2));
    }
}
