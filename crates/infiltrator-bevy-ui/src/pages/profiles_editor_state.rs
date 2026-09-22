//! DUAL-09-03/05/14: pure state of the Bevy profile document editor.
//!
//! Split out of `profiles_editor.rs` to keep both files inside the business
//! line budget: this module owns the open buffer, the live shared-preflight
//! verdict and the formatter state machine; the scene module owns the `bsn!`
//! composition, the keyboard seam and the command submission.

use bevy::color::Color;
use bevy::ecs::resource::Resource;
use infiltrator_bevy_widgets::editor::CodeEditorState;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::editor_viewport::{EditorViewport, line_indent_level};
use infiltrator_contract::profile_document::{ProfileDocumentSnapshot, SyntaxDiagnosticSnapshot};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_domain::config::preflight_yaml_syntax;
use infiltrator_domain::yaml_edit::format::format_yaml;

use crate::pages::profiles::ProfilesProjection;

/// Maximum number of lines rendered per frame (no virtual scroll; see the
/// scene module docs). The window follows the cursor, so editing never leaves
/// the window, and it is the `window_lines` parameter of the *shared*
/// [`EditorViewport`] — the same window arithmetic the Iced editor uses.
pub const PROFILE_EDITOR_RENDER_LIMIT: usize = 240;

/// Surface-local editor state. The document itself stays owned by the shared
/// application projection; this is only the open buffer plus the live
/// preflight verdict.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ProfileEditorState {
    pub profile: String,
    pub buffer: CodeEditorState,
    /// The keyboard is routed here only while focused (explicit seam).
    pub focused: bool,
    pub dirty: bool,
    /// Content the shared application last published (used to detect new loads).
    pub loaded_content: Option<String>,
    /// Live shared-preflight verdict for the current buffer.
    pub diagnostic: Option<SyntaxDiagnosticSnapshot>,
    /// Honest note: formatter refusals and skipped layout rules.
    pub notice: Option<String>,
    /// Explicit unlock toggle for a protected subscription.
    pub protection_override: bool,
    /// Bumped on every buffer mutation; the body rebuilds when it moved.
    pub generation: u64,
    /// Last generation the scene body was rendered for.
    pub last_rendered: u64,
}

impl ProfileEditorState {
    /// Adopt a document published by the shared application.
    pub fn load_document(&mut self, document: &ProfileDocumentSnapshot) -> bool {
        let changed = self.loaded_content.as_deref() != Some(document.content.as_str());
        self.profile = document.profile.clone();
        if changed {
            self.buffer = CodeEditorState::new(&document.content);
            self.loaded_content = Some(document.content.clone());
            self.dirty = false;
            self.generation += 1;
        }
        self.diagnostic = document.syntax.clone();
        changed
    }

    /// Run the shared preflight over the buffer (called after every edit).
    pub fn refresh_preflight(&mut self) {
        self.diagnostic = preflight_yaml_syntax(&self.buffer.full_text())
            .err()
            .map(|diagnostic| SyntaxDiagnosticSnapshot {
                line: diagnostic.line,
                column: diagnostic.column,
                message: diagnostic.message,
            });
    }

    fn after_edit(&mut self) {
        self.dirty = true;
        self.generation += 1;
        self.refresh_preflight();
    }

    pub fn insert_text(&mut self, text: &str) {
        self.buffer.insert_text(text);
        self.after_edit();
    }

    pub fn backspace(&mut self) {
        self.buffer.delete_backwards();
        self.after_edit();
    }

    pub fn delete_forward(&mut self) {
        self.buffer.delete_forward();
        self.after_edit();
    }

    pub fn move_cursor(&mut self, up: bool) {
        if up {
            self.buffer.move_up();
        } else {
            self.buffer.move_down();
        }
        self.generation += 1;
    }

    /// Format through the shared formatter; a refusal keeps the user's bytes.
    pub fn format(&mut self) -> Result<(), String> {
        let text = self.buffer.full_text();
        match format_yaml(&text) {
            Ok(report) => {
                let cursor_row = self.buffer.cursor_row;
                self.buffer = CodeEditorState::new(&report.content);
                self.buffer.cursor_row = cursor_row.min(self.buffer.line_count().saturating_sub(1));
                self.notice = report
                    .skip_reason()
                    .map(|reason| reason.label_zh().to_owned());
                self.after_edit();
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                self.notice = Some(message.clone());
                Err(message)
            }
        }
    }

