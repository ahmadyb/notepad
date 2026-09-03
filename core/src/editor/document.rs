use super::{adjust_indent, handle_enter, parse_list_prefix, MetadataOverlay};
use crate::types::{LineMetadata, ListType};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub caret: usize,
}

impl Selection {
    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.caret)..self.anchor.max(self.caret)
    }

    pub fn is_empty(self) -> bool {
        self.anchor == self.caret
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSnapshot {
    pub text: String,
    pub metadata: Vec<LineMetadata>,
    pub cursor: usize,
    pub selection_anchor: usize,
}

#[derive(Debug, Clone)]
pub struct EditorBuffer {
    text: String,
    metadata: MetadataOverlay,
    cursor: usize,
    anchor: usize,
    undo: Vec<DocumentSnapshot>,
    redo: Vec<DocumentSnapshot>,
}

impl Default for EditorBuffer {
    fn default() -> Self {
        Self::new("")
    }
}

impl EditorBuffer {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            text: text.clone(),
            metadata: MetadataOverlay::new(line_count(&text)),
            cursor: 0,
            anchor: 0,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn from_parts(
        text: impl Into<String>,
        metadata: Vec<LineMetadata>,
        cursor: usize,
    ) -> Self {
        let text = text.into();
        let mut metadata = MetadataOverlay::from_lines(metadata);
        metadata.sync_to_line_count(line_count(&text));
        let cursor = boundary(&text, cursor);
        Self {
            text,
            metadata,
            cursor,
            anchor: cursor,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn metadata(&self) -> &[LineMetadata] {
        self.metadata.as_slice()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn selection(&self) -> Selection {
        Selection {
            anchor: self.anchor,
            caret: self.cursor,
        }
    }

    pub fn line_count(&self) -> usize {
        line_count(&self.text)
    }

    pub fn line_of(&self, position: usize) -> usize {
        self.text[..boundary(&self.text, position)]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
    }

    pub fn column_of(&self, position: usize) -> usize {
        let position = boundary(&self.text, position);
        let start = self.text[..position].rfind('\n').map_or(0, |index| index + 1);
        self.text[start..position].chars().count()
    }

    pub fn set_cursor(&mut self, position: usize, extend: bool) {
        let position = boundary(&self.text, position);
        if !extend {
            self.anchor = position;
        }
        self.cursor = position;
    }

    pub fn set_selection(&mut self, anchor: usize, caret: usize) {
        self.anchor = boundary(&self.text, anchor);
        self.cursor = boundary(&self.text, caret);
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.text.len();
    }

    pub fn clear_selection(&mut self) {
        self.anchor = self.cursor;
    }

    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            text: self.text.clone(),
            metadata: self.metadata.as_slice().to_vec(),
            cursor: self.cursor,
            selection_anchor: self.anchor,
        }
    }

    pub fn restore(&mut self, snapshot: DocumentSnapshot) {
        self.text = snapshot.text;
        self.metadata = MetadataOverlay::from_lines(snapshot.metadata);
        self.metadata.sync_to_line_count(line_count(&self.text));
        self.cursor = boundary(&self.text, snapshot.cursor);
        self.anchor = boundary(&self.text, snapshot.selection_anchor);
    }

    pub fn current_line(&self) -> &str {
        self.text
            .split('\n')
            .nth(self.line_of(self.cursor))
            .unwrap_or("")
            .trim_end_matches('\r')
    }

    pub fn selected_line_range(&self) -> (usize, usize) {
        let range = self.selection().range();
        let end = if range.is_empty() {
            range.start
        } else {
            range.end.saturating_sub(1)
        };
        (self.line_of(range.start), self.line_of(end))
    }

    pub fn insert_text(&mut self, text: &str) {
        let range = self.selection().range();
        if !range.is_empty() {
            self.replace_range(range, text);
            return;
        }
        if text.is_empty() {
            return;
        }
        self.replace_range(self.cursor..self.cursor, text);
    }

    pub fn replace_selection(&mut self, text: &str) {
        let range = self.selection().range();
        self.replace_range(range, text);
    }

    pub fn replace_range(&mut self, range: Range<usize>, text: &str) {
        let start = boundary(&self.text, range.start);
        let end = boundary(&self.text, range.end).max(start);
        if start == end && text.is_empty() {
            return;
        }
        self.edit();
        let line = self.line_of(start);
        let deleted = self.text[start..end]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
        self.text.replace_range(start..end, text);
        if deleted > 0 {
            self.metadata.on_delete(line, deleted);
        }
        let added = text.bytes().filter(|byte| *byte == b'\n').count();
        if added > 0 {
            self.metadata.on_insert(line, added);
        }
        self.metadata.sync_to_line_count(line_count(&self.text));
        self.cursor = boundary(&self.text, start + text.len());
        self.anchor = self.cursor;
        self.finish();
    }

    pub fn delete_backward(&mut self) {
        let range = self.selection().range();
        if !range.is_empty() {
            self.replace_range(range, "");
        } else if self.cursor > 0 {
            let start = previous_boundary(&self.text, self.cursor);
            self.replace_range(start..self.cursor, "");
        }
    }

    pub fn delete_forward(&mut self) {
        let range = self.selection().range();
        if !range.is_empty() {
            self.replace_range(range, "");
        } else if self.cursor < self.text.len() {
            let end = next_boundary(&self.text, self.cursor);
            self.replace_range(self.cursor..end, "");
        }
    }

    pub fn move_horizontal(&mut self, direction: i32, extend: bool) {
        let position = if direction < 0 {
            previous_boundary(&self.text, self.cursor)
        } else {
            next_boundary(&self.text, self.cursor)
        };
        self.set_cursor(position, extend);
    }

    pub fn move_vertical(&mut self, direction: i32, extend: bool) {
        let current_line = self.line_of(self.cursor) as isize;
        let target_line = (current_line + direction as isize)
            .clamp(0, self.line_count().saturating_sub(1) as isize) as usize;
        let column = self.column_of(self.cursor);
        let start = self.line_start(target_line);
        let end = self.line_end(target_line);
        let target = self.text[start..end]
            .char_indices()
            .nth(column)
            .map_or(end, |(index, _)| start + index);
        self.set_cursor(target, extend);
    }

    pub fn move_line_start(&mut self, extend: bool) {
        self.set_cursor(self.line_start(self.line_of(self.cursor)), extend);
    }

    pub fn move_line_end(&mut self, extend: bool) {
        self.set_cursor(self.line_end(self.line_of(self.cursor)), extend);
    }

    pub fn line_start(&self, line: usize) -> usize {
        let mut current = 0;
        for (index, byte) in self.text.bytes().enumerate() {
            if current == line {
                return index;
            }
            if byte == b'\n' {
                current += 1;
            }
        }
        self.text.len()
    }

    pub fn line_end(&self, line: usize) -> usize {
        let start = self.line_start(line);
        self.text[start..]
            .find('\n')
            .map_or(self.text.len(), |offset| start + offset)
    }

    pub fn apply_colour(&mut self, colour: crate::types::LineColour) {
        let (start, end) = self.selected_line_range();
        self.edit();
        self.metadata.apply_colour(start, end, colour);
        self.finish();
    }

    pub fn toggle_checkbox(&mut self, line: usize) -> Option<bool> {
        self.edit();
        let result = self.metadata.toggle_checked(line);
        self.finish();
        result
    }

    pub fn recognise_list_prefixes(&mut self, tab_width: usize) {
        self.edit();
        for (index, line) in self.text.split('\n').enumerate() {
            if let Some(metadata) = self.metadata.get_mut(index) {
                if let Some(prefix) = parse_list_prefix(line, tab_width) {
                    metadata.list_type = prefix.list_type;
                    metadata.indent = prefix.indent;
                    metadata.checked = prefix.checked;
                } else {
                    metadata.list_type = ListType::None;
                    metadata.indent = 0;
                    metadata.checked = false;
                }
            }
        }
        self.finish();
    }

    pub fn handle_enter(&mut self, tab_width: usize) {
        let line = self.current_line().to_owned();
        let line_index = self.line_of(self.cursor);
        let metadata = self
            .metadata
            .get(line_index)
            .cloned()
            .or_else(|| {
                parse_list_prefix(&line, tab_width).map(|prefix| LineMetadata {
                    list_type: prefix.list_type,
                    indent: prefix.indent,
                    checked: prefix.checked,
                    ..LineMetadata::default()
                })
            })
            .unwrap_or_default();
        let result = handle_enter(&line, &metadata);
        self.insert_text("\n");
        if let Some(next) = self.metadata.get_mut(line_index + 1) {
            *next = result.next_metadata;
        }
        if result.exits_list {
            if let Some(current) = self.metadata.get_mut(line_index) {
                current.list_type = ListType::None;
                current.indent = 0;
                current.checked = false;
            }
        }
    }

    pub fn adjust_indent(&mut self, increase: bool) {
        let line = self.line_of(self.cursor);
        self.edit();
        if let Some(metadata) = self.metadata.get_mut(line) {
            adjust_indent(metadata, increase);
        }
        self.finish();
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(snapshot);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(snapshot);
        true
    }

    fn edit(&mut self) {
        self.undo.push(self.snapshot());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn finish(&mut self) {
        if self
            .undo
            .last()
            .is_some_and(|snapshot| snapshot == &self.snapshot())
        {
            self.undo.pop();
        }
    }
}

fn line_count(text: &str) -> usize {
    text.split('\n').count().max(1)
}

fn boundary(text: &str, position: usize) -> usize {
    let mut position = position.min(text.len());
    while position > 0 && !text.is_char_boundary(position) {
        position -= 1;
    }
    position
}

fn previous_boundary(text: &str, position: usize) -> usize {
    boundary(text, position.saturating_sub(1))
}

fn next_boundary(text: &str, position: usize) -> usize {
    let position = boundary(text, position);
    position + text[position..].chars().next().map_or(0, char::len_utf8)
}
