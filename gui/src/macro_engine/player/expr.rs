//! evaluates a `Calculate` expression: decimal arithmetic, text concatenation, comparisons, a ternary, function calls, string literals and `$variable` references.

use std::collections::HashMap;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use wmacro_core_types::{CompareOp, Value};

use crate::macro_engine::humanize::rng::{Pcg, time_seed};
use crate::macro_engine::player::variables::compare_values;

/// shared rng for `random()`; seeded once from the clock.
static EXPR_RNG: Lazy<Mutex<Pcg>> = Lazy::new(|| Mutex::new(Pcg::new(time_seed())));

/// evaluates `expr` against `variables`, returning the resulting value.
pub(crate) fn eval_expression(
    expr: &str,
    variables: &HashMap<String, Value>,
) -> Result<Value, String> {
    let mut tokens = tokenize(expr).map_err(|e| format!("{e} in '{expr}'"))?;
    if tokens.is_empty() {
        return Err(format!("empty expression in '{expr}'"));
    }
    let value = parse_ternary(&mut tokens, variables)?;
    if let Some(tok) = tokens.first() {
        return Err(format!("unexpected '{}' in '{expr}'", tok.text()));
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    String(String),
    Var(String),
    Ident(String),
    Op(BinOp),
    Question,
    Colon,
    LParen,
    RParen,
    Comma,
}

impl Token {
    fn text(&self) -> String {
        match self {
            Token::Number(n) => n.to_string(),
            Token::String(s) => format!("\"{s}\""),
            Token::Var(name) => format!("${name}"),
            Token::Ident(name) => name.clone(),
            Token::Op(op) => op.symbol().to_string(),
            Token::Question => "?".into(),
            Token::Colon => ":".into(),
            Token::LParen => "(".into(),
            Token::RParen => ")".into(),
            Token::Comma => ",".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Concat => ".",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        }
    }

    fn to_compare_op(self) -> Option<CompareOp> {
        match self {
            BinOp::Eq => Some(CompareOp::Eq),
            BinOp::Ne => Some(CompareOp::Ne),
            BinOp::Lt => Some(CompareOp::Lt),
            BinOp::Le => Some(CompareOp::Le),
            BinOp::Gt => Some(CompareOp::Gt),
            BinOp::Ge => Some(CompareOp::Ge),
            _ => None,
        }
    }
}

fn single_token(c: char) -> Option<Token> {
    Some(match c {
        '+' => Token::Op(BinOp::Add),
        '-' => Token::Op(BinOp::Sub),
        '*' => Token::Op(BinOp::Mul),
        '/' => Token::Op(BinOp::Div),
        '%' => Token::Op(BinOp::Mod),
        '.' => Token::Op(BinOp::Concat),
        '(' => Token::LParen,
        ')' => Token::RParen,
        ',' => Token::Comma,
        '?' => Token::Question,
        ':' => Token::Colon,
        _ => return None,
    })
}

fn tokenize(src: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = src.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if let Some(tok) = single_token(c) {
            chars.next();
            tokens.push(tok);
            continue;
        }
        match c {
            '0'..='9' => tokens.push(parse_number(&mut chars)?),
            '"' | '\'' => tokens.push(Token::String(parse_string(&mut chars)?)),
            '$' => {
                chars.next();
                let name = take_name(&mut chars);
                if name.is_empty() {
                    return Err("expected a variable name after '$'".into());
                }
                tokens.push(Token::Var(name));
            }
            'a'..='z' | 'A'..='Z' | '_' => tokens.push(Token::Ident(take_name(&mut chars))),
            '=' => {
                chars.next();
                if chars.next_if(|&c| c == '=').is_some() {
                    tokens.push(Token::Op(BinOp::Eq));
                } else {
                    return Err("expected '=' after '=' (use '==' to compare)".into());
                }
            }
            '!' => {
                chars.next();
                if chars.next_if(|&c| c == '=').is_some() {
                    tokens.push(Token::Op(BinOp::Ne));
                } else {
                    return Err("expected '=' after '!' (use '!=' to compare)".into());
                }
            }
            '<' => {
                chars.next();
                let op = if chars.next_if(|&c| c == '=').is_some() {
                    BinOp::Le
                } else {
                    BinOp::Lt
                };
                tokens.push(Token::Op(op));
            }
            '>' => {
                chars.next();
                let op = if chars.next_if(|&c| c == '=').is_some() {
                    BinOp::Ge
                } else {
                    BinOp::Gt
                };
                tokens.push(Token::Op(op));
            }
            other => return Err(format!("unexpected character '{other}'")),
        }
    }
    Ok(tokens)
}

