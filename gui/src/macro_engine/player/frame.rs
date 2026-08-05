use std::collections::HashMap;
use wmacro_core_types::MacroCommand;

#[derive(Clone)]
pub struct ExecFrame {
    pub commands: Vec<MacroCommand>,
    pub idx: usize,
    pub loop_stack: Vec<(usize, u32)>,
    pub labels: HashMap<String, usize>,
}

impl ExecFrame {
    pub fn new(commands: Vec<MacroCommand>) -> Self {
        let labels = Self::extract_labels(&commands);
        Self { commands, idx: 0, loop_stack: Vec::new(), labels }
    }

    fn extract_labels(commands: &[MacroCommand]) -> HashMap<String, usize> {
        commands.iter().enumerate().filter_map(|(i, cmd)| match cmd {
            MacroCommand::Label(name) => Some((name.clone(), i)),
            _ => None,
        }).collect()
    }

    pub fn skip_to_else_or_endif(&mut self) {
        let mut nested = 0;
        while self.idx + 1 < self.commands.len() {
            self.idx += 1;
            if self.check_else_endif_break(&mut nested) { return; }
        }
    }

    fn check_else_endif_break(&self, nested: &mut usize) -> bool {
        match &self.commands[self.idx] {
            MacroCommand::IfPixelColor { .. } | MacroCommand::IfImageFound { .. } => { *nested += 1; false }
            MacroCommand::Else if *nested == 0 => true,
            MacroCommand::EndIf => self.handle_nested_end(nested),
            _ => false,
        }
    }

    pub fn skip_to_endif(&mut self) {
        let mut nested = 0;
        while self.idx + 1 < self.commands.len() {
            self.idx += 1;
            if self.check_endif_break(&mut nested) { return; }
        }
    }

    fn check_endif_break(&self, nested: &mut usize) -> bool {
        match &self.commands[self.idx] {
            MacroCommand::IfPixelColor { .. } | MacroCommand::IfImageFound { .. } => { *nested += 1; false }
            MacroCommand::EndIf => self.handle_nested_end(nested),
            _ => false,
        }
    }

    pub fn skip_to_endloop(&mut self) {
        let mut nested = 0;
        while self.idx + 1 < self.commands.len() {
            self.idx += 1;
            if self.check_endloop_break(&mut nested) { return; }
        }
    }

    fn check_endloop_break(&self, nested: &mut usize) -> bool {
        match &self.commands[self.idx] {
            MacroCommand::Loop { .. } => { *nested += 1; false }
            MacroCommand::EndLoop => self.handle_nested_end(nested),
            _ => false,
        }
    }

    fn handle_nested_end(&self, nested: &mut usize) -> bool {
        if *nested == 0 { return true; }
        *nested -= 1;
        false
    }

    pub fn find_label(&self, target: &str) -> Option<usize> {
        self.labels.get(target).copied()
    }
}
