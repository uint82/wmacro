//! toolbox sidebar listing the insertable tools and opening their config modals.

use super::IdeState;
use super::block_analysis::{BlockAnalysis, analyze_blocks};
use super::components::tool_button;
use super::modals::{
    KeyActionType, Modal, ModalWidget, MouseActionType,
    calculate::CalculateModal,
    comment::CommentModal,
    delay::DelayModal,
    get_clipboard::GetClipboardModal,
    goto::GotoModal,
    if_color::IfPixelColorModal,
    if_color_found::IfColorFoundModal,
    if_compare::IfCompareModal,
    if_image::{IfImageFoundModal, SearchRegion},
    import_macro::ImportMacroModal,
    keyboard::KeyModal,
    label::LabelModal,
    loop_macro::LoopModal,
    mouse::MouseModal,
    open_file::OpenFileModal,
    set_clipboard::SetClipboardModal,
    set_variable::SetVariableModal,
    type_text::TypeTextModal,
};
use super::settings::save_settings;
use super::theme::*;
use crate::state::{DelayUnit, SharedState};
use eframe::egui;
use wmacro_core_types::{CompareOp, Coord, MacroCommand};

const MAX_RECENTS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ToolId {
    Delay,
    Mouse,
    Keyboard,
    TypeText,
    IfPixelColor,
    IfImageFound,
    IfColorFound,
    IfVariable,
    Else,
    EndIf,
    Loop,
    EndLoop,
    Label,
    Goto,
    ImportMacro,
    SetVariable,
    Calculate,
    SetClipboard,
    GetClipboard,
    OpenFile,
    Comment,
}

pub struct ToolDef {
    pub id: ToolId,
    pub icon: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub color: fn(&ThemePalette) -> egui::Color32,
}

impl ToolDef {
    fn matches(&self, query: &str) -> bool {
        self.label.to_lowercase().contains(query) || self.description.to_lowercase().contains(query)
    }
}

struct ToolCategory {
    name: &'static str,
    tools: &'static [ToolId],
}

