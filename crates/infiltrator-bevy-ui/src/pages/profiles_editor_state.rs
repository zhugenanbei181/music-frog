//! DUAL-09-03/05/14: pure state of the Bevy profile document editor.
//!
//! Split out of `profiles_editor.rs` to keep both files inside the business
//! line budget: this module owns the open buffer, the snapshot inspection,
//! codec-authored buffer validation and formatter state; the scene owns the `bsn!`
//! composition, the keyboard seam and the command submission.

use crate::pages::profiles::ProfilesProjection;
use bevy::color::Color;
use bevy::ecs::resource::Resource;
use bevy::input::keyboard::Key;
use infiltrator_bevy_widgets::editor::state::CodeEditorState;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::editor_viewport::{EditorViewport, line_indent_level};
use infiltrator_contract::profile_document::{
    DocumentInspection, DocumentInspectionUnavailableReason, ProfileDocumentSnapshot,
};
use infiltrator_contract::profile_mixin_studio::ProfileMixinPreflight;
use infiltrator_contract::yaml_snippets::{
    SyntaxDiagnosticSnapshot, byte_offset_of_column, column_of_byte_offset,
};
use infiltrator_domain::yaml_edit::format::{FormatSkipReason, format_yaml};
use infiltrator_presentation::profile_edit_session::{ProfileEditObservation, ProfileEditSession};
use infiltrator_presentation::profile_edit_status_projection;
use infiltrator_presentation::profile_editor_projection::{self, EditorStatus};
use infiltrator_shared::locales::Lang;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::profile_document::{DocumentDiagnosticSnapshot, DocumentValidation};
use lurker_core_contract::profile_protection::{ProfileWriteAuthorization, ProfileWriteProtection};
use lurker_core_ports::profile_document_codec::ProfileDocumentSnippetOutcome;

/// Maximum number of lines rendered per frame (no virtual scroll; see the
/// scene module docs). The window follows the cursor, so editing never leaves
/// the window, and it is the `window_lines` parameter of the *shared*
/// [`EditorViewport`] — the same window arithmetic the Iced editor uses.
pub const PROFILE_EDITOR_RENDER_LIMIT: usize = 240;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProfileValidationSource {
    #[default]
    Snapshot,
    LocalYamlBuffer,
    MixinDraft,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MixinPreflightState {
    #[default]
    NotRequested,
    Accepted,
    Blocked(Failure),
    Unavailable(Failure),
}

/// Surface-local editor state. The document itself stays owned by the shared
/// application projection; this is only the open buffer plus the live
/// preflight verdict.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ProfileEditorState {
    pub profile: String,
    pub session: ProfileEditSession,
    pub protection: ProfileWriteProtection,
    pub save_request: Option<(RequestId, u64)>,
    pub buffer: CodeEditorState,
    /// The keyboard is routed here only while focused (explicit seam).
    pub focused: bool,
    pub dirty: bool,
    /// Content the shared application last published (used to detect new loads).
    pub loaded_content: Option<String>,
    /// Live identity-scoped codec inspection diagnostic for the current buffer.
    pub diagnostic: Option<SyntaxDiagnosticSnapshot>,
    pub document_inspection: Option<DocumentInspection>,
    pub mixin_preflight: MixinPreflightState,
    pub validation_source: ProfileValidationSource,
    /// Honest note: formatter refusals and skipped layout rules.
    pub notice: Option<String>,
    pub format_note: Option<FormatSkipReason>,
    /// Explicit unlock toggle for a protected subscription.
    pub write_authorization: ProfileWriteAuthorization,
    /// Bumped on every buffer mutation; the body rebuilds when it moved.
    pub generation: u64,
    /// Last generation the scene body was rendered for.
    pub last_rendered: u64,
}

impl ProfileEditorState {
    /// Adopt a document published by the shared application.
    pub fn load_document(&mut self, document: &ProfileDocumentSnapshot) -> ProfileEditObservation {
        if self.protection != document.write_protection {
            self.write_authorization = ProfileWriteAuthorization::RequireUnlock;
        }
        self.protection = document.write_protection;
        let observation = self.session.observe(
            &document.workspace_identity,
            &document.content,
            &self.buffer.full_text(),
        );
        if matches!(observation, ProfileEditObservation::Updated) {
            self.profile = document
                .workspace_identity
                .document
                .profile_id
                .as_str()
                .to_owned();
            self.buffer = CodeEditorState::new(&document.content);
            self.loaded_content = Some(document.content.clone());
            self.dirty = false;
            self.write_authorization = ProfileWriteAuthorization::RequireUnlock;
            self.generation += 1;
            self.document_inspection = Some(document.inspection.clone());
            self.mixin_preflight = MixinPreflightState::NotRequested;
            self.validation_source = ProfileValidationSource::Snapshot;
            self.diagnostic = None;
        }
        observation
    }