fn parse_number(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<Token, String> {
    let mut num = String::new();
    while let Some(&d) = chars.peek() {
        if d.is_ascii_digit() {
            num.push(d);
            chars.next();
        } else {
            break;
        }
    }
    if chars.peek() == Some(&'.') {
        num.push('.');
        chars.next();
        while let Some(&d) = chars.peek() {
            if d.is_ascii_digit() {
                num.push(d);
                chars.next();
            } else {
                break;
            }
        }
    }
    let value = num
        .parse::<f64>()
        .map_err(|_| format!("number '{num}' is out of range"))?;
    Ok(Token::Number(value))
}

fn parse_string(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String, String> {
    let quote = chars.next().unwrap();
    let mut s = String::new();
    for c in chars.by_ref() {
        if c == quote {
            return Ok(s);
        }
        s.push(c);
    }
    Err("unterminated string literal".into())
}

fn take_name(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut name = String::new();
    while let Some(&d) = chars.peek() {
        if d.is_alphanumeric() || d == '_' {
            name.push(d);
            chars.next();
        } else {
            break;
        }
    }
    name
}

fn take(tokens: &mut Vec<Token>, tok: Token) -> bool {
    if tokens.first() == Some(&tok) {
        tokens.remove(0);
        true
    } else {
        false
    }
}

fn take_arith(tokens: &mut Vec<Token>, allowed: &[BinOp]) -> Option<BinOp> {
    let op = match tokens.first() {
        Some(Token::Op(op)) if allowed.contains(op) => *op,
        _ => return None,
    };
    tokens.remove(0);
    Some(op)
}

fn take_compare(tokens: &mut Vec<Token>) -> Option<CompareOp> {
    let op = match tokens.first() {
        Some(Token::Op(op)) => op.to_compare_op(),
        _ => None,
    }?;
    tokens.remove(0);
    Some(op)
}

/// integral in-range values collapse to `Number`, everything else stays `Float`; non-finite results are errors.
fn collapse(value: f64) -> Result<Value, String> {
    if !value.is_finite() {
        return Err("result is out of range".into());
    }
    if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
        Ok(Value::Number(value as i64))
    } else {
        Ok(Value::Float(value))
    }
}

/// ternary conditions: numbers are true when non-zero, text when non-empty.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Number(n) => *n != 0,
        Value::Float(f) => *f != 0.0,
        Value::Text(s) => !s.is_empty(),
    }
}

/// applies an arithmetic or concat operator; text coerces to a number (unparseable reads as 0), `.` joins.
fn apply_arith(op: BinOp, left: Value, right: Value) -> Result<Value, String> {
    if op == BinOp::Concat {
        return Ok(Value::Text(format!(
            "{}{}",
            left.as_text(),
            right.as_text()
        )));
    }
    let (a, b) = (left.as_f64(), right.as_f64());
    let result = match op {
        BinOp::Div => {
            if b == 0.0 {
                return Err("division by zero".into());
            }
            a / b
        }
        BinOp::Mod => {
            if b == 0.0 {
                return Err("modulo by zero".into());
            }
            a % b
        }
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        _ => unreachable!("non-arithmetic op reached apply_arith"),
    };
    collapse(result)
}

/// ternary := comparison [ "?" ternary ":" ternary ] (right-associative)
fn parse_ternary(
    tokens: &mut Vec<Token>,
    variables: &HashMap<String, Value>,
) -> Result<Value, String> {
    let cond = parse_comparison(tokens, variables)?;
    if !take(tokens, Token::Question) {
        return Ok(cond);
    }
    let then_branch = parse_ternary(tokens, variables)?;
    if !take(tokens, Token::Colon) {
        return Err("expected ':' for ternary".into());
    }
    let else_branch = parse_ternary(tokens, variables)?;
    Ok(if truthy(&cond) {
        then_branch
    } else {
        else_branch
    })
}