const ALL_TOOLS: &[ToolDef] = &[
    ToolDef {
        id: ToolId::Delay,
        icon: egui_phosphor::regular::TIMER,
        label: "Delay",
        description: "Pause the macro for a set amount of time",
        color: |p| p.col_delay,
    },
    ToolDef {
        id: ToolId::Mouse,
        icon: egui_phosphor::regular::MOUSE,
        label: "Mouse",
        description: "Click, move, or scroll the mouse",
        color: |p| p.col_click,
    },
    ToolDef {
        id: ToolId::Keyboard,
        icon: egui_phosphor::regular::KEYBOARD,
        label: "Keyboard",
        description: "Press, hold, or release keyboard keys",
        color: |p| p.col_keyboard,
    },
    ToolDef {
        id: ToolId::TypeText,
        icon: egui_phosphor::regular::TEXT_T,
        label: "Type Text",
        description: "Type text as if typed on the keyboard",
        color: |p| p.col_type_text,
    },
    ToolDef {
        id: ToolId::IfPixelColor,
        icon: egui_phosphor::regular::PALETTE,
        label: "If Pixel Color Equals",
        description: "Branch when a pixel matches an exact color",
        color: |p| p.col_if,
    },
    ToolDef {
        id: ToolId::IfImageFound,
        icon: egui_phosphor::regular::IMAGE,
        label: "If Image Found",
        description: "Branch when an image appears on screen",
        color: |p| p.col_if,
    },
    ToolDef {
        id: ToolId::IfColorFound,
        icon: egui_phosphor::regular::CHECKERBOARD,
        label: "If Color Found",
        description: "Branch when a color appears on screen",
        color: |p| p.col_if,
    },
    ToolDef {
        id: ToolId::IfVariable,
        icon: egui_phosphor::regular::FLOW_ARROW,
        label: "If Variable",
        description: "Branch based on a variable comparison",
        color: |p| p.col_if,
    },
    ToolDef {
        id: ToolId::Else,
        icon: egui_phosphor::regular::ARROWS_LEFT_RIGHT,
        label: "Else",
        description: "Run when the previous condition is false",
        color: |p| p.col_else,
    },
    ToolDef {
        id: ToolId::EndIf,
        icon: egui_phosphor::regular::STOP_CIRCLE,
        label: "End If",
        description: "Close the current condition block",
        color: |p| p.col_end_if,
    },
    ToolDef {
        id: ToolId::Loop,
        icon: egui_phosphor::regular::REPEAT,
        label: "Loop",
        description: "Repeat a block of commands",
        color: |p| p.col_loop,
    },
    ToolDef {
        id: ToolId::EndLoop,
        icon: egui_phosphor::regular::ARROW_U_UP_LEFT,
        label: "End Loop",
        description: "Close the current loop block",
        color: |p| p.col_end_loop,
    },
    ToolDef {
        id: ToolId::Label,
        icon: egui_phosphor::regular::TAG,
        label: "Label",
        description: "Mark a position to jump to",
        color: |p| p.col_label,
    },
    ToolDef {
        id: ToolId::Goto,
        icon: egui_phosphor::regular::LINK,
        label: "GOTO",
        description: "Jump to a labeled position",
        color: |p| p.col_goto,
    },
    ToolDef {
        id: ToolId::ImportMacro,
        icon: egui_phosphor::regular::FOLDER_OPEN,
        label: "Import Macro",
        description: "Run another macro from a file",
        color: |p| p.col_import_saved_macro,
    },
    ToolDef {
        id: ToolId::SetVariable,
        icon: egui_phosphor::regular::FUNCTION,
        label: "Set Variable",
        description: "Store a value in a variable",
        color: |p| p.col_var,
    },
    ToolDef {
        id: ToolId::Calculate,
        icon: egui_phosphor::regular::CALCULATOR,
        label: "Calculate",
        description: "Evaluate an expression and store the result",
        color: |p| p.col_calc,
    },
    ToolDef {
        id: ToolId::SetClipboard,
        icon: egui_phosphor::regular::CLIPBOARD_TEXT,
        label: "Set Clipboard",
        description: "Copy text to the clipboard",
        color: |p| p.col_clipboard,
    },
    ToolDef {
        id: ToolId::GetClipboard,
        icon: egui_phosphor::regular::CLIPBOARD,
        label: "Get Clipboard",
        description: "Read clipboard text into a variable",
        color: |p| p.col_clipboard,
    },
    ToolDef {
        id: ToolId::OpenFile,
        icon: egui_phosphor::regular::FILE_ARROW_UP,
        label: "Open File / Program",
        description: "Launch a file or program with arguments",
        color: |p| p.col_import_saved_macro,
    },
    ToolDef {
        id: ToolId::Comment,
        icon: egui_phosphor::regular::NOTE,
        label: "Comment",
        description: "Add a note to explain part of the macro",
        color: |p| p.text_muted,
    },
];

const CATEGORIES: &[ToolCategory] = &[
    ToolCategory {
        name: "Input",
        tools: &[
            ToolId::Delay,
            ToolId::Mouse,
            ToolId::Keyboard,
            ToolId::TypeText,
        ],
    },
    ToolCategory {
        name: "Control Flow",
        tools: &[
            ToolId::IfPixelColor,
            ToolId::IfImageFound,
            ToolId::IfColorFound,
            ToolId::IfVariable,
            ToolId::Else,
            ToolId::EndIf,
            ToolId::Loop,
            ToolId::EndLoop,
            ToolId::Label,
            ToolId::Goto,
        ],
    },
    ToolCategory {
        name: "Data",
        tools: &[
            ToolId::SetVariable,
            ToolId::Calculate,
            ToolId::SetClipboard,
            ToolId::GetClipboard,
        ],
    },
    ToolCategory {
        name: "System",
        tools: &[ToolId::ImportMacro, ToolId::OpenFile],
    },
    ToolCategory {
        name: "Notes",
        tools: &[ToolId::Comment],
    },
];

fn tool_disabled_reason(id: ToolId, balance: &BlockAnalysis) -> Option<&'static str> {
    // block-closing tools are dead weight when no matching block is open, so disable them with a hint instead of letting the user create orphans.
    match id {
        ToolId::Else | ToolId::EndIf if balance.open_ifs == 0 => Some("Add an If command first"),
        ToolId::EndLoop if balance.open_loops == 0 => Some("Add a Loop command first"),
        _ => None,
    }
}

