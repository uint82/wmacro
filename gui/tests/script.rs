use wmacro_core_types::{
    CompareOp, Coord, Macro, MacroButton, MacroCommand, MacroEvent, MousePosition, Operand, Value,
};
use wmacro_gui::macro_engine::script::{deserialize, serialize, strip_quotes};

fn sample_macro() -> Macro {
    let mut m = Macro::new("vars_test");
    m.commands = vec![
        MacroCommand::IfImageFound {
            target_image_path: "/tmp/target.png".into(),
            similarity_threshold: 0.85,
            move_cursor_if_found: false,
            trigger_if_not_found: false,
            region: Some((100, 200, 300, 400)),
            store_x: Some("hit_x".into()),
            store_y: Some("hit_y".into()),
        },
        MacroCommand::IfColorFound {
            region: Some((10, 20, 640, 480)),
            r: 255,
            g: 128,
            b: 0,
            tolerance: 15,
            min_width: 40,
            min_height: 10,
            move_cursor_if_found: false,
            store_x: Some("fx".into()),
            store_y: Some("fy".into()),
            store_w: Some("fw".into()),
            store_h: Some("fh".into()),
        },
        MacroCommand::SetVariable {
            target: "count".into(),
            value: Operand::Literal(Value::Number(0)),
        },
        MacroCommand::Calculate {
            target: "count".into(),
            expression: "$count + 1".into(),
        },
        MacroCommand::IfCompare {
            left: Operand::Var("hit_y".into()),
            op: CompareOp::Ge,
            right: Operand::Literal(Value::Number(500)),
        },
        MacroCommand::Action(MacroEvent::MouseMove {
            x: Coord::Var("hit_x".into()),
            y: Coord::Const(10),
        }),
        MacroCommand::Action(MacroEvent::Click {
            position: MousePosition::Absolute {
                x: Coord::Var("hit_x".into()),
                y: Coord::Var("hit_y".into()),
            },
            button: MacroButton::Left,
            jitter: 0,
            hold_time_ms: 30,
        }),
    ];
    m
}

// serialize then parse and compare every command; keeps the script format and its parser honest. TODO: add a proptest that round-trips randomly generated macros.
fn assert_round_trips(m: &Macro) {
    let script = serialize(m);
    let parsed = deserialize(&script).expect("deserialize should succeed");
    assert_eq!(parsed.version, wmacro_core_types::CURRENT_FORMAT_VERSION);
    assert_eq!(parsed.name, m.name);
    assert_eq!(parsed.commands.len(), m.commands.len());
    for (expected, actual) in m.commands.iter().zip(parsed.commands.iter()) {
        assert_eq!(expected, actual, "command mismatch after round trip");
    }
}

#[test]
fn round_trip_variable_commands() {
    assert_round_trips(&sample_macro());
}

#[test]
fn round_trip_string_operands() {
    let mut m = Macro::new("strings");
    m.commands = vec![
        MacroCommand::SetVariable {
            target: "name".into(),
            value: Operand::Literal(Value::Text("wmacro rocks".into())),
        },
        MacroCommand::SetVariable {
            target: "five".into(),
            value: Operand::Literal(Value::Text("5".into())),
        },
        MacroCommand::IfCompare {
            left: Operand::Var("name".into()),
            op: CompareOp::Contains,
            right: Operand::Literal(Value::Text("wmacro".into())),
        },
    ];
    assert_round_trips(&m);
}

#[test]
fn round_trip_escaped_quotes() {
    let mut m = Macro::new("quotes");
    m.commands = vec![
        MacroCommand::SetVariable {
            target: "greeting".into(),
            value: Operand::Literal(Value::Text("He said \"hi\" to me".into())),
        },
        MacroCommand::IfCompare {
            left: Operand::Var("greeting".into()),
            op: CompareOp::Contains,
            right: Operand::Literal(Value::Text("said \"hi\"".into())),
        },
        MacroCommand::SetVariable {
            target: "apostrophe".into(),
            value: Operand::Literal(Value::Text("don't worry".into())),
        },
        MacroCommand::TypeText("She wrote \"wmacro\" on the board".into()),
    ];
    assert_round_trips(&m);
}

#[test]
fn round_trip_clipboard_commands() {
    let mut m = Macro::new("clipboard");
    m.commands = vec![
        MacroCommand::SetClipboard {
            text: Operand::Literal(Value::Text("paste me".into())),
        },
        MacroCommand::SetClipboard {
            text: Operand::Var("generated".into()),
        },
        MacroCommand::GetClipboard {
            target: "clip".into(),
        },
    ];
    assert_round_trips(&m);
}

#[test]
fn round_trip_comment_commands() {
    let mut m = Macro::new("notes");
    m.commands = vec![
        MacroCommand::Comment("wait for the window to open".into()),
        MacroCommand::Action(MacroEvent::Delay(1500)),
        MacroCommand::Comment("He said \"hi\" and left".into()),
    ];
    assert_round_trips(&m);
}

