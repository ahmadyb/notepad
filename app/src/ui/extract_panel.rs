use crate::layout::Rect;
use notepad_core::{ColorOrder, LineColour};

#[derive(Debug, Clone)]
pub struct ExtractPanelState {
    pub open: bool,
    pub bounds: Rect,
    pub selected_colours: Vec<LineColour>,
    pub available_colours: Vec<(LineColour, usize)>,
    pub selection_initialized: bool,
    pub order: ColorOrder,
    pub preview: String,
    pub copied: bool,
}

impl Default for ExtractPanelState {
    fn default() -> Self {
        Self {
            open: false,
            bounds: Rect::default(),
            selected_colours: Vec::new(),
            available_colours: Vec::new(),
            selection_initialized: false,
            order: ColorOrder::Document,
            preview: String::new(),
            copied: false,
        }
    }
}

impl ExtractPanelState {
    pub fn set_available(&mut self, colours: &[(LineColour, usize)]) {
        let had_available = !self.available_colours.is_empty();
        self.available_colours = colours.to_vec();
        if !self.selection_initialized || (!had_available && !colours.is_empty() && self.selected_colours.is_empty()) {
            self.selected_colours = colours.iter().map(|(colour, _)| *colour).collect();
            self.selection_initialized = true;
        } else {
            self.selected_colours
                .retain(|colour| colours.iter().any(|(available, _)| available == colour));
        }
    }

    pub fn toggle_colour(&mut self, colour: LineColour) {
        if let Some(index) = self.selected_colours.iter().position(|item| *item == colour) {
            self.selected_colours.remove(index);
        } else {
            self.selected_colours.push(colour);
        }
        self.selection_initialized = true;
        self.copied = false;
    }

    pub fn select_all(&mut self) {
        self.selection_initialized = true;
        self.selected_colours = self.available_colours.iter().map(|(colour, _)| *colour).collect();
    }

    pub fn clear_selection(&mut self) {
        self.selected_colours.clear();
        self.selection_initialized = true;
    }

    pub fn selected(&self, colour: LineColour) -> bool {
        self.selected_colours.contains(&colour)
    }

    pub fn colour_row(&self, index: usize) -> Rect {
        Rect::new(self.bounds.x + 14, self.bounds.y + 98 + index as i32 * 30, self.bounds.w - 28, 26)
    }

    pub fn preview_rect(&self) -> Rect {
        let y = self.bounds.y + 98 + self.available_colours.len() as i32 * 30 + 12;
        Rect::new(self.bounds.x + 14, y, self.bounds.w - 28, (self.bounds.bottom() - y - 54).max(70))
    }
}