enum ToolAction {
    OpenModal(Modal),
    Append(MacroCommand),
}

use ToolAction::{Append, OpenModal};

pub fn render_toolbox(ui: &mut egui::Ui, state: &SharedState, ide: &mut IdeState) {
    // TODO: make the toolbox width persistent per-user; right now it resets to 220.0 on every launch.
    let palette = {
        let s = state.lock().unwrap();
        s.theme_manager.get_theme(&s.theme_name)
    };

    egui::Panel::left("ide_toolbox")
        .resizable(true)
        .default_size(220.0)
        .size_range(150.0..=400.0)
        .frame(sidebar_frame(&palette))
        .show_inside(ui, |ui| {
            ui.style_mut().spacing.scroll.bar_width = 4.0;

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    render_toolbox_contents(ui, state, ide, &palette);
                });
        });
}
fn render_toolbox_contents(
    ui: &mut egui::Ui,
    state: &SharedState,
    ide: &mut IdeState,
    palette: &ThemePalette,
) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Add Command")
            .strong()
            .size(13.0)
            .color(palette.text_primary),
    );
    ui.add_space(8.0);

    render_search_box(ui, ide, palette);
    ui.add_space(8.0);

    let query = ide.toolbox_search.trim().to_lowercase();
    let balance = block_balance_of(state);
    // an empty query shows the categorized browse view; anything typed flips to a flat, cross-category result list.
    if query.is_empty() {
        render_browse(ui, state, ide, palette, &balance);
    } else {
        render_search_results(ui, state, ide, palette, &query, &balance);
    }
}

fn render_search_box(ui: &mut egui::Ui, ide: &mut IdeState, palette: &ThemePalette) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS).color(palette.text_muted),
        );
        let resp = ui.add(
            egui::TextEdit::singleline(&mut ide.toolbox_search)
                .hint_text("Search commands...  (Ctrl+F)")
                .desired_width(f32::INFINITY),
        );
        if ide.focus_toolbox_search {
            resp.request_focus();
            ide.focus_toolbox_search = false;
        }
    });

    if !ide.toolbox_search.is_empty()
        && matches!(ide.modal, Modal::None)
        && ui.input(|i| i.key_pressed(egui::Key::Escape))
    {
        ide.toolbox_search.clear();
    }
}

fn render_browse(
    ui: &mut egui::Ui,
    state: &SharedState,
    ide: &mut IdeState,
    palette: &ThemePalette,
    balance: &BlockAnalysis,
) {
    let recents = {
        let s = state.lock().unwrap();
        s.toolbox_recents.clone()
    };

    if !recents.is_empty() {
        let collapsed = state.lock().unwrap().recents_collapsed;

        let prev_indent = ui.spacing().indent;
        ui.spacing_mut().indent = 0.0;
        let resp = egui::CollapsingHeader::new(
            egui::RichText::new("Recent")
                .strong()
                .size(11.0)
                .color(palette.text_muted),
        )
        .id_salt("toolbox_recents")
        .open(Some(!collapsed))
        .icon(|_, _, _| {})
        .show_unindented(ui, |ui| {
            for id in recents {
                render_tool_row(ui, state, ide, palette, id, balance);
            }
        });
        ui.spacing_mut().indent = prev_indent;

        if resp.header_response.clicked() {
            {
                let mut s = state.lock().unwrap();
                s.recents_collapsed = !s.recents_collapsed;
            }
            save_settings(state);
        }

        ui.add_space(8.0);
    }

    for category in CATEGORIES {
        render_category(ui, state, ide, palette, category, "", balance);
    }
}

fn render_search_results(
    ui: &mut egui::Ui,
    state: &SharedState,
    ide: &mut IdeState,
    palette: &ThemePalette,
    query: &str,
    balance: &BlockAnalysis,
) {
    let mut shown = false;
    for category in CATEGORIES {
        shown |= render_category(ui, state, ide, palette, category, query, balance);
    }

    if !shown {
        ui.label(
            egui::RichText::new(format!("No commands match \"{}\"", query))
                .color(palette.text_muted)
                .size(12.0),
        );
    }
}

