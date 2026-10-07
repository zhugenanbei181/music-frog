//! Profile YAML editor handlers: load profile content into the editor,
//! editor actions and saving back to disk.

use crate::snapshot_commands::execute;
use crate::state::AppState;
use crate::types::app::{Route, ToastStatus};
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::types::profile_edit::{ProfileDocumentReadReply, ProfileEditReply};
use crate::types::snapshot_restore::RestoreAction;
use crate::view::editor_viewport::window_lines_for_window_height;
use iced::Task;
use iced::widget::text_editor;
use infiltrator_application::failure_projection::failure_message;
use infiltrator_application::profile_document_application::insert_snippet;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::editor_viewport::EditorViewport;
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason, InfiltratorError};
use infiltrator_contract::snapshot_history::{SNAPSHOT_DEFAULT_KEEP, SnapshotHistorySnapshot};
use infiltrator_contract::snapshot_restore::SnapshotRestoreTarget;
use infiltrator_contract::yaml_snippets::{byte_offset_of_column, column_of_byte_offset};
use infiltrator_domain::config::preflight_yaml_syntax;

/// Why the shared editor window is being re-synced (DUAL-09-02/13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ViewportSync {
    /// The widget published its own wheel delta and already applied it; the
    /// shared window mirrors the same clamped delta.
    Scrolled(i32),
    /// An action may have moved the caret; the window follows it into view.
    Caret,
}