/// comparison := additive [ (== | != | < | <= | > | >=) additive ]
fn parse_comparison(
    tokens: &mut Vec<Token>,
    variables: &HashMap<String, Value>,
) -> Result<Value, String> {
    let left = parse_additive(tokens, variables)?;
    let Some(op) = take_compare(tokens) else {
        return Ok(left);
    };
    let right = parse_additive(tokens, variables)?;
    Ok(Value::Number(i64::from(compare_values(&left, op, &right))))
}

/// additive := multiplicative { ("+" | "-" | ".") multiplicative }
fn parse_additive(
    tokens: &mut Vec<Token>,
    variables: &HashMap<String, Value>,
) -> Result<Value, String> {
    let mut left = parse_multiplicative(tokens, variables)?;
    while let Some(op) = take_arith(tokens, &[BinOp::Add, BinOp::Sub, BinOp::Concat]) {
        let right = parse_multiplicative(tokens, variables)?;
        left = apply_arith(op, left, right)?;
    }
    Ok(left)
}

/// multiplicative := unary { ("*" | "/" | "%") unary }
fn parse_multiplicative(
    tokens: &mut Vec<Token>,
    variables: &HashMap<String, Value>,
) -> Result<Value, String> {
    let mut left = parse_unary(tokens, variables)?;
    while let Some(op) = take_arith(tokens, &[BinOp::Mul, BinOp::Div, BinOp::Mod]) {
        let right = parse_unary(tokens, variables)?;
        left = apply_arith(op, left, right)?;
    }
    Ok(left)
}

/// unary := ("-" | "+") unary | primary
fn parse_unary(
    tokens: &mut Vec<Token>,
    variables: &HashMap<String, Value>,
) -> Result<Value, String> {
    let negate = match tokens.first() {
        Some(Token::Op(BinOp::Add)) => false,
        Some(Token::Op(BinOp::Sub)) => true,
        _ => return parse_primary(tokens, variables),
    };
    tokens.remove(0);
    let value = parse_unary(tokens, variables)?;
    if negate {
        collapse(-value.as_f64())
    } else {
        Ok(value)
    }
}

/// primary := number | string | "$" var | ident "(" args ")" | "(" ternary ")"
fn parse_primary(
    tokens: &mut Vec<Token>,
    variables: &HashMap<String, Value>,
) -> Result<Value, String> {
    let Some(tok) = tokens.first().cloned() else {
        return Err("unexpected end of expression".into());
    };
    match tok {
        Token::Number(n) => {
            tokens.remove(0);
            collapse(n)
        }
        Token::String(s) => {
            tokens.remove(0);
            Ok(Value::Text(s))
        }
        Token::Var(name) => {
            tokens.remove(0);
            match variables.get(&name) {
                Some(value) => Ok(value.clone()),
                None => {
                    log::warn!("variable '{}' is not set, using 0", name);
                    Ok(Value::Number(0))
                }
            }
        }
        Token::Ident(name) => {
            tokens.remove(0);
            parse_call(name, tokens, variables)
        }
        Token::LParen => {
            tokens.remove(0);
            let value = parse_ternary(tokens, variables)?;
            if !take(tokens, Token::RParen) {
                return Err("missing closing parenthesis".into());
            }
            Ok(value)
        }
        Token::Op(op) => Err(format!("unexpected operator '{}'", op.symbol())),
        Token::RParen => Err("unexpected ')'".into()),
        Token::Question | Token::Colon | Token::Comma => {
            Err(format!("unexpected '{}'", tok.text()))
        }
    }
}

