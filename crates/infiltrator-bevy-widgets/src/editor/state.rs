//! Multiline editing preserves document bytes; caret columns are grapheme-boundary byte offsets.
use bevy::ecs::component::Component;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorUndoFrame {
    lines: Vec<String>,
    cursor: (usize, usize),
    anchor: Option<(usize, usize)>,
}
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct CodeEditorState {
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub selection_anchor: Option<(usize, usize)>,
    pub undo_stack: Vec<EditorUndoFrame>,
    pub redo_stack: Vec<EditorUndoFrame>,
    pub max_undo_depth: usize,
}
impl Default for CodeEditorState {
    fn default() -> Self {
        Self::new("")
    }
}
impl CodeEditorState {
    pub fn new(text: &str) -> Self {
        Self {
            lines: text.split('\n').map(str::to_owned).collect(),
            cursor_row: 0,
            cursor_col: 0,
            selection_anchor: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_undo_depth: 50,
        }
    }
    pub fn full_text(&self) -> String {
        self.lines.join("\n")
    }
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
    pub fn line_content(&self, row: usize) -> &str {
        let Some(line) = self.lines.get(row) else {
            return "";
        };
        if row + 1 < self.lines.len() {
            line.strip_suffix('\r').unwrap_or(line)
        } else {
            line
        }
    }
    fn position(&self, (row, col): (usize, usize)) -> (usize, usize) {
        let row = row.min(self.lines.len().saturating_sub(1));
        let line = self.line_content(row);
        let col = if col >= line.len() {
            line.len()
        } else {
            line.grapheme_indices(true)
                .map(|(offset, _)| offset)
                .take_while(|offset| *offset <= col)
                .last()
                .unwrap_or(0)
        };
        (row, col)
    }
    fn normalize(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        (self.cursor_row, self.cursor_col) = self.position((self.cursor_row, self.cursor_col));
        self.selection_anchor = self
            .selection_anchor
            .map(|position| self.position(position));
    }
    pub fn set_cursor(&mut self, row: usize, col: usize, selecting: bool) {
        self.normalize();
        if selecting {
            self.selection_anchor
                .get_or_insert((self.cursor_row, self.cursor_col));
        } else {
            self.selection_anchor = None;
        }
        (self.cursor_row, self.cursor_col) = self.position((row, col));
    }
    fn offset(&self, position: (usize, usize)) -> usize {
        let (row, col) = self.position(position);
        self.lines
            .iter()
            .take(row)
            .map(|line| line.len() + 1)
            .sum::<usize>()
            + col
    }
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.offset(self.selection_anchor?);
        let cursor = self.offset((self.cursor_row, self.cursor_col));
        (anchor != cursor).then_some((anchor.min(cursor), anchor.max(cursor)))
    }
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        Some(self.full_text()[start..end].to_owned())
    }
    pub fn select_all(&mut self) {
        self.normalize();
        self.selection_anchor = Some((0, 0));
        self.cursor_row = self.lines.len() - 1;
        self.cursor_col = self.line_content(self.cursor_row).len();
    }
    fn frame(&self) -> EditorUndoFrame {
        EditorUndoFrame {
            lines: self.lines.clone(),
            cursor: (self.cursor_row, self.cursor_col),
            anchor: self.selection_anchor,
        }
    }
    fn restore(&mut self, frame: EditorUndoFrame) {
        self.lines = frame.lines;
        (self.cursor_row, self.cursor_col) = frame.cursor;
        self.selection_anchor = frame.anchor;
        self.normalize();
    }
    fn snapshot_undo(&mut self) {
        if self.max_undo_depth > 0 {
            while self.undo_stack.len() >= self.max_undo_depth {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(self.frame());
        }
        self.redo_stack.clear();
    }
    pub fn undo(&mut self) -> bool {
        if let Some(frame) = self.undo_stack.pop() {
            self.redo_stack.push(self.frame());
            self.restore(frame);
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(frame) = self.redo_stack.pop() {
            self.undo_stack.push(self.frame());
            self.restore(frame);
            true
        } else {
            false
        }
    }
    pub fn set_text(&mut self, text: &str) {
        self.normalize();
        if self.full_text() == text {
            return;
        }
        self.snapshot_undo();
        self.lines = text.split('\n').map(str::to_owned).collect();
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.selection_anchor = None;
    }
    fn replace(&mut self, start: usize, end: usize, inserted: &str) {
        self.snapshot_undo();
        let mut text = self.full_text();
        text.replace_range(start..end, inserted);
        let caret = start + inserted.len();
        self.lines = text.split('\n').map(str::to_owned).collect();
        let prefix = &text[..caret];
        self.cursor_row = prefix.bytes().filter(|byte| *byte == b'\n').count();
        self.cursor_col = prefix.rsplit('\n').next().unwrap_or("").len();
        self.selection_anchor = None;
        self.normalize();
    }
    pub fn insert_char(&mut self, character: char) {
        self.insert_text(&character.to_string());
    }
    pub fn insert_text(&mut self, text: &str) {
        self.normalize();
        let offset = self.offset((self.cursor_row, self.cursor_col));
        let (start, end) = self.selection_range().unwrap_or((offset, offset));
        if text.is_empty() && start == end {
            return;
        }
        self.replace(start, end, text);
    }
    pub fn insert_newline(&mut self) {
        let newline = if self
            .lines
            .iter()
            .take(self.lines.len().saturating_sub(1))
            .any(|line| line.ends_with('\r'))
        {
            "\r\n"
        } else {
            "\n"
        };
        self.insert_text(newline);
    }
    pub fn indent(&mut self) {
        self.insert_text("  ");
    }
    pub fn delete_backwards(&mut self) {
        self.normalize();
        if let Some((start, end)) = self.selection_range() {
            self.replace(start, end, "");
            return;
        }
        let end = self.offset((self.cursor_row, self.cursor_col));
        if end == 0 {
            return;
        }
        let text = self.full_text();
        let start = text[..end]
            .grapheme_indices(true)
            .next_back()
            .map(|(offset, _)| offset)
            .unwrap_or(0);
        self.replace(start, end, "");
    }
    pub fn delete_forward(&mut self) -> bool {
        self.normalize();
        if let Some((start, end)) = self.selection_range() {
            self.replace(start, end, "");
            return true;
        }
        let start = self.offset((self.cursor_row, self.cursor_col));
        let text = self.full_text();
        let Some(grapheme) = text[start..].graphemes(true).next() else {
            return false;
        };
        self.replace(start, start + grapheme.len(), "");
        true
    }
    pub fn move_left(&mut self) -> bool {
        self.normalize();
        self.selection_anchor = None;
        if self.cursor_col > 0 {
            self.cursor_col = self.line_content(self.cursor_row)[..self.cursor_col]
                .grapheme_indices(true)
                .next_back()
                .map(|(offset, _)| offset)
                .unwrap_or(0);
            true
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.line_content(self.cursor_row).len();
            true
        } else {
            false
        }
    }
    pub fn move_right(&mut self) -> bool {
        self.normalize();
        self.selection_anchor = None;
        if let Some(grapheme) = self.line_content(self.cursor_row)[self.cursor_col..]
            .graphemes(true)
            .next()
        {
            self.cursor_col += grapheme.len();
            true
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
            true
        } else {
            false
        }
    }
    pub fn move_up(&mut self) -> bool {
        self.normalize();
        if self.cursor_row == 0 {
            return false;
        }
        let column = self.line_content(self.cursor_row)[..self.cursor_col]
            .graphemes(true)
            .count();
        self.cursor_row -= 1;
        self.cursor_col = self
            .line_content(self.cursor_row)
            .graphemes(true)
            .take(column)
            .map(str::len)
            .sum();
        self.selection_anchor = None;
        true
    }
    pub fn move_down(&mut self) -> bool {
        self.normalize();
        if self.cursor_row + 1 == self.lines.len() {
            return false;
        }
        let column = self.line_content(self.cursor_row)[..self.cursor_col]
            .graphemes(true)
            .count();
        self.cursor_row += 1;
        self.cursor_col = self
            .line_content(self.cursor_row)
            .graphemes(true)
            .take(column)
            .map(str::len)
            .sum();
        self.selection_anchor = None;
        true
    }
    pub fn move_home(&mut self) -> bool {
        self.normalize();
        let moved = self.cursor_col != 0;
        self.cursor_col = 0;
        self.selection_anchor = None;
        moved
    }
    pub fn move_end(&mut self) -> bool {
        self.normalize();
        let end = self.line_content(self.cursor_row).len();
        let moved = self.cursor_col != end;
        self.cursor_col = end;
        self.selection_anchor = None;
        moved
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