impl AppState {
    /// Profile name of the document currently open in the editor.
    fn edited_profile(&self) -> Option<String> {
        self.editor
            .editor_path
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|name| name.to_str())
            .map(str::to_string)
    }

    /// The window height the document panes can afford, in lines (DUAL-09-02).
    fn document_window_lines(&self) -> usize {
        window_lines_for_window_height(self.shell.viewport.height_px)
    }
    /// DUAL-09-02/13: keep the shared window in step with the text widget.
    ///
    /// The widget owns its pixel scroll offset, so the surface mirrors the
    /// deltas the widget publishes and applies the same shared clamp. A window
    /// that moved without the widget moving (a caret step past the window
    /// edge) is written back through `Action::Scroll`, which is the only way
    /// Iced 0.14 lets a surface place the editor's viewport.
    pub(super) fn sync_document_viewport(&mut self, pane: EditorPane, source: ViewportSync) {
        let window_lines = self.document_window_lines();
        let (content, viewport) = match pane {
            EditorPane::Mixin => (
                &mut self.editor.mixin_content,
                &mut self.editor.mixin_viewport,
            ),
            EditorPane::Profile | EditorPane::Filter | EditorPane::Script => (
                &mut self.editor.editor_content,
                &mut self.editor.profile_viewport,
            ),
        };
        let line_count = content.line_count().max(1);
        let current = viewport.with_document(line_count, window_lines);
        let caret_line = content.cursor().position.line + 1;
        let next = match source {
            ViewportSync::Scrolled(delta) => current.scrolled(delta),
            ViewportSync::Caret => current.follow_caret(caret_line),
        };
        if next.first_line() != current.first_line() {
            let delta = next.first_line() as i64 - current.first_line() as i64;
            content.perform(text_editor::Action::Scroll {
                lines: delta as i32,
            });
        }
        *viewport = next;
    }

    /// The widget's own scroll restarts at line 0 whenever its content is
    /// replaced, so the shared window restarts there too and then re-reveals
    /// the caret. Also called from the format action in the UI domain, which
    /// replaces the profile buffer the same way.
    pub(crate) fn reset_document_viewport(&mut self, pane: EditorPane) {
        let window_lines = self.document_window_lines();
        let (content, viewport) = match pane {
            EditorPane::Mixin => (
                &mut self.editor.mixin_content,
                &mut self.editor.mixin_viewport,
            ),
            EditorPane::Profile | EditorPane::Filter | EditorPane::Script => (
                &mut self.editor.editor_content,
                &mut self.editor.profile_viewport,
            ),
        };
        *viewport = EditorViewport::top(content.line_count().max(1), window_lines);
        self.sync_document_viewport(pane, ViewportSync::Caret);
    }

    /// Live shared preflight on the profile buffer (same rule as the save gate).
    fn refresh_profile_preflight(&mut self) {
        let text = self.editor.editor_content.text();
        match preflight_yaml_syntax(&text) {
            Ok(()) => {
                self.editor.syntax_error = None;
                self.editor.syntax_error_line = None;
            }
            Err(diag) => {
                self.editor.syntax_error = Some(diag.message);
                self.editor.syntax_error_line = Some(diag.line);
            }
        }
    }

    /// DUAL-09-04: insert a catalogue snippet at the caret through the shared
    /// application use-case (byte-faithful splice + shared preflight gate).
    /// The active document pane receives it: the Mixin overlay edits its own
    /// document, every other pane edits the profile document.
    pub(super) fn insert_yaml_snippet(&mut self, snippet_id: &'static str) -> Task<Message> {
        let pane = self.editor.editor_pane;
        if (pane == EditorPane::Mixin && !self.editor.mixin_session.can_edit())
            || (pane != EditorPane::Mixin && !self.editor.document_session.can_edit())
        {
            return Task::none();
        }
        let content = match pane {
            EditorPane::Mixin => self.editor.mixin_content.text(),
            EditorPane::Profile | EditorPane::Filter | EditorPane::Script => {
                self.editor.editor_content.text()
            }
        };
        // The widget reports a byte column; the shared caret is a character
        // column (see the contract's conversions).
        let widget_cursor = match pane {
            EditorPane::Mixin => self.editor.mixin_content.cursor(),
            EditorPane::Profile | EditorPane::Filter | EditorPane::Script => {
                self.editor.editor_content.cursor()
            }
        };
        let caret_line = widget_cursor.position.line;
        let line_text = content.split('\n').nth(caret_line).unwrap_or_default();
        let caret_column = column_of_byte_offset(line_text, widget_cursor.position.column);
        match insert_snippet(&content, snippet_id, caret_line + 1, caret_column) {
            Ok(insertion) => {
                let inserted_line = insertion
                    .content
                    .split('\n')
                    .nth(insertion.cursor_line - 1)
                    .unwrap_or_default();
                let cursor = text_editor::Cursor {
                    position: text_editor::Position {
                        line: insertion.cursor_line - 1,
                        column: byte_offset_of_column(inserted_line, insertion.cursor_column),
                    },
                    selection: None,
                };
                match pane {
                    EditorPane::Mixin => {
                        self.editor.mixin_content =
                            text_editor::Content::with_text(&insertion.content);
                        self.editor.mixin_content.move_to(cursor);
                    }
                    EditorPane::Profile | EditorPane::Filter | EditorPane::Script => {
                        self.editor.editor_content =
                            text_editor::Content::with_text(&insertion.content);
                        self.editor.editor_content.move_to(cursor);
                    }
                }
                self.reset_document_viewport(pane);
                if pane == EditorPane::Profile {
                    self.refresh_profile_preflight();
                }
                Task::none()
            }
            Err(failure) => Task::done(Message::ShowToast(
                failure_message(&failure, &self.shell.lang),
                ToastStatus::Error,
            )),
        }
    }

    /// DUAL-09-11: mirror the host core's typed apply outcome into the editor
    /// banner. The record is the same fact the shared projection carries.
    fn refresh_apply_transaction(&mut self) {
        let profile = self.editor_profile_name();
        self.editor.apply_transaction = self
            .surface
            .latest()
            .and_then(|snapshot| snapshot.pages.profiles.data.as_ref())
            .and_then(|page| page.apply_transaction.as_ref())
            .filter(|record| Some(&record.profile) == profile.as_ref())
            .cloned();
    }

    pub(super) fn update_editor(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EditProfileAs(path, pane) => {
                self.editor.editor_pane = pane;
                self.update_editor(Message::EditProfile(path))
            }
            Message::EditProfile(path) => {
                let Some(profile) = path
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
                else {
                    return Task::none();
                };
                self.editor.next_editor_read = self
                    .editor
                    .next_editor_read
                    .checked_add(1)
                    .expect("editor read identity exhausted");
                let ticket = self.editor.next_editor_read;
                self.editor.document_load = Some((ticket, path.clone()));
                let commands = self.commands.clone();
                Task::perform(
                    async move {
                        let result = match commands {
                            Some(commands) => commands
                                .execute(CommandIntent::LoadProfileDocument {
                                    profile: Some(profile),
                                })
                                .await
                                .into_output()
                                .and_then(CommandOutput::into_profile_document),
                            None => Err(Failure::new(
                                ErrorCode::NotReady,
                                "Profile editor command service is unavailable",
                                true,
                            )),
                        };
                        ProfileDocumentReadReply {
                            ticket,
                            path,
                            result,
                        }
                    },
                    Message::ProfileContentLoaded,
                )
            }
            Message::ProfileContentLoaded(reply) => {
                if self.editor.document_load.as_ref() != Some(&(reply.ticket, reply.path.clone())) {
                    return Task::none();
                }
                self.editor.document_load = None;
                let document = match reply.result {
                    Ok(document) => document,
                    Err(failure) => {
                        self.editor.document_session.read_failed(failure);
                        return Task::none();
                    }
                };
                let Some(source) = document.source.as_ref() else {
                    return Task::none();
                };
                let adopt = self.editor.document_session.observe(
                    source,
                    &document.content,
                    &self.editor.editor_content.text(),
                );
                self.editor.document_session.read_completed(source);
                self.editor.document_latest = Some((reply.path.clone(), document.clone()));
                if adopt {
                    self.editor.editor_path = Some(reply.path);
                    self.editor.editor_content = text_editor::Content::with_text(&document.content);
                    self.editor.profile_protection_override = false;
                    self.reset_document_viewport(EditorPane::Profile);
                }
                let mut tasks = vec![
                    Task::done(Message::Navigate(Route::Editor)),
                    Task::done(Message::LoadProfileSnapshots),
                ];
                match self.editor.editor_pane {
                    EditorPane::Mixin => tasks.push(self.ensure_mixin_loaded()),
                    EditorPane::Filter => tasks.push(self.ensure_filter_loaded()),
                    _ => {}
                }
                Task::batch(tasks)
            }
            Message::DiscardProfileDraft => {
                if let Some(content) = self.editor.document_session.discard() {
                    self.editor.editor_content = text_editor::Content::with_text(&content);
                    if let Some((path, _)) = &self.editor.document_latest {
                        self.editor.editor_path = Some(path.clone());
                    }
                    self.editor.profile_protection_override = false;
                    self.reset_document_viewport(EditorPane::Profile);
                    self.refresh_profile_preflight();
                }
                Task::none()
            }
            Message::EditorAction(action) => {
                if !self.editor.document_session.can_edit() {
                    return Task::none();
                }
                let scroll_delta = match &action {
                    text_editor::Action::Scroll { lines } => Some(*lines),
                    _ => None,
                };
                self.editor.editor_content.perform(action);
                // DUAL-09-02/13: the widget's own scroll and the shared window
                // are synced from the same deltas, so the gutter always shows
                // the lines the widget rendered.
                match scroll_delta {
                    Some(delta) => self
                        .sync_document_viewport(EditorPane::Profile, ViewportSync::Scrolled(delta)),
                    None => self.sync_document_viewport(EditorPane::Profile, ViewportSync::Caret),
                }
                self.refresh_profile_preflight();
                Task::none()
            }
            // DUAL-09-04: the snippet bar dispatches the catalogue id.
            Message::InsertYamlSnippet(snippet_id) => self.insert_yaml_snippet(snippet_id),
            Message::LoadProfileSnapshots => {
                let Some(profile) = self.edited_profile() else {
                    self.editor.snapshot_history = None;
                    return Task::none();
                };
                let commands = self.commands.clone();
                self.editor.is_loading_snapshots = true;
                Task::perform(
                    async move {
                        execute(
                            commands,
                            CommandIntent::LoadSnapshotHistory {
                                profile: Some(profile),
                                keep: SNAPSHOT_DEFAULT_KEEP,
                            },
                        )
                        .await?
                        .into_snapshot_history()
                        .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::ProfileSnapshotsLoaded,
                )
            }
            Message::ProfileSnapshotsLoaded(result) => {
                self.editor.is_loading_snapshots = false;
                match result {
                    Ok(history) => self.editor.snapshot_history = Some(history),
                    Err(error) => self.set_error(&error),
                }
                Task::none()
            }
            // DUAL-09-06: manual backup through the same snapshot application
            // the apply transaction uses.
            Message::BackupProfileSnapshot => {
                if self.editor.is_backing_up_snapshot {
                    return Task::none();
                }
                let Some(profile) = self.edited_profile() else {
                    return Task::none();
                };
                let commands = self.commands.clone();
                self.editor.is_backing_up_snapshot = true;
                Task::perform(
                    async move {
                        execute(
                            commands,
                            CommandIntent::CreateBackupSnapshot {
                                profile: Some(profile),
                            },
                        )
                        .await
                        .map(|_| ())
                    },
                    Message::ProfileSnapshotBackedUp,
                )
            }
            Message::ProfileSnapshotBackedUp(result) => {
                self.editor.is_backing_up_snapshot = false;
                match result {
                    Ok(()) => Task::batch(vec![
                        Task::done(Message::LoadProfileSnapshots),
                        Task::done(Message::ShowToast(
                            "Snapshot stored".to_string(),
                            ToastStatus::Success,
                        )),
                    ]),
                    Err(error) => {
                        self.set_error(&error);
                        Task::none()
                    }
                }
            }
            // DUAL-09-07: the retention selection and the shared prune command.
            Message::SetSnapshotPruneKeep(keep) => {
                self.editor.snapshot_prune_keep = SnapshotHistorySnapshot::clamp_keep(keep);
                Task::none()
            }
            Message::PruneProfileSnapshots => {
                if self.editor.is_pruning_snapshots {
                    return Task::none();
                }
                let Some(profile) = self.edited_profile() else {
                    return Task::none();
                };
                let keep = self.editor.snapshot_prune_keep;
                let commands = self.commands.clone();
                self.editor.is_pruning_snapshots = true;
                Task::perform(
                    async move {
                        execute(
                            commands,
                            CommandIntent::PruneSnapshots {
                                profile: Some(profile),
                                keep: Some(keep),
                            },
                        )
                        .await?
                        .into_snapshots_pruned()
                        .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::ProfileSnapshotsPruned,
                )
            }
            Message::ProfileSnapshotsPruned(result) => {
                self.editor.is_pruning_snapshots = false;
                match result {
                    Ok(report) => Task::batch(vec![
                        Task::done(Message::LoadProfileSnapshots),
                        Task::done(Message::ShowToast(
                            format!("Pruned {} snapshots", report.removed),
                            ToastStatus::Success,
                        )),
                    ]),
                    Err(error) => {
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::ArmRestoreProfileSnapshot(path) | Message::RestoreProfileSnapshot(path) => {
                let Some(profile) = self.edited_profile() else {
                    return Task::none();
                };
                self.update_snapshot_restore(RestoreAction::Open(SnapshotRestoreTarget {
                    profile,
                    snapshot_id: path.to_string_lossy().into_owned(),
                }))
            }
            Message::CancelRestoreProfileSnapshot => {
                self.update_snapshot_restore(RestoreAction::Cancel)
            }
            Message::SaveProfile => {
                if self.profile.is_saving_profile {
                    return Task::none();
                }
                let content = self.editor.editor_content.text();
                if let Err(diag) = preflight_yaml_syntax(&content) {
                    self.editor.syntax_error = Some(diag.message.clone());
                    self.editor.syntax_error_line = Some(diag.line);
                    return Task::done(Message::ShowToast(
                        failure_message(
                            &Failure::new(ErrorCode::Configuration, diag.message, false)
                                .with_reason(FailureReason::YamlSyntax {
                                    line: diag.line,
                                    column: diag.column,
                                }),
                            &self.shell.lang,
                        ),
                        ToastStatus::Error,
                    ));
                }
                let pending = match self
                    .editor
                    .document_session
                    .begin_document(content, self.editor.profile_protection_override)
                {
                    Ok(pending) => pending,
                    Err(failure) => {
                        self.editor.document_session.failure = Some(failure);
                        return Task::none();
                    }
                };
                self.profile.is_saving_profile = true;
                let commands = self.commands.clone();
                Task::perform(
                    async move {
                        let result = match commands {
                            Some(commands) => {
                                commands.execute(pending.intent.clone()).await.into_output()
                            }
                            None => Err(Failure::new(
                                ErrorCode::NotReady,
                                "Profile editor command service is unavailable",
                                true,
                            )),
                        };
                        ProfileEditReply { pending, result }
                    },
                    Message::ProfileSaved,
                )
            }
            Message::ProfileSaved(reply) => {
                if !self.editor.document_session.finish(
                    reply.pending.operation,
                    &reply.pending.intent,
                    reply.result.clone(),
                ) {
                    return Task::none();
                }
                self.profile.is_saving_profile = false;
                self.refresh_apply_transaction();
                if let Ok(CommandOutput::ProfileDocumentSaved(saved)) = reply.result {
                    if let Some((_, document)) = &mut self.editor.document_latest {
                        *document = saved.document;
                    }
                    self.invalidate_rules_dns_views();
                    Task::done(Message::LoadProfileSnapshots)
                } else {
                    Task::none()
                }
            }
            _ => Task::none(),
        }
    }
}