    /// Adopt the neutral result returned by the selected document codec.
    pub fn record_profile_inspection(&mut self, inspection: DocumentInspection) {
        self.validation_source = ProfileValidationSource::LocalYamlBuffer;
        self.diagnostic = match &inspection {
            DocumentInspection::Inspected(DocumentValidation::Invalid { diagnostics }) => {
                diagnostics.first().and_then(|diagnostic| {
                    Some(SyntaxDiagnosticSnapshot {
                        line: diagnostic.line?,
                        column: diagnostic.column.unwrap_or_default(),
                        message: diagnostic.message.clone(),
                    })
                })
            }
            DocumentInspection::Inspected(DocumentValidation::Valid)
            | DocumentInspection::Unavailable(_)
            | DocumentInspection::Unsupported(_) => None,
        };
        self.document_inspection = Some(inspection);
    }

    /// Adopt the neutral, engine-scoped Mixin preflight returned by the codec.
    pub fn record_mixin_preflight(&mut self, result: Result<(), Failure>) {
        self.validation_source = ProfileValidationSource::MixinDraft;
        self.mixin_preflight = match result {
            Ok(()) => {
                self.document_inspection =
                    Some(DocumentInspection::Inspected(DocumentValidation::Valid));
                self.diagnostic = None;
                MixinPreflightState::Accepted
            }
            Err(failure)
                if matches!(
                    &failure.code,
                    ErrorCode::InvalidInput | ErrorCode::Configuration
                ) =>
            {
                self.document_inspection =
                    Some(DocumentInspection::Inspected(DocumentValidation::Invalid {
                        diagnostics: vec![DocumentDiagnosticSnapshot {
                            line: None,
                            column: None,
                            message: failure.message.clone(),
                        }],
                    }));
                self.diagnostic = None;
                MixinPreflightState::Blocked(failure)
            }
            Err(failure) => {
                self.document_inspection = Some(DocumentInspection::Unavailable(
                    DocumentInspectionUnavailableReason::AdapterUnavailable {
                        detail: failure.message.clone(),
                    },
                ));
                self.diagnostic = None;
                MixinPreflightState::Unavailable(failure)
            }
        };
    }

    pub fn record_mixin_preflight_status(&mut self, preflight: &ProfileMixinPreflight) {
        match preflight {
            ProfileMixinPreflight::Accepted => self.record_mixin_preflight(Ok(())),
            ProfileMixinPreflight::Blocked { reason } => self.record_mixin_preflight(Err(
                Failure::new(ErrorCode::Configuration, reason.clone(), false),
            )),
        }
    }

