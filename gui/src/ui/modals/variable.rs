use crate::state::SharedState;
use crate::ui::theme::ThemePalette;
use eframe::egui;
use wmacro_core_types::{Coord, MacroCommand};

/// variable names referenced by the current macro: set/calculate targets,
/// clipboard targets, and `IfImageFound`/`IfColorFound` stores.
pub fn available_variable_names(state: &SharedState) -> Vec<String> {
    let s = state.lock().unwrap_or_else(|e| {
        log::error!("State mutex poisoned: {e}");
        e.into_inner()
    });
    let Some(m) = s.macro_state.current_macro.as_ref() else {
        return Vec::new();
    };

    let mut names: Vec<String> = Vec::new();
    for cmd in &m.commands {
        match cmd {
            MacroCommand::SetVariable { target, .. }
            | MacroCommand::Calculate { target, .. }
            | MacroCommand::GetClipboard { target } => {
                if !names.contains(target) {
                    names.push(target.clone());
                }
            }
            MacroCommand::IfImageFound {
                store_x, store_y, ..
            } => {
                for name in [store_x, store_y].into_iter().flatten() {
                    if !names.contains(name) {
                        names.push(name.clone());
                    }
                }
            }
            MacroCommand::IfColorFound {
                store_x,
                store_y,
                store_w,
                store_h,
                ..
            } => {
                for name in [store_x, store_y, store_w, store_h].into_iter().flatten() {
                    if !names.contains(name) {
                        names.push(name.clone());
                    }
                }
            }
            _ => {}
        }
    }
    names.sort();
    names
}

/// parses a user-typed string into an `Operand`: `$name` → `Operand::Var`,
/// quoted or non-numeric text → `Text` literal, numbers → `Number` literal.
pub fn parse_var_or_num(text: &str) -> wmacro_core_types::Operand {
    use wmacro_core_types::{Operand, Value};
    let t = text.trim();
    if let Some(name) = t.strip_prefix('$') {
        Operand::Var(name.to_string())
    } else if let Some(inner) = crate::macro_engine::script::strip_quotes(t) {
        Operand::Literal(Value::Text(inner))
    } else {
        match t.parse::<i64>() {
            Ok(n) => Operand::Literal(Value::Number(n)),
            Err(_) => Operand::Literal(Value::Text(t.to_string())),
        }
    }
}

/// converts an `Operand` back to user-visible text; text literals are shown
/// quoted so they round-trip exactly.
pub fn format_var_operand(op: &wmacro_core_types::Operand) -> String {
    crate::macro_engine::script::format_operand(op)
}

/// parses a duration string (e.g. "1h 30m", "500ms", "500") into milliseconds;
/// None if invalid or contains unknown units.
pub fn parse_duration_string(text: &str) -> Option<u64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }

    // pure numbers without letters are treated as milliseconds.
    if t.chars()
        .all(|c| c.is_ascii_digit() || c.is_ascii_whitespace())
    {
        let stripped: String = t.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        return stripped.parse::<u64>().ok();
    }

    let mut total_ms = 0u64;
    let chars: Vec<char> = t.chars().filter(|c| !c.is_ascii_whitespace()).collect();

    let mut i = 0;
    while i < chars.len() {
        let mut num_str = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() {
            num_str.push(chars[i]);
            i += 1;
        }

        let mut unit_str = String::new();
        while i < chars.len() && chars[i].is_ascii_alphabetic() {
            unit_str.push(chars[i]);
            i += 1;
        }

        if num_str.is_empty() {
            return None;
        }

        let num = num_str.parse::<u64>().ok()?;

        match unit_str.to_lowercase().as_str() {
            "h" => total_ms += num * 3600 * 1000,
            "m" => total_ms += num * 60 * 1000,
            "s" => total_ms += num * 1000,
            "ms" | "" => total_ms += num,
            _ => return None,
        }
    }

    Some(total_ms)
}

/// formats milliseconds into a readable duration string (e.g. "1h 30m").
pub fn format_duration_string(ms: u64) -> String {
    if ms == 0 {
        return "0ms".to_string();
    }

    let hours = ms / (3600 * 1000);
    let rem = ms % (3600 * 1000);
    let minutes = rem / (60 * 1000);
    let rem2 = rem % (60 * 1000);
    let seconds = rem2 / 1000;
    let millis = rem2 % 1000;

    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if minutes > 0 {
        parts.push(format!("{}m", minutes));
    }
    if seconds > 0 {
        parts.push(format!("{}s", seconds));
    }
    if millis > 0 || parts.is_empty() {
        parts.push(format!("{}ms", millis));
    }

    parts.join(" ")
}

/// if `text` starts with `$`, returns the variable name portion; else None.
fn extract_var_ref(text: &str) -> Option<&str> {
    let t = text.trim();
    if t.starts_with('$') && t.len() > 1 {
        Some(&t[1..])
    } else {
        None
    }
}