fn render_category(
    ui: &mut egui::Ui,
    state: &SharedState,
    ide: &mut IdeState,
    palette: &ThemePalette,
    category: &ToolCategory,
    query: &str,
    balance: &BlockAnalysis,
) -> bool {
    let matched: Vec<ToolId> = if query.is_empty() {
        category.tools.to_vec()
    } else {
        category
            .tools
            .iter()
            .copied()
            .filter(|id| tool_def(*id).matches(query))
            .collect()
    };

    if matched.is_empty() {
        return false;
    }

    let prev_indent = ui.spacing().indent;
    ui.spacing_mut().indent = 0.0;
    egui::CollapsingHeader::new(
        egui::RichText::new(category.name)
            .strong()
            .size(11.0)
            .color(palette.text_muted),
    )
    .default_open(true)
    .icon(|_, _, _| {})
    .show_unindented(ui, |ui| {
        for id in matched {
            render_tool_row(ui, state, ide, palette, id, balance);
        }
    });
    ui.spacing_mut().indent = prev_indent;

    true
}

fn render_tool_row(
    ui: &mut egui::Ui,
    state: &SharedState,
    ide: &mut IdeState,
    palette: &ThemePalette,
    id: ToolId,
    balance: &BlockAnalysis,
) {
    let def = tool_def(id);
    let disabled_reason = tool_disabled_reason(id, balance);
    let tooltip = disabled_reason.unwrap_or(def.description);
    let clicked = tool_button(
        ui,
        def.icon,
        def.label,
        (def.color)(palette),
        tooltip,
        disabled_reason.is_none(),
        palette,
    );
    ui.add_space(4.0);

    if clicked {
        activate_tool(id, state, ide);
    }
}

fn tool_def(id: ToolId) -> &'static ToolDef {
    ALL_TOOLS
        .iter()
        .find(|t| t.id == id)
        .expect("tool def must exist for every ToolId")
}

fn block_balance_of(state: &SharedState) -> BlockAnalysis {
    let s = state.lock().unwrap();
    let commands = s
        .macro_state
        .current_macro
        .as_ref()
        .map(|m| m.commands.as_slice())
        .unwrap_or(&[]);
    analyze_blocks(commands)
}

fn activate_tool(id: ToolId, state: &SharedState, ide: &mut IdeState) {
    {
        let mut s = state.lock().unwrap();
        record_tool_use(&mut s.toolbox_recents, id);
    }
    save_settings(state);

    match tool_action(id, state) {
        ToolAction::OpenModal(modal) => ide.modal = modal,
        ToolAction::Append(cmd) => ide.append_command_after_selection(state, cmd),
    }
}

fn record_tool_use(recents: &mut Vec<ToolId>, id: ToolId) {
    // a small MRU list, like a browser's frequently-used bookmarks; the most recent tool always leads the pack.
    if recents.first() == Some(&id) {
        return;
    }

    recents.retain(|r| *r != id);
    recents.insert(0, id);
    recents.truncate(MAX_RECENTS);
}

