//! Pure state machine for single-line text field editing.
//!
//! Contains [`ValidationStatus`], [`TextFieldInput`], [`TextFieldState`],
//! [`FieldVisual`], and [`field_visual`] — the zero-Bevy editing core that
//! hosts drive from whatever input seam they own.

use bevy::ecs::component::Component;

use unicode_segmentation::UnicodeSegmentation;

use crate::palette::UiPalette;

use super::ime::{
    ImeTransaction, PreeditStateMachine, find_next_word_boundary, find_prev_word_boundary,
};

/// Validation status of an input field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValidationStatus {
    /// Normal default styling.
    #[default]
    Normal,
    /// Successfully validated field.
    Valid,
    /// Warning / advisory message styling.
    Warning,
    /// Erroneous or invalid input styling.
    Error,
}

/// Resolve input border color based on validation status, focus, and live palette.
/// Pure function.
pub fn validation_border_color(
    status: ValidationStatus,
    is_focused: bool,
    palette: &UiPalette,
) -> bevy::color::Color {
    match status {
        ValidationStatus::Normal => {
            if is_focused {
                palette.accent
            } else {
                palette.border
            }
        }
        ValidationStatus::Valid => palette.success,
        ValidationStatus::Warning => palette.warning,
        ValidationStatus::Error => palette.danger,
    }
}

/// One editing operation the host's input seam feeds into
/// [`TextFieldState::apply`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextFieldInput {
    /// Insert text at the caret, replacing any selection.
    Insert(String),
    /// Remove the selection, or the char before the caret.
    Backspace,
    /// Remove the selection, or the char after the caret.
    Delete,
    /// Move the caret left one char; `true` extends the selection.
    Left(bool),
    /// Move the caret right one char; `true` extends the selection.
    Right(bool),
    /// Move the caret left one word boundary; `true` extends selection.
    WordLeft(bool),
    /// Move the caret right one word boundary; `true` extends selection.
    WordRight(bool),
    /// Delete previous word before cursor.
    BackspaceWord,
    /// Delete next word after cursor.
    DeleteWord,
    /// Clear all text and selection.
    Clear,
    /// Set the whole text, parking caret at the end.
    SetText(String),
    /// Move the caret to the start; `true` extends the selection.
    Home,
    /// Move the caret to the end; `true` extends the selection.
    End,
    /// Select the whole text.
    SelectAll,
}

/// The controlled state of one single-line field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextFieldState {
    text: String,
    cursor: usize,
    anchor: Option<usize>,
    placeholder: String,
    is_masked: bool,
    mask_char: char,
    is_readonly: bool,
    is_disabled: bool,
    validation: ValidationStatus,
    /// The active IME composition string.
    preedit: String,
    /// The IME preedit state machine managing CJK syllable segmentation.
    preedit_machine: PreeditStateMachine,
    /// Snapshot for active IME transaction rollback.
    ime_transaction: Option<ImeTransaction>,
}