#[test]
fn recorded_delays_keep_microsecond_precision() {
    let mut m = Macro::new("precise");
    m.commands = vec![
        MacroCommand::Action(MacroEvent::Delay(1500)),
        MacroCommand::Action(MacroEvent::Delay(1_234_567)),
    ];
    assert_round_trips(&m);
}

#[test]
fn legacy_ms_delays_still_parse() {
    let script = "\
# wmacro script
version 8
name \"legacy\"

Delay ms=500
";
    let parsed = deserialize(script).unwrap();
    assert_eq!(
        parsed.commands,
        vec![MacroCommand::Action(MacroEvent::Delay(500_000))]
    );
}

#[test]
fn strip_quotes_handles_doubling() {
    assert_eq!(strip_quotes("\"hello\"").as_deref(), Some("hello"));
    assert_eq!(strip_quotes("'point'").as_deref(), Some("point"));
    assert_eq!(
        strip_quotes("\"He said \"\"hi\"\"\"").as_deref(),
        Some("He said \"hi\"")
    );
    assert_eq!(strip_quotes("'don''t'").as_deref(), Some("don't"));
    assert_eq!(strip_quotes("'a''b''c'").as_deref(), Some("a'b'c"));
    assert_eq!(strip_quotes("\"\"\"\"").as_deref(), Some("\""));
    assert_eq!(strip_quotes("\"\"").as_deref(), Some(""));
    assert_eq!(strip_quotes("unquoted"), None);
    assert_eq!(strip_quotes("\""), None);
    assert_eq!(strip_quotes("'"), None);
    assert_eq!(strip_quotes(""), None);
}

#[test]
fn parses_variable_script_lines() {
    let script = "\
# wmacro script
version 6
name \"vars\"

SetVariable target=\"n\" value=5
SetVariable target=\"msg\" value=\"hello world\"
Calculate target=\"n\" expr=\"$n + 1\"
Calculate target=\"sum\" expr=\"($a * 2) / 4\"
IfCompare left=$n op=\">=\" right=5
IfCompare left=$msg op=\"contains\" right=\"hello\"
MouseMove x=$n y=3
IfImageFound target=\"t.png\" tol=0.9 move=true not_found=false store_x=\"hit_x\" store_y=\"hit_y\"
";
    let parsed = deserialize(script).unwrap();
    assert_eq!(
        parsed.commands[0],
        MacroCommand::SetVariable {
            target: "n".into(),
            value: Operand::Literal(Value::Number(5)),
        }
    );
    assert_eq!(
        parsed.commands[1],
        MacroCommand::SetVariable {
            target: "msg".into(),
            value: Operand::Literal(Value::Text("hello world".into())),
        }
    );
    assert_eq!(
        parsed.commands[2],
        MacroCommand::Calculate {
            target: "n".into(),
            expression: "$n + 1".into(),
        }
    );
    assert_eq!(
        parsed.commands[3],
        MacroCommand::Calculate {
            target: "sum".into(),
            expression: "($a * 2) / 4".into(),
        }
    );
    assert_eq!(
        parsed.commands[4],
        MacroCommand::IfCompare {
            left: Operand::Var("n".into()),
            op: CompareOp::Ge,
            right: Operand::Literal(Value::Number(5)),
        }
    );
    assert_eq!(
        parsed.commands[5],
        MacroCommand::IfCompare {
            left: Operand::Var("msg".into()),
            op: CompareOp::Contains,
            right: Operand::Literal(Value::Text("hello".into())),
        }
    );
    assert_eq!(
        parsed.commands[6],
        MacroCommand::Action(MacroEvent::MouseMove {
            x: Coord::Var("n".into()),
            y: Coord::Const(3),
        })
    );
    match &parsed.commands[7] {
        MacroCommand::IfImageFound {
            store_x, store_y, ..
        } => {
            assert_eq!(store_x.as_deref(), Some("hit_x"));
            assert_eq!(store_y.as_deref(), Some("hit_y"));
        }
        other => panic!("expected IfImageFound, got {:?}", other),
    }
}

#[test]
fn legacy_calculate_lines_still_load() {
    let script = "\
# wmacro script
version 4
name \"legacy\"

Calculate target=\"n\" left=$n op=+ right=1
Calculate target=\"m\" left=2 op=* right=3
";
    let parsed = deserialize(script).unwrap();
    assert_eq!(
        parsed.commands[0],
        MacroCommand::Calculate {
            target: "n".into(),
            expression: "$n + 1".into(),
        }
    );
    assert_eq!(
        parsed.commands[1],
        MacroCommand::Calculate {
            target: "m".into(),
            expression: "2 * 3".into(),
        }
    );
}