fn tool_action(id: ToolId, state: &SharedState) -> ToolAction {
    match id {
        ToolId::Delay => OpenModal(widget_modal(Box::new(DelayModal {
            value: 500,
            unit: DelayUnit::Milliseconds,
            target_indices: vec![],
            duration_text: "500".to_string(),
            edit_idx: None,
        }))),
        ToolId::Mouse => {
            let (x, y) = current_pos_coord(state);
            OpenModal(widget_modal(Box::new(MouseModal {
                action: MouseActionType::LeftClick,
                x,
                y,
                use_current_pos: false,
                jitter: 0,
                hold_time_ms: 30,
                scroll_dx: 0,
                scroll_dy: 0,
                edit_idx: None,
            })))
        }
        ToolId::Keyboard => OpenModal(widget_modal(Box::new(KeyModal {
            key: String::new(),
            code: 0,
            action: KeyActionType::Press,
            hold_time_ms: 30,
            edit_idx: None,
            search: String::new(),
        }))),
        ToolId::TypeText => OpenModal(widget_modal(Box::new(TypeTextModal {
            text: String::new(),
            edit_idx: None,
        }))),
        ToolId::IfPixelColor => {
            let (x, y) = current_pos_coord(state);
            OpenModal(widget_modal(Box::new(IfPixelColorModal {
                x,
                y,
                r: 255,
                g: 255,
                b: 255,
                tolerance: 0,
                edit_idx: None,
                last_check: None,
            })))
        }
        ToolId::IfImageFound => OpenModal(widget_modal(Box::new(IfImageFoundModal {
            target_image_path: String::new(),
            similarity_threshold: 0.85,
            move_cursor_if_found: false,
            trigger_if_not_found: false,
            search_region: SearchRegion::WholeScreen,
            region_top: 0,
            region_left: 0,
            region_width: 0,
            region_height: 0,
            store_x: None,
            store_y: None,
            test_result: empty_arc(),
            preview_texture: None,
            edit_idx: None,
        }))),
        ToolId::IfColorFound => OpenModal(widget_modal(Box::new(IfColorFoundModal {
            r: 255,
            g: 255,
            b: 255,
            tolerance: 0,
            min_width: 1,
            min_height: 1,
            move_cursor_if_found: false,
            search_region: SearchRegion::WholeScreen,
            region_top: 0,
            region_left: 0,
            region_width: 0,
            region_height: 0,
            store_x: None,
            store_y: None,
            store_w: None,
            store_h: None,
            test_result: empty_arc(),
            edit_idx: None,
        }))),
        ToolId::IfVariable => OpenModal(widget_modal(Box::new(IfCompareModal {
            left_text: String::new(),
            op: CompareOp::Eq,
            right_text: String::new(),
            edit_idx: None,
        }))),
        ToolId::Else => Append(MacroCommand::Else),
        ToolId::EndIf => Append(MacroCommand::EndIf),
        ToolId::Loop => OpenModal(widget_modal(Box::new(LoopModal {
            count_text: "5".to_string(),
            edit_idx: None,
        }))),
        ToolId::EndLoop => Append(MacroCommand::EndLoop),
        ToolId::Label => OpenModal(widget_modal(Box::new(LabelModal {
            name: String::new(),
            edit_idx: None,
        }))),
        ToolId::Goto => OpenModal(widget_modal(Box::new(GotoModal {
            target: String::new(),
            edit_idx: None,
        }))),
        ToolId::ImportMacro => OpenModal(widget_modal(Box::new(ImportMacroModal {
            path: String::new(),
            edit_idx: None,
            pending_path: empty_arc(),
        }))),
        ToolId::SetVariable => OpenModal(widget_modal(Box::new(SetVariableModal {
            target: String::new(),
            value_text: String::new(),
            edit_idx: None,
        }))),
        ToolId::Calculate => OpenModal(widget_modal(Box::new(CalculateModal {
            target: String::new(),
            expression: String::new(),
            edit_idx: None,
        }))),
        ToolId::SetClipboard => OpenModal(widget_modal(Box::new(SetClipboardModal {
            text: String::new(),
            edit_idx: None,
        }))),
        ToolId::GetClipboard => OpenModal(widget_modal(Box::new(GetClipboardModal {
            target: String::new(),
            edit_idx: None,
        }))),
        ToolId::OpenFile => OpenModal(widget_modal(Box::new(OpenFileModal {
            path: String::new(),
            args: String::new(),
            run_as_admin: false,
            edit_idx: None,
            pending_path: empty_arc(),
        }))),
        ToolId::Comment => OpenModal(widget_modal(Box::new(CommentModal {
            text: String::new(),
            edit_idx: None,
        }))),
    }
}

fn widget_modal(widget: Box<dyn ModalWidget>) -> Modal {
    Modal::Widget(widget)
}

fn empty_arc<T>() -> std::sync::Arc<std::sync::Mutex<Option<T>>> {
    std::sync::Arc::new(std::sync::Mutex::new(None))
}

fn current_pos_coord(state: &SharedState) -> (Coord, Coord) {
    let (x, y) = current_cursor_pos(state);
    (Coord::Const(x), Coord::Const(y))
}

fn current_cursor_pos(state: &SharedState) -> (i32, i32) {
    let s = state.lock().unwrap();
    (s.cursor_x, s.cursor_y)
}
