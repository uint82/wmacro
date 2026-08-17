use log::error;
use std::collections::HashMap;
use wmacro_core_types::{CompareOp, Operand, Value};

use crate::macro_engine::player::expr::eval_expression;
use crate::macro_engine::player::frame::ExecFrame;
use crate::macro_engine::player::models::FlowControl;

/// resolves an operand to its current value; missing variables read as 0 and warn.
pub(super) fn resolve_value(op: &Operand, variables: &HashMap<String, Value>) -> Value {
    match op {
        Operand::Literal(value) => value.clone(),
        Operand::Var(name) => match variables.get(name) {
            Some(value) => value.clone(),
            None => {
                log::warn!("variable '{}' is not set, using 0", name);
                Value::Number(0)
            }
        },
    }
}

/// resolves an operand to a number; non-numeric text reads as 0, decimals truncate toward zero.
pub(super) fn resolve_num(op: &Operand, variables: &HashMap<String, Value>) -> i64 {
    match resolve_value(op, variables) {
        Value::Number(n) => n,
        Value::Float(f) => f.trunc() as i64,
        Value::Text(s) => s.trim().parse().unwrap_or_else(|_| {
            log::warn!("'{}' is not a number, using 0", s);
            0
        }),
    }
}

pub(super) fn execute_set_variable(
    target: &str,
    value: &Operand,
    variables: &mut HashMap<String, Value>,
) {
    variables.insert(target.to_string(), resolve_value(value, variables));
}

pub(super) fn execute_calculate(
    target: &str,
    expression: &str,
    variables: &mut HashMap<String, Value>,
) {
    match eval_expression(expression, variables) {
        Ok(value) => {
            variables.insert(target.to_string(), value);
        }
        Err(e) => {
            error!("Calculate: {}", e);
        }
    }
}

pub(super) fn execute_if_compare(
    left: &Operand,
    op: CompareOp,
    right: &Operand,
    variables: &HashMap<String, Value>,
    frame: &mut ExecFrame,
) -> FlowControl {
    let a = resolve_value(left, variables);
    let b = resolve_value(right, variables);

    if !compare_values(&a, op, &b) {
        frame.skip_to_else_or_endif();
    }
    FlowControl::Continue
}

/// `==`/`!=`/`contains` compare text forms; the ordering ops compare
/// numerically when both sides parse as numbers, lexicographically otherwise.
pub(super) fn compare_values(left: &Value, op: CompareOp, right: &Value) -> bool {
    match op {
        CompareOp::Eq => left.as_text() == right.as_text(),
        CompareOp::Ne => left.as_text() != right.as_text(),
        CompareOp::Contains => left.as_text().contains(&right.as_text()),
        CompareOp::Lt | CompareOp::Le | CompareOp::Gt | CompareOp::Ge => {
            match (left.as_f64_opt(), right.as_f64_opt()) {
                (Some(a), Some(b)) => match a.partial_cmp(&b) {
                    Some(ord) => ordering(ord, op),
                    None => false, // NaN compares false against everything
                },
                _ => ordering(left.as_text().cmp(&right.as_text()), op),
            }
        }
    }
}

fn ordering(a: std::cmp::Ordering, op: CompareOp) -> bool {
    match op {
        CompareOp::Lt => a.is_lt(),
        CompareOp::Le => a.is_le(),
        CompareOp::Gt => a.is_gt(),
        CompareOp::Ge => a.is_ge(),
        _ => unreachable!("ordering op expected"),
    }
}