    fn after_edit(&mut self) {
        self.dirty = true;
        self.generation += 1;
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

    pub fn move_left(&mut self) {
        self.buffer.move_left();
        self.generation += 1;
    }

    pub fn move_right(&mut self) {
        self.buffer.move_right();
        self.generation += 1;
    }

    pub fn move_home(&mut self) {
        self.buffer.move_home();
        self.generation += 1;
    }

    pub fn move_end(&mut self) {
        self.buffer.move_end();
        self.generation += 1;
    }

    /// DUAL-09-14: apply one unmodified key press to the buffer. The Profile
    /// and Mixin panes share this rule set through the single keyboard seam,
    /// so the two document panes can never diverge in editing behavior.
    /// Returns `true` when the key was consumed.
    pub fn apply_key(&mut self, key: &Key) -> bool {
        match key {
            Key::Character(text) => self.insert_text(text),
            Key::Space => self.insert_text(" "),
            Key::Enter => {
                self.buffer.insert_newline();
                self.after_edit();
            }
            Key::Tab => self.insert_text("  "),
            Key::Backspace => self.backspace(),
            Key::Delete => self.delete_forward(),
            Key::ArrowUp => self.move_cursor(true),
            Key::ArrowDown => self.move_cursor(false),
            Key::ArrowLeft => self.move_left(),
            Key::ArrowRight => self.move_right(),
            Key::Home => self.move_home(),
            Key::End => self.move_end(),
            Key::Escape => {
                self.focused = false;
                self.generation = self.generation.wrapping_add(1);
            }
            _ => return false,
        }
        true
    }

    /// Format through the shared formatter; a refusal keeps the user's bytes.
    pub fn format(&mut self) -> Result<(), String> {
        let text = self.buffer.full_text();
        match format_yaml(&text) {
            Ok(report) => {
                let cursor_row = self.buffer.cursor_row;
                self.buffer = CodeEditorState::new(&report.content);
                self.buffer.cursor_row = cursor_row.min(self.buffer.line_count().saturating_sub(1));
                self.format_note = report.skip_reason();
                self.notice = None;
                self.after_edit();
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                self.format_note = None;
                self.notice = Some(message.clone());
                Err(message)
            }
        }
    }

    /// Whether the buffer may be saved right now.
    pub fn can_save(&self, protection: ProfileWriteProtection) -> bool {
        self.session.can_save()
            && (!protection.is_protected() || self.write_authorization.allows_protected_write())
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

    /// Adopt a splice returned by the identity-scoped document codec.
    pub fn adopt_snippet(&mut self, outcome: ProfileDocumentSnippetOutcome) -> Result<(), String> {
        match outcome {
            ProfileDocumentSnippetOutcome::Inserted(insertion) => {
                let cursor_row = insertion.cursor_line - 1;
                self.buffer = CodeEditorState::new(&insertion.content);
                self.buffer.cursor_row = cursor_row.min(self.buffer.line_count().saturating_sub(1));
                let inserted_line = &self.buffer.lines[self.buffer.cursor_row];
                self.buffer.cursor_col =
                    byte_offset_of_column(inserted_line, insertion.cursor_column);
                self.record_profile_inspection(DocumentInspection::Inspected(insertion.validation));
                self.notice = None;
                self.after_edit();
                Ok(())
            }
            ProfileDocumentSnippetOutcome::Rejected { validation } => {
                let message = match &validation {
                    DocumentValidation::Invalid { diagnostics } => diagnostics
                        .first()
                        .map(|diagnostic| diagnostic.message.clone())
                        .unwrap_or_else(|| "Snippet would make the document invalid".into()),
                    DocumentValidation::Valid => "Snippet was rejected by the codec".into(),
                };
                self.record_profile_inspection(DocumentInspection::Inspected(validation));
                self.notice = Some(message.clone());
                Err(message)
            }
        }
    }
}

pub(crate) fn status_line(
    state: &ProfileEditorState,
    projection: Option<&ProfilesProjection>,
    locale: &str,
) -> String {
    let viewport = state.viewport();
    let status = profile_editor_projection::status(
        &EditorStatus {
            dirty: state.dirty,
            line_count: state.buffer.line_count(),
            cursor_line: state.buffer.cursor_row + 1,
            cursor_column: column_of_byte_offset(
                state.buffer.line_content(state.buffer.cursor_row),
                state.buffer.cursor_col,
            ) + 1,
            viewport: &viewport,
            focused: state.focused,
            notice: state.notice.as_deref(),
            transaction: projection.and_then(|projection| projection.apply_transaction.as_ref()),
        },
        &Lang(locale),
    );
    let session_status = profile_edit_status_projection::status(
        &state.session.status_fact(&state.buffer.full_text()),
        &Lang(locale),
    );
    [status, session_status]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}
pub(crate) fn diagnostic_line(
    state: &ProfileEditorState,
    locale: &str,
) -> (String, profile_editor_projection::DocumentValidationStatus) {
    match state.validation_source {
        ProfileValidationSource::Snapshot => profile_editor_projection::document_inspection(
            state.document_inspection.as_ref(),
            &Lang(locale),
        ),
        ProfileValidationSource::LocalYamlBuffer | ProfileValidationSource::MixinDraft => {
            profile_editor_projection::document_inspection(
                state.document_inspection.as_ref(),
                &Lang(locale),
            )
        }
    }
}

pub(crate) fn diagnostic_label(
    source: ProfileValidationSource,
    status: profile_editor_projection::DocumentValidationStatus,
    locale: &str,
) -> String {
    match source {
        ProfileValidationSource::Snapshot => {
            profile_editor_projection::document_validation_label(status, &Lang(locale))
        }
        ProfileValidationSource::LocalYamlBuffer => {
            profile_editor_projection::syntax_label(status, &Lang(locale))
        }
        ProfileValidationSource::MixinDraft => {
            profile_editor_projection::document_validation_label(status, &Lang(locale))
        }
    }
}

pub(crate) fn protection_toggle_visual(
    protection: ProfileWriteProtection,
    unlocked: bool,
    palette: &UiPalette,
    locale: &str,
) -> (String, Color) {
    let fill = if !protection.is_protected() {
        palette.surface_elevated
    } else if unlocked {
        palette.accent
    } else {
        palette.warning
    };
    (
        profile_editor_projection::protection_action(protection, unlocked, &Lang(locale)),
        fill,
    )
}