/// ident "(" [ternary { "," ternary }] ")"
fn parse_call(
    name: String,
    tokens: &mut Vec<Token>,
    variables: &HashMap<String, Value>,
) -> Result<Value, String> {
    if !take(tokens, Token::LParen) {
        return Err(format!("expected '(' after '{name}'"));
    }
    let mut args = Vec::new();
    if !matches!(tokens.first(), Some(Token::RParen)) {
        args.push(parse_ternary(tokens, variables)?);
        while take(tokens, Token::Comma) {
            args.push(parse_ternary(tokens, variables)?);
        }
    }
    if !take(tokens, Token::RParen) {
        return Err(format!("missing ')' for '{name}'"));
    }
    apply_function(&name, args)
}

fn apply_function(name: &str, args: Vec<Value>) -> Result<Value, String> {
    match name {
        "abs" => unary_num(name, &args, |x| Ok(x.abs())),
        "floor" => unary_num(name, &args, |x| Ok(x.floor())),
        "ceil" => unary_num(name, &args, |x| Ok(x.ceil())),
        "sqrt" => unary_num(name, &args, |x| {
            if x < 0.0 {
                return Err("sqrt: negative number".into());
            }
            Ok(x.sqrt())
        }),
        "round" => call_round(&args),
        "min" => binary_num(name, &args, |a, b| a.min(b)),
        "max" => binary_num(name, &args, |a, b| a.max(b)),
        "random" => call_random(&args),
        _ => Err(format!("unknown function '{name}'")),
    }
}

fn unary_num(
    name: &str,
    args: &[Value],
    f: impl Fn(f64) -> Result<f64, String>,
) -> Result<Value, String> {
    let x = single_arg(name, args)?.as_f64();
    collapse(f(x)?)
}

fn binary_num(name: &str, args: &[Value], f: impl Fn(f64, f64) -> f64) -> Result<Value, String> {
    let (a, b) = two_args(name, args)?;
    collapse(f(a.as_f64(), b.as_f64()))
}

fn single_arg<'a>(name: &str, args: &'a [Value]) -> Result<&'a Value, String> {
    match args {
        [x] => Ok(x),
        _ => Err(format!("{name}() expects 1 argument, got {}", args.len())),
    }
}

fn two_args<'a>(name: &str, args: &'a [Value]) -> Result<(&'a Value, &'a Value), String> {
    match args {
        [a, b] => Ok((a, b)),
        _ => Err(format!("{name}() expects 2 arguments, got {}", args.len())),
    }
}

/// `round(x)` to the nearest integer; `round(x, n)` to `n` decimal places (negative `n` rounds to tens/hundreds).
fn call_round(args: &[Value]) -> Result<Value, String> {
    match args {
        [x] => collapse(x.as_f64().round()),
        [x, n] => {
            let factor = 10f64.powi(n.as_f64().trunc() as i32);
            collapse((x.as_f64() * factor).round() / factor)
        }
        _ => Err(format!(
            "round() expects 1 or 2 arguments, got {}",
            args.len()
        )),
    }
}