    /// Whether the buffer may be saved right now.
    pub fn can_save(&self, protection: ProfileWriteProtection) -> bool {
        !self.profile.is_empty() && (!protection.is_protected() || self.protection_override)
    }

    /// DUAL-09-02: the shared window the body renders. The caret follows the
    /// window through the same rule the Iced surface uses.
    pub fn viewport(&self) -> EditorViewport {
        EditorViewport::new(self.buffer.line_count(), PROFILE_EDITOR_RENDER_LIMIT, 0)
            .follow_caret(self.buffer.cursor_row + 1)
    }

    /// The lines of the rendered window, with their shared indentation levels.
    pub fn rendered_lines(&self) -> Vec<(usize, &str, usize)> {
        let viewport = self.viewport();
        (viewport.first_line()..=viewport.last_line())
            .filter_map(|index| {
                self.buffer
                    .lines
                    .get(index)
                    .map(|line| (index + 1, line.as_str(), line_indent_level(line)))
            })
            .collect()
    }

    /// DUAL-09-04: insert a catalogue snippet at the caret through the shared
    /// application use-case — the same splice and the same preflight gate the
    /// Iced surface runs.
    pub fn insert_snippet(&mut self, snippet_id: &str) -> Result<(), String> {
        let text = self.buffer.full_text();
        let caret_line = self.buffer.cursor_row.min(self.buffer.line_count() - 1);
        let line_text = self.buffer.lines[caret_line].as_str();
        let caret_column = infiltrator_contract::yaml_snippets::column_of_byte_offset(
            line_text,
            self.buffer.cursor_col,
        );
        match infiltrator_application::profile_document_application::insert_snippet(
            &text,
            snippet_id,
            caret_line + 1,
            caret_column,
        ) {
            Ok(insertion) => {
                let cursor_row = insertion.cursor_line - 1;
                self.buffer = CodeEditorState::new(&insertion.content);
                self.buffer.cursor_row = cursor_row.min(self.buffer.line_count().saturating_sub(1));
                let inserted_line = &self.buffer.lines[self.buffer.cursor_row];
                self.buffer.cursor_col = infiltrator_contract::yaml_snippets::byte_offset_of_column(
                    inserted_line,
                    insertion.cursor_column,
                );
                self.notice = insertion.syntax.as_ref().map(|diagnostic| {
                    format!("{} · {}", diagnostic.line_label(), diagnostic.message)
                });
                self.after_edit();
                Ok(())
            }
            Err(failure) => {
                self.notice = Some(failure.message.clone());
                Err(failure.message)
            }
        }
    }
}

pub(crate) fn status_line(
    state: &ProfileEditorState,
    projection: Option<&ProfilesProjection>,
) -> String {
    let mut parts = vec![if state.dirty {
        "有未保存修改".to_owned()
    } else {
        "与共享配置一致".to_owned()
    }];
    parts.push(format!(
        "{} 行 · 光标 {}:{}",
        state.buffer.line_count(),
        state.buffer.cursor_row + 1,
        state.buffer.cursor_col + 1
    ));
    // DUAL-09-02: the shared window is stated, not implied — a windowed
    // document says which lines it is rendering right now.
    let viewport = state.viewport();
    if !viewport.covers_document() {
        parts.push(viewport.range_label());
    }
    if state.focused {
        parts.push("键盘已接管".to_owned());
    }
    if let Some(record) = projection.and_then(|projection| projection.apply_transaction.as_ref()) {
        parts.push(record.summary_zh());
    }
    parts.join(" · ")
}

pub(crate) fn diagnostic_line(state: &ProfileEditorState) -> (String, bool) {
    match state.diagnostic.as_ref() {
        Some(diagnostic) => (
            format!("{} · {}", diagnostic.line_label(), diagnostic.message),
            true,
        ),
        None => ("语法正确（共享预检实时通过）".to_owned(), false),
    }
}

pub(crate) fn protection_toggle_visual(
    protection: ProfileWriteProtection,
    unlocked: bool,
    palette: &UiPalette,
) -> (String, Color) {
    if !protection.is_protected() {
        return ("本地配置：可直接保存".to_owned(), palette.surface_elevated);
    }
    if unlocked {
        ("已解锁：点击恢复只读".to_owned(), palette.accent)
    } else {
        ("显式解锁编辑".to_owned(), palette.warning)
    }
}