/// auto-focuses the primary input once per modal open (one-shot egui temp flag).
pub fn auto_focus(ui: &egui::Ui, id: &str, resp: &egui::Response) {
    let eid = egui::Id::new(id).with("__autofocused");
    let done = ui.ctx().data(|d| d.get_temp::<bool>(eid).unwrap_or(false));
    if !done {
        resp.request_focus();
        ui.ctx().data_mut(|d| d.insert_temp(eid, true));
    }
}

/// renders a smart text field accepting a literal number or `$var_name`, with a
/// dropdown of known variables; returns `(submitted, response)`.
pub fn var_or_num_field(
    ui: &mut egui::Ui,
    state: &SharedState,
    id: &str,
    text: &mut String,
    width: f32,
) -> (bool, egui::Response) {
    let known = available_variable_names(state);
    let mut submitted = false;
    let mut field_resp: Option<egui::Response> = None;
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let resp = ui.add(
                egui::TextEdit::singleline(text)
                    .id(egui::Id::new(id))
                    .hint_text("$var, num, or text")
                    .desired_width(width),
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submitted = true;
            }
            field_resp = Some(resp);
            if !known.is_empty() {
                ui.menu_button(egui_phosphor::regular::CARET_DOWN, |ui| {
                    ui.set_min_width(120.0);
                    for name in &known {
                        if ui.button(format!("${}", name)).clicked() {
                            *text = format!("${}", name);
                            ui.close();
                        }
                    }
                });
            }
        });

        // always render one warning line; an empty string still reserves height, so the layout never jumps.
        let (warn_text, warn_color) = match extract_var_ref(text) {
            Some(var_name) if !known.contains(&var_name.to_string()) => (
                format!("⚠ ${}  not found (defaults to 0)", var_name),
                egui::Color32::from_rgb(220, 170, 50),
            ),
            _ => (String::new(), egui::Color32::TRANSPARENT),
        };
        ui.label(egui::RichText::new(warn_text).color(warn_color).size(10.0));
    });
    (submitted, field_resp.unwrap())
}

/// scans a formula for `$name` references not present in the known variable list.
pub fn unknown_vars_in_formula<'a>(expression: &'a str, known: &[String]) -> Vec<&'a str> {
    let mut result = Vec::new();
    let mut chars = expression.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '$' {
            let start = i + 1;
            let end = expression[start..]
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .map(|n| start + n)
                .unwrap_or(expression.len());
            let name = &expression[start..end];
            if !name.is_empty() && !known.contains(&name.to_string()) && !result.contains(&name) {
                result.push(name);
            }
            // advance past the name so its characters are not re-scanned as new `$` starts.
            for _ in 0..name.len() {
                chars.next();
            }
        }
    }
    result
}

// TODO: share the `$name` scanning logic between `unknown_vars_in_formula` and `extract_var_ref`.
pub fn coord_controls(
    ui: &mut egui::Ui,
    state: &SharedState,
    palette: &ThemePalette,
    label: &str,
    c: &mut Coord,
    should_focus: bool,
) -> bool {
    let mut is_var = matches!(c, Coord::Var(_));
    let mut var_name = match c {
        Coord::Var(name) => name.clone(),
        Coord::Const(_) => String::new(),
    };
    let mut num = match c {
        Coord::Const(v) => *v,
        Coord::Var(_) => 0,
    };

    let mut submitted = false;
    let focus_id_str = format!("coord_input_{}", label);

    ui.label(egui::RichText::new(label).color(palette.text_muted));
    ui.horizontal(|ui| {
        let type_label = if is_var { "Variable" } else { "Value" };
        egui::ComboBox::from_id_salt(format!("{}_type", label))
            .selected_text(type_label)
            .width(80.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut is_var, false, "Value");
                ui.selectable_value(&mut is_var, true, "Variable");
            });

        ui.add_space(4.0);

        let field_resp;

        if is_var {
            let known = available_variable_names(state);
            ui.spacing_mut().item_spacing.x = 2.0;
            let resp = ui.add(
                egui::TextEdit::singleline(&mut var_name)
                    .id(egui::Id::new(&focus_id_str))
                    .hint_text("var_name")
                    .desired_width(140.0),
            );
            field_resp = Some(resp.clone());
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submitted = true;
            }

            if !known.is_empty() {
                ui.menu_button(egui_phosphor::regular::CARET_DOWN, |ui| {
                    ui.set_min_width(120.0);
                    for name in &known {
                        if ui.button(name).clicked() {
                            var_name = name.clone();
                            ui.close();
                        }
                    }
                });
            }
        } else {
            let resp = ui.add(egui::DragValue::new(&mut num).speed(1));
            field_resp = Some(resp.clone());
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submitted = true;
            }
        }

        if should_focus && let Some(resp) = field_resp {
            auto_focus(ui, &focus_id_str, &resp);
        }
    });
    ui.end_row();

    *c = if is_var {
        Coord::Var(var_name)
    } else {
        Coord::Const(num)
    };

    submitted
}
