use crate::types::{LineColour, LineMetadata, ListType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModificationKind {
    InsertText,
    DeleteText,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataSnapshot {
    pub lines: Vec<LineMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataOverlay {
    lines: Vec<LineMetadata>,
}

impl Default for MetadataOverlay {
    fn default() -> Self {
        Self::new(1)
    }
}

impl MetadataOverlay {
    pub fn new(line_count: usize) -> Self {
        Self {
            lines: vec![LineMetadata::default(); line_count.max(1)],
        }
    }

    pub fn from_lines(mut lines: Vec<LineMetadata>) -> Self {
        if lines.is_empty() {
            lines.push(LineMetadata::default());
        }
        Self { lines }
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&LineMetadata> {
        self.lines.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut LineMetadata> {
        self.lines.get_mut(index)
    }

    pub fn as_slice(&self) -> &[LineMetadata] {
        &self.lines
    }

    pub fn snapshot(&self) -> MetadataSnapshot {
        MetadataSnapshot {
            lines: self.lines.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: MetadataSnapshot) {
        *self = Self::from_lines(snapshot.lines);
    }

    pub fn on_insert(&mut self, line: usize, count: usize) {
        if count == 0 {
            return;
        }
        let at = (line + 1).min(self.lines.len());
        self.lines.splice(
            at..at,
            std::iter::repeat_with(LineMetadata::default).take(count),
        );
    }

    pub fn on_delete(&mut self, line: usize, count: usize) {
        if count == 0 || self.lines.len() < 2 {
            return;
        }
        let start = (line + 1).min(self.lines.len());
        let end = (start + count).min(self.lines.len());
        if start < end {
            self.lines.drain(start..end);
        }
        if self.lines.is_empty() {
            self.lines.push(LineMetadata::default());
        }
    }

    pub fn on_modified(&mut self, line: usize, kind: ModificationKind, lines_added: isize) {
        match kind {
            ModificationKind::InsertText if lines_added > 0 => {
                self.on_insert(line, lines_added as usize);
            }
            ModificationKind::DeleteText if lines_added < 0 => {
                self.on_delete(line, (-lines_added) as usize);
            }
            _ => {}
        }
    }

    pub fn sync_to_line_count(&mut self, line_count: usize) {
        let line_count = line_count.max(1);
        if line_count > self.lines.len() {
            self.lines.extend(
                std::iter::repeat_with(LineMetadata::default)
                    .take(line_count - self.lines.len()),
            );
        } else {
            self.lines.truncate(line_count);
        }
    }

    pub fn inserted_newlines(&mut self, line: usize, count: usize) {
        self.on_insert(line, count);
    }

    pub fn deleted_newlines(&mut self, line: usize, count: usize) {
        self.on_delete(line, count);
    }

    pub fn apply_colour(&mut self, start: usize, end: usize, colour: LineColour) {
        if self.lines.is_empty() || start > end || start >= self.lines.len() {
            return;
        }
        let end = end.min(self.lines.len() - 1);
        let toggle_off = (start..=end).all(|index| self.lines[index].colour == colour);
        for index in start..=end {
            self.lines[index].colour = if toggle_off {
                LineColour::None
            } else {
                colour
            };
        }
    }

    pub fn clear_colour(&mut self, start: usize, end: usize) {
        if self.lines.is_empty() || start > end || start >= self.lines.len() {
            return;
        }
        for index in start..=end.min(self.lines.len() - 1) {
            self.lines[index].colour = LineColour::None;
        }
    }

    pub fn toggle_checked(&mut self, line: usize) -> Option<bool> {
        let metadata = self.lines.get_mut(line)?;
        if metadata.list_type != ListType::Check {
            return None;
        }
        metadata.checked = !metadata.checked;
        Some(metadata.checked)
    }
}