/// replaces `$name` tokens with the variable's value; `$$` escapes to a
/// literal `$`, unknown names are left untouched.
pub(super) fn interpolate_variables(text: &str, variables: &HashMap<String, Value>) -> String {
    if variables.is_empty() || !text.contains('$') {
        return text.to_string();
    }

    let mut result = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            result.push(c);
            continue;
        }
        if chars.peek() == Some(&'$') {
            chars.next();
            result.push('$');
            continue;
        }
        let mut name = String::new();
        while let Some(&d) = chars.peek() {
            if d.is_alphanumeric() || d == '_' {
                name.push(d);
                chars.next();
            } else {
                break;
            }
        }
        match (name.is_empty(), variables.get(&name)) {
            (true, _) => result.push('$'),
            (false, Some(value)) => result.push_str(&value.as_text()),
            (false, None) => {
                result.push('$');
                result.push_str(&name);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use wmacro_core_types::MacroCommand;

    fn num(n: i64) -> Value {
        Value::Number(n)
    }

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    #[test]
    fn compare_eq_ne_on_text_forms() {
        assert!(compare_values(&num(5), CompareOp::Eq, &num(5)));
        assert!(compare_values(&num(5), CompareOp::Eq, &text("5")));
        assert!(compare_values(
            &text("hello"),
            CompareOp::Eq,
            &text("hello")
        ));
        assert!(!compare_values(
            &text("hello"),
            CompareOp::Eq,
            &text("world")
        ));
        assert!(compare_values(
            &text("hello"),
            CompareOp::Ne,
            &text("world")
        ));
    }

    #[test]
    fn ordering_is_numeric_when_both_sides_are_numbers() {
        assert!(compare_values(&num(2), CompareOp::Lt, &num(10)));
        assert!(compare_values(&text("2"), CompareOp::Lt, &num(10)));
        assert!(compare_values(&num(10), CompareOp::Gt, &text("2")));
        assert!(compare_values(&num(5), CompareOp::Le, &num(5)));
    }

    #[test]
    fn ordering_is_lexicographic_when_text_is_involved() {
        assert!(compare_values(&text("abc"), CompareOp::Lt, &text("abd")));
        assert!(!compare_values(&text("10"), CompareOp::Lt, &text("2")));
        assert!(compare_values(&text("nope"), CompareOp::Gt, &num(2)));
    }

    #[test]
    fn contains_checks_text_form() {
        assert!(compare_values(
            &text("Loading... done"),
            CompareOp::Contains,
            &text("done")
        ));
        assert!(!compare_values(
            &text("done"),
            CompareOp::Contains,
            &text("Loading")
        ));
        assert!(compare_values(&num(12345), CompareOp::Contains, &num(234)));
    }

    #[test]
    fn interpolate_numbers_and_text() {
        let mut vars = HashMap::new();
        vars.insert("n".to_string(), num(42));
        vars.insert("name".to_string(), text("wmacro"));
        assert_eq!(
            interpolate_variables("n=$n name=$name", &vars),
            "n=42 name=wmacro"
        );
        assert_eq!(interpolate_variables("no vars here", &vars), "no vars here");
    }

    #[test]
    fn interpolate_escape_and_unknown_names() {
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), num(1));
        assert_eq!(interpolate_variables("$$a", &vars), "$a");
        assert_eq!(
            interpolate_variables("$a $$ $missing", &vars),
            "1 $ $missing"
        );
        assert_eq!(interpolate_variables("$", &vars), "$");
    }

    #[test]
    fn resolve_num_coerces_text() {
        let vars = HashMap::new();
        let number = Operand::Literal(Value::Number(5));
        let text_num = Operand::Literal(Value::Text("500".to_string()));
        assert_eq!(resolve_num(&number, &vars), 5);
        assert_eq!(resolve_num(&text_num, &vars), 500);
    }

    #[test]
    fn contains_detects_substring_through_full_script_path() {
        use crate::macro_engine::script::{deserialize, serialize};

        let mut macro_def = wmacro_core_types::Macro::new("repro");
        macro_def.commands = vec![
            MacroCommand::SetVariable {
                target: "var".into(),
                value: Operand::Literal(Value::Text("lol lol point".into())),
            },
            MacroCommand::IfCompare {
                left: Operand::Var("var".into()),
                op: CompareOp::Contains,
                right: Operand::Literal(Value::Text("point".into())),
            },
            MacroCommand::EndIf,
        ];
        let script = serialize(&macro_def);
        let parsed = deserialize(&script).expect("deserialize");

        let mut vars: HashMap<String, Value> = HashMap::new();
        match &parsed.commands[0] {
            MacroCommand::SetVariable { target, value } => {
                execute_set_variable(target, value, &mut vars)
            }
            other => panic!("expected SetVariable, got {:?}", other),
        }
        let mut frame = ExecFrame::new(parsed.commands);
        let idx = frame.idx;
        let cmd = frame.commands[1].clone();
        match &cmd {
            MacroCommand::IfCompare { left, op, right } => {
                let flow = execute_if_compare(left, *op, right, &vars, &mut frame);
                assert!(matches!(flow, FlowControl::Continue));
            }
            other => panic!("expected IfCompare, got {:?}", other),
        }
        assert_eq!(
            frame.idx, idx,
            "contains should fall through to EndIf without skipping"
        );
    }

    #[test]
    fn contains_accepts_single_quoted_literals() {
        use crate::macro_engine::script::deserialize;

        let script = "\
# wmacro script
version 6
name \"repro\"

SetVariable target=\"var\" value='lol lol point'
IfCompare left=$var op='contains' right='point'
EndIf
";
        let parsed = deserialize(script).expect("deserialize");
        assert_eq!(
            parsed.commands[0],
            MacroCommand::SetVariable {
                target: "var".into(),
                value: Operand::Literal(Value::Text("lol lol point".into())),
            }
        );
        assert_eq!(
            parsed.commands[1],
            MacroCommand::IfCompare {
                left: Operand::Var("var".into()),
                op: CompareOp::Contains,
                right: Operand::Literal(Value::Text("point".into())),
            }
        );

        let mut vars: HashMap<String, Value> = HashMap::new();
        match &parsed.commands[0] {
            MacroCommand::SetVariable { target, value } => {
                execute_set_variable(target, value, &mut vars)
            }
            other => panic!("expected SetVariable, got {:?}", other),
        }
        let mut frame = ExecFrame::new(parsed.commands);
        let idx = frame.idx;
        let cmd = frame.commands[1].clone();
        match &cmd {
            MacroCommand::IfCompare { left, op, right } => {
                let flow = execute_if_compare(left, *op, right, &vars, &mut frame);
                assert!(matches!(flow, FlowControl::Continue));
            }
            other => panic!("expected IfCompare, got {:?}", other),
        }
        assert_eq!(frame.idx, idx, "contains must match through the full path");
    }
}