/// `random(min, max)` returns a uniform integer in `[min, max]` (inclusive).
fn call_random(args: &[Value]) -> Result<Value, String> {
    let (min, max) = two_args("random", args)?;
    let (min, max) = (min.as_f64().trunc() as i64, max.as_f64().trunc() as i64);
    if min > max {
        return Err(format!("random: min ({min}) is greater than max ({max})"));
    }
    let mut rng = EXPR_RNG.lock().unwrap();
    Ok(Value::Number(rng.next_range_i64(min, max)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn num(n: i64) -> Value {
        Value::Number(n)
    }

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    fn eval(expr: &str, v: &HashMap<String, Value>) -> Value {
        eval_expression(expr, v).unwrap()
    }

    fn eval_err(expr: &str) -> String {
        eval_expression(expr, &HashMap::new()).unwrap_err()
    }

    #[test]
    fn plain_arithmetic() {
        let v = HashMap::new();
        assert_eq!(eval("1 + 2", &v), num(3));
        assert_eq!(eval("2 * 3 + 4", &v), num(10));
        assert_eq!(eval("2 + 3 * 4", &v), num(14));
        assert_eq!(eval("10 - 2 * 3", &v), num(4));
        assert_eq!(eval("7 % 3", &v), num(1));
        assert_eq!(eval("8 / 2 / 2", &v), num(2));
    }

    #[test]
    fn floats_and_division() {
        let v = HashMap::new();
        assert_eq!(eval("5 / 2", &v), Value::Float(2.5));
        assert_eq!(eval("10.0 / 4", &v), Value::Float(2.5));
        assert_eq!(eval("0.5 * 4", &v), num(2));
        assert_eq!(eval("1.5 + 1.5", &v), num(3));
        assert_eq!(eval("10 % 4.5", &v), num(1));
    }

    #[test]
    fn parentheses_and_unary() {
        let v = HashMap::new();
        assert_eq!(eval("(2 + 3) * 4", &v), num(20));
        assert_eq!(eval("-(3 + 2)", &v), num(-5));
        assert_eq!(eval("2 * -(3)", &v), num(-6));
        assert_eq!(eval("+5", &v), num(5));
    }

    #[test]
    fn variables() {
        let v = vars(&[("a", num(5)), ("b", num(2))]);
        assert_eq!(eval("$a + $b", &v), num(7));
        assert_eq!(eval("$a * $b + 1", &v), num(11));
        assert_eq!(eval("($a - 1) / $b", &v), num(2));
        assert_eq!(eval("$b", &v), num(2));
    }

    #[test]
    fn missing_variable_reads_zero() {
        let v = vars(&[("a", num(5))]);
        assert_eq!(eval("$a + $nope", &v), num(5));
    }

    #[test]
    fn string_literals_and_concat() {
        let v = vars(&[("name", text("wmacro"))]);
        assert_eq!(
            eval("\"hello\" . \" \" . \"world\"", &v),
            text("hello world")
        );
        assert_eq!(eval("'a' . \"b\"", &v), text("ab"));
        assert_eq!(eval("$name . \"!\"", &v), text("wmacro!"));
        assert_eq!(eval("\"n=\" . 42", &v), text("n=42"));
        assert_eq!(eval("5 . 5", &v), text("55"));
        assert_eq!(eval("$name", &v), text("wmacro"));
    }

    #[test]
    fn arithmetic_coerces_text() {
        let v = vars(&[("t", text("10"))]);
        assert_eq!(eval("\"10\" + 5", &v), num(15));
        assert_eq!(eval("\"5\" * 2", &v), num(10));
        assert_eq!(eval("$t / 4", &v), Value::Float(2.5));
        assert_eq!(eval("\"abc\" + 1", &v), num(1));
    }

    #[test]
    fn comparisons_return_1_or_0() {
        let v = vars(&[("a", num(5))]);
        assert_eq!(eval("1 < 2", &v), num(1));
        assert_eq!(eval("2 <= 1", &v), num(0));
        assert_eq!(eval("3 > 3", &v), num(0));
        assert_eq!(eval("3 >= 3", &v), num(1));
        assert_eq!(eval("\"abc\" == \"abc\"", &v), num(1));
        assert_eq!(eval("\"abc\" != \"abd\"", &v), num(1));
        assert_eq!(eval("$a == 5", &v), num(1));
        assert_eq!(eval("2.5 > 2", &v), num(1));
    }

    #[test]
    fn ternary_picks_branch() {
        let v = HashMap::new();
        assert_eq!(eval("5 > 3 ? 100 : 200", &v), num(100));
        assert_eq!(eval("5 < 3 ? 100 : 200", &v), num(200));
        assert_eq!(eval("0 ? 1 : 2", &v), num(2));
        assert_eq!(eval("\"\" ? 1 : 2", &v), num(2));
        assert_eq!(eval("\"x\" ? 1 : 2", &v), num(1));
        assert_eq!(eval("1 ? 2 ? 3 : 4 : 5", &v), num(3));
        assert_eq!(eval("0 ? 1 : 0 ? 2 : 3", &v), num(3));
    }

    #[test]
    fn math_functions() {
        let v = HashMap::new();
        assert_eq!(eval("abs(-3)", &v), num(3));
        assert_eq!(eval("abs(3)", &v), num(3));
        assert_eq!(eval("floor(2.7)", &v), num(2));
        assert_eq!(eval("ceil(2.1)", &v), num(3));
        assert_eq!(eval("ceil(-2.1)", &v), num(-2));
        assert_eq!(eval("round(2.5)", &v), num(3));
        assert_eq!(eval("round(2.4)", &v), num(2));
        assert_eq!(eval("sqrt(9)", &v), num(3));
        assert_eq!(eval("sqrt(0)", &v), num(0));
        assert_eq!(eval("min(3, 7)", &v), num(3));
        assert_eq!(eval("max(3, 7)", &v), num(7));
        assert_eq!(eval("min(-1.5, -2.5)", &v), Value::Float(-2.5));
    }

    #[test]
    fn round_with_digits() {
        let v = HashMap::new();
        let got = eval("round(123.456, 2)", &v).as_f64();
        assert!((got - 123.46).abs() < 1e-9);
        let got = eval("round(123.456, -1)", &v).as_f64();
        assert!((got - 120.0).abs() < 1e-9);
        assert_eq!(eval("round(1.5, 0)", &v), num(2));
    }

    #[test]
    fn random_stays_within_bounds() {
        let v = HashMap::new();
        assert_eq!(eval("random(7, 7)", &v), num(7));
        for _ in 0..200 {
            let n = eval("random(1, 6)", &v).as_i64();
            assert!((1..=6).contains(&n), "draw {n} outside 1..=6");
        }
        let draws: Vec<i64> = (0..100)
            .map(|_| eval("random(1, 10)", &v).as_i64())
            .collect();
        assert!(draws.iter().any(|&n| n <= 5), "expected low draws");
        assert!(draws.iter().any(|&n| n >= 6), "expected high draws");
    }

    #[test]
    fn functions_on_variables() {
        let v = vars(&[("a", num(10)), ("b", num(3))]);
        assert_eq!(eval("abs($a - $b * 5)", &v), num(5));
        assert_eq!(eval("max($a, $b)", &v), num(10));
    }

    #[test]
    fn errors() {
        assert!(eval_expression("", &HashMap::new()).is_err());
        assert!(eval_expression("1 +", &HashMap::new()).is_err());
        assert!(eval_expression("1 2", &HashMap::new()).is_err());
        assert!(eval_expression("(1 + 2", &HashMap::new()).is_err());
        assert!(eval_expression("1 / 0", &HashMap::new()).is_err());
        assert!(eval_expression("1 % 0", &HashMap::new()).is_err());
        assert!(eval_expression("$", &HashMap::new()).is_err());
        assert!(eval_expression("1 @ 2", &HashMap::new()).is_err());
        assert!(eval_expression("\"unterminated", &HashMap::new()).is_err());
        assert!(eval_expression("1 ? 2", &HashMap::new()).is_err());
        assert!(eval_expression("1 = 2", &HashMap::new()).is_err());
        assert!(eval_expression("1 ! 2", &HashMap::new()).is_err());
        assert!(eval_expression("1 == 2 == 3", &HashMap::new()).is_err());
    }

    #[test]
    fn function_errors() {
        assert_eq!(eval_err("sqrt(-1)"), "sqrt: negative number");
        assert_eq!(
            eval_err("random(5, 1)"),
            "random: min (5) is greater than max (1)"
        );
        assert_eq!(eval_err("random(1)"), "random() expects 2 arguments, got 1");
        assert_eq!(eval_err("abs()"), "abs() expects 1 argument, got 0");
        assert_eq!(eval_err("min(1)"), "min() expects 2 arguments, got 1");
        assert_eq!(eval_err("foo(1)"), "unknown function 'foo'");
        assert_eq!(
            eval_err("round(1, 2, 3)"),
            "round() expects 1 or 2 arguments, got 3"
        );
    }

    #[test]
    fn overflow_is_reported() {
        let v = HashMap::new();
        let big = "9".repeat(309);
        assert_eq!(
            eval_expression(&format!("{big} * {big}"), &v).unwrap_err(),
            "result is out of range"
        );
        assert!(eval_expression("$missing", &v).is_ok());
    }
}