impl TextFieldState {
    /// A field holding `text` with the caret at the end and no selection.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.chars().count();
        Self {
            text,
            cursor,
            anchor: None,
            placeholder: String::new(),
            is_masked: false,
            mask_char: '•',
            is_readonly: false,
            is_disabled: false,
            validation: ValidationStatus::Normal,
            preedit: String::new(),
            preedit_machine: PreeditStateMachine::new(),
            ime_transaction: None,
        }
    }

    /// Fluent builder for placeholder.
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Fluent builder for masked/password mode.
    pub fn with_masked(mut self, masked: bool) -> Self {
        self.is_masked = masked;
        self
    }

    /// Fluent builder for custom mask character.
    pub fn with_mask_char(mut self, mask_char: char) -> Self {
        self.mask_char = mask_char;
        self
    }

    /// Fluent builder for validation status.
    pub fn with_validation(mut self, validation: ValidationStatus) -> Self {
        self.validation = validation;
        self
    }

    /// Fluent builder for readonly state.
    pub fn with_readonly(mut self, readonly: bool) -> Self {
        self.is_readonly = readonly;
        self
    }

    /// Fluent builder for disabled state.
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }

    /// The controlled text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret as a char index into [`Self::text`].
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The selection as ordered `(start, end)` char indices, or `None`.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        if anchor == self.cursor {
            return None;
        }
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    /// Placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Whether placeholder is visible.
    pub fn is_placeholder_visible(&self) -> bool {
        self.text.is_empty() && self.preedit.is_empty() && !self.placeholder.is_empty()
    }

    /// Whether password masking is enabled.
    pub fn is_masked(&self) -> bool {
        self.is_masked
    }

    /// Masking character (defaults to '•').
    pub fn mask_char(&self) -> char {
        self.mask_char
    }

    /// Validation status.
    pub fn validation(&self) -> ValidationStatus {
        self.validation
    }

    /// Whether the field is read-only.
    pub fn is_readonly(&self) -> bool {
        self.is_readonly
    }

    /// Whether the field is disabled.
    pub fn is_disabled(&self) -> bool {
        self.is_disabled
    }

    /// Set placeholder.
    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = placeholder.into();
    }

    /// Set masked mode.
    pub fn set_masked(&mut self, masked: bool) {
        self.is_masked = masked;
    }

    /// Set validation status.
    pub fn set_validation(&mut self, validation: ValidationStatus) {
        self.validation = validation;
    }

    /// Set read-only.
    pub fn set_readonly(&mut self, readonly: bool) {
        self.is_readonly = readonly;
    }

    /// Set disabled.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.is_disabled = disabled;
    }

    /// The active IME composition string, if any.
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    /// Access the preedit state machine.
    pub fn preedit_machine(&self) -> &PreeditStateMachine {
        &self.preedit_machine
    }

    /// Access the mutable preedit state machine.
    pub fn preedit_machine_mut(&mut self) -> &mut PreeditStateMachine {
        &mut self.preedit_machine
    }

    /// Set the IME composition string.
    pub fn set_preedit(&mut self, preedit: impl Into<String>) -> bool {
        let preedit = preedit.into();
        if self.preedit == preedit {
            return false;
        }
        self.preedit = preedit.clone();
        self.preedit_machine.update(preedit, None);
        true
    }

    /// Clear the IME composition.
    pub fn clear_preedit(&mut self) -> bool {
        self.preedit_machine.cancel();
        self.set_preedit("")
    }

    /// Begin an IME transaction snapshot if none is currently active.
    pub fn begin_ime_transaction(&mut self) {
        if self.ime_transaction.is_none() {
            self.ime_transaction = Some(ImeTransaction {
                original_text: self.text.clone(),
                original_cursor: self.cursor,
                original_anchor: self.anchor,
            });
        }
    }

    /// Roll back the text field state to the snapshot taken when the IME transaction began.
    pub fn rollback_ime_transaction(&mut self) -> bool {
        self.preedit_machine.cancel();
        self.preedit.clear();
        if let Some(tx) = self.ime_transaction.take() {
            self.text = tx.original_text;
            self.cursor = tx.original_cursor;
            self.anchor = tx.original_anchor;
            true
        } else {
            false
        }
    }

    /// Commit the IME transaction.
    pub fn commit_ime_transaction(&mut self, committed_text: &str) -> bool {
        self.preedit_machine.commit();
        self.preedit.clear();
        if let Some(tx) = self.ime_transaction.take() {
            self.text = tx.original_text;
            self.cursor = tx.original_cursor;
            self.anchor = tx.original_anchor;
            self.insert(committed_text);
            true
        } else {
            self.insert(committed_text)
        }
    }

    /// Whether an IME transaction is currently active.
    pub fn is_in_ime_transaction(&self) -> bool {
        self.ime_transaction.is_some()
    }

    /// Apply one input, reporting whether the field changed.
    pub fn apply(&mut self, input: TextFieldInput) -> bool {
        if self.is_disabled {
            return false;
        }

        match input {
            TextFieldInput::Insert(inserted) => {
                if self.is_readonly {
                    false
                } else {
                    self.insert(&inserted)
                }
            }
            TextFieldInput::Backspace => {
                if self.is_readonly {
                    false
                } else {
                    self.safe_backspace()
                }
            }
            TextFieldInput::Delete => {
                if self.is_readonly {
                    false
                } else {
                    self.safe_delete()
                }
            }
            TextFieldInput::Left(extend) => self.move_left(extend),
            TextFieldInput::Right(extend) => self.move_right(extend),
            TextFieldInput::WordLeft(extend) => self.move_word_left(extend),
            TextFieldInput::WordRight(extend) => self.move_word_right(extend),
            TextFieldInput::BackspaceWord => {
                if self.is_readonly {
                    false
                } else {
                    self.backspace_word()
                }
            }
            TextFieldInput::DeleteWord => {
                if self.is_readonly {
                    false
                } else {
                    self.delete_word()
                }
            }
            TextFieldInput::Clear => {
                if self.is_readonly {
                    false
                } else {
                    self.clear_all()
                }
            }
            TextFieldInput::SetText(new_text) => {
                if self.is_readonly {
                    false
                } else {
                    self.set_full_text(new_text)
                }
            }
            TextFieldInput::Home => self.move_to(0),
            TextFieldInput::End => self.move_to(self.text.chars().count()),
            TextFieldInput::SelectAll => self.select_all(),
        }
    }

    /// Safely remove the Unicode extended grapheme cluster before the cursor.
    pub fn safe_backspace(&mut self) -> bool {
        if self.remove_selected() {
            return true;
        }
        if self.cursor == 0 || self.text.is_empty() {
            return false;
        }

        let head: String = self.text.chars().take(self.cursor).collect();
        let tail: String = self.text.chars().skip(self.cursor).collect();

        let grapheme_indices: Vec<(usize, &str)> = head.grapheme_indices(true).collect();
        if let Some((byte_offset, last_grapheme)) = grapheme_indices.last() {
            let grapheme_chars = last_grapheme.chars().count();
            let new_head = &head[..*byte_offset];
            self.text = format!("{new_head}{tail}");
            self.cursor = self.cursor.saturating_sub(grapheme_chars);
            self.anchor = None;
            true
        } else {
            false
        }
    }

    /// Safely remove the Unicode extended grapheme cluster at/after the cursor.
    pub fn safe_delete(&mut self) -> bool {
        if self.remove_selected() {
            return true;
        }
        let total_chars = self.text.chars().count();
        if self.cursor >= total_chars {
            return false;
        }

        let head: String = self.text.chars().take(self.cursor).collect();
        let tail: String = self.text.chars().skip(self.cursor).collect();

        let mut graphemes = tail.graphemes(true);
        if let Some(first_grapheme) = graphemes.next() {
            let grapheme_bytes = first_grapheme.len();
            let new_tail = &tail[grapheme_bytes..];
            self.text = format!("{head}{new_tail}");
            self.anchor = None;
            true
        } else {
            false
        }
    }

    fn move_word_left(&mut self, extend: bool) -> bool {
        if !extend {
            self.anchor = None;
        } else if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        let target = find_prev_word_boundary(&self.text, self.cursor);
        if target == self.cursor {
            return false;
        }
        self.cursor = target;
        true
    }

    fn move_word_right(&mut self, extend: bool) -> bool {
        if !extend {
            self.anchor = None;
        } else if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        let target = find_next_word_boundary(&self.text, self.cursor);
        if target == self.cursor {
            return false;
        }
        self.cursor = target;
        true
    }

    fn backspace_word(&mut self) -> bool {
        if self.remove_selected() {
            return true;
        }
        if self.cursor == 0 || self.text.is_empty() {
            return false;
        }
        let start = find_prev_word_boundary(&self.text, self.cursor);
        let head: String = self.text.chars().take(start).collect();
        let tail: String = self.text.chars().skip(self.cursor).collect();
        self.text = head + &tail;
        self.cursor = start;
        self.anchor = None;
        true
    }

    fn delete_word(&mut self) -> bool {
        if self.remove_selected() {
            return true;
        }
        let total_chars = self.text.chars().count();
        if self.cursor >= total_chars {
            return false;
        }
        let end = find_next_word_boundary(&self.text, self.cursor);
        let head: String = self.text.chars().take(self.cursor).collect();
        let tail: String = self.text.chars().skip(end).collect();
        self.text = head + &tail;
        self.anchor = None;
        true
    }

    fn clear_all(&mut self) -> bool {
        if self.text.is_empty() && self.anchor.is_none() && self.cursor == 0 {
            return false;
        }
        self.text.clear();
        self.cursor = 0;
        self.anchor = None;
        true
    }

    fn set_full_text(&mut self, new_text: String) -> bool {
        if self.text == new_text {
            return false;
        }
        let count = new_text.chars().count();
        self.text = new_text;
        self.cursor = count;
        self.anchor = None;
        true
    }

    fn selected_range(&self) -> Option<(usize, usize)> {
        self.selection()
    }

    fn insert(&mut self, inserted: &str) -> bool {
        let count = inserted.chars().count();
        let kept = self.remove_selected();
        let head = self.text.chars().take(self.cursor).collect::<String>();
        let tail = self.text.chars().skip(self.cursor).collect::<String>();
        self.text = head + inserted + &tail;
        self.cursor += count;
        self.anchor = None;
        kept || !inserted.is_empty()
    }

    fn move_left(&mut self, extend: bool) -> bool {
        if !extend {
            self.anchor = None;
        } else if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        true
    }

    fn move_right(&mut self, extend: bool) -> bool {
        if !extend {
            self.anchor = None;
        } else if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        if self.cursor >= self.text.chars().count() {
            return false;
        }
        self.cursor += 1;
        true
    }

    fn move_to(&mut self, index: usize) -> bool {
        self.anchor = None;
        if self.cursor == index {
            return false;
        }
        self.cursor = index;
        true
    }

    fn select_all(&mut self) -> bool {
        let len = self.text.chars().count();
        if len == 0 || self.selection() == Some((0, len)) {
            return false;
        }
        self.anchor = Some(0);
        self.cursor = len;
        true
    }

    /// Drop any selection, reporting whether one existed.
    fn remove_selected(&mut self) -> bool {
        let Some((start, end)) = self.selected_range() else {
            return false;
        };
        let head = self.text.chars().take(start).collect::<String>();
        let tail = self.text.chars().skip(end).collect::<String>();
        self.text = head + &tail;
        self.cursor = start;
        self.anchor = None;
        true
    }
}

/// The visible decomposition of one field state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldVisual {
    pub before: String,
    pub selected: String,
    pub after: String,
    pub caret_slot: usize,
    pub placeholder: String,
    pub is_placeholder_visible: bool,
}

/// Project a controlled state onto visible runs. Pure function.
pub fn field_visual(state: &TextFieldState) -> FieldVisual {
    let chars: Vec<char> = if state.is_masked {
        state.text.chars().map(|_| state.mask_char).collect()
    } else {
        state.text.chars().collect()
    };
    let len = chars.len();
    let cursor = state.cursor.min(len);
    let (start, end) = state.selection().unwrap_or((cursor, cursor));
    let caret_slot = usize::from(cursor > start);
    let slice = |from: usize, to: usize| chars[from.min(len)..to.min(len)].iter().collect();
    FieldVisual {
        before: slice(0, start),
        selected: slice(start, end),
        after: slice(end, len),
        caret_slot,
        placeholder: state.placeholder.clone(),
        is_placeholder_visible: state.is_placeholder_visible(),
    }
}
