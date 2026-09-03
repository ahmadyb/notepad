use crate::layout::Rect;
use notepad_core::Note;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteSort {
    Modified,
    Alphabetical,
}

#[derive(Debug, Clone)]
pub struct SidebarState {
    pub query: String,
    pub selected_note: Option<i64>,
    pub notes: Vec<Note>,
    pub sort: NoteSort,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            query: String::new(),
            selected_note: None,
            notes: Vec::new(),
            sort: NoteSort::Modified,
        }
    }
}

impl SidebarState {
    pub fn set_notes(&mut self, mut notes: Vec<Note>) {
        if matches!(self.sort, NoteSort::Alphabetical) {
            notes.sort_by_key(|note| note.title.to_lowercase());
        }
        self.notes = notes;
        if self
            .selected_note
            .is_some_and(|id| !self.notes.iter().any(|note| note.id == id))
        {
            self.selected_note = None;
        }
    }

    pub fn card_rect(&self, bounds: Rect, index: usize) -> Rect {
        Rect::new(
            bounds.x + 12,
            bounds.y + 82 + index as i32 * 74,
            (bounds.w - 24).max(0),
            64,
        )
    }

    pub fn note_at(&self, bounds: Rect, x: i32, y: i32) -> Option<(i64, NoteAction)> {
        for (index, note) in self.notes.iter().enumerate() {
            let card = self.card_rect(bounds, index);
            if !card.contains(x, y) {
                continue;
            }
            let pin = Rect::new(card.right() - 54, card.y + 8, 22, 22);
            let delete = Rect::new(card.right() - 29, card.y + 8, 22, 22);
            let action = if pin.contains(x, y) {
                NoteAction::TogglePin
            } else if delete.contains(x, y) {
                NoteAction::Delete
            } else {
                NoteAction::Open
            };
            return Some((note.id, action));
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteAction {
    Open,
    TogglePin,
    Delete,
}
