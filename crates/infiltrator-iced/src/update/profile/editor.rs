//! Profile YAML editor handlers: load profile content into the editor,
//! editor actions and saving back to disk.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use iced::Task;
use iced::widget::text_editor;
use infiltrator_contract::editor_viewport::EditorViewport;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_domain::apply::ApplyStrategy;

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
        crate::view::editor_viewport::window_lines_for_window_height(self.shell.viewport.height_px)
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
        match infiltrator_domain::config::preflight_yaml_syntax(&text) {
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
        let caret_column = infiltrator_contract::yaml_snippets::column_of_byte_offset(
            line_text,
            widget_cursor.position.column,
        );
        match infiltrator_application::profile_document_application::insert_snippet(
            &content,
            snippet_id,
            caret_line + 1,
            caret_column,
        ) {
            Ok(insertion) => {
                let inserted_line = insertion
                    .content
                    .split('\n')
                    .nth(insertion.cursor_line - 1)
                    .unwrap_or_default();
                let cursor = text_editor::Cursor {
                    position: text_editor::Position {
                        line: insertion.cursor_line - 1,
                        column: infiltrator_contract::yaml_snippets::byte_offset_of_column(
                            inserted_line,
                            insertion.cursor_column,
                        ),
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
            Err(failure) => Task::done(Message::ShowToast(failure.message, ToastStatus::Error)),
        }
    }

    /// DUAL-09-11: mirror the host core's typed apply outcome into the editor
    /// banner. The record is the same fact the shared projection carries.
    fn refresh_apply_transaction(&mut self) {
        self.editor.apply_transaction =
            infiltrator_contract::apply_transaction::last_apply_transaction();
    }

    pub(super) fn update_editor(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EditProfileAs(path, pane) => {
                self.editor.editor_pane = pane;
                self.update_editor(Message::EditProfile(path))
            }
            Message::EditProfile(path) => {
                let p = path.clone();
                Task::perform(
                    async move {
                        let content = tokio::fs::read_to_string(&p)
                            .await
                            .map_err(|e| InfiltratorError::Io(e.to_string()))?;
                        Ok((p, content))
                    },
                    Message::ProfileContentLoaded,
                )
            }
            Message::ProfileContentLoaded(result) => match result {
                Ok((path, content)) => {
                    self.editor.editor_path = Some(path);
                    self.editor.editor_content = text_editor::Content::with_text(&content);
                    self.reset_document_viewport(EditorPane::Profile);
                    let mut tasks = vec![
                        Task::done(Message::Navigate(crate::types::app::Route::Editor)),
                        Task::done(Message::LoadProfileSnapshots),
                    ];
                    // Preselected panes load their overlay document lazily
                    // now that editor_path is known.
                    match self.editor.editor_pane {
                        crate::types::options::EditorPane::Mixin => {
                            tasks.push(self.ensure_mixin_loaded());
                        }
                        crate::types::options::EditorPane::Filter => {
                            tasks.push(self.ensure_filter_loaded());
                        }
                        crate::types::options::EditorPane::Profile
                        | crate::types::options::EditorPane::Script => {}
                    }
                    Task::batch(tasks)
                }
                Err(e) => {
                    self.set_error(&e);
                    Task::none()
                }
            },
            Message::EditorAction(action) => {
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
                self.editor.is_loading_snapshots = true;
                Task::perform(
                    async move {
                        crate::snapshot_application::application()
                            .await?
                            .history(
                                &profile,
                                infiltrator_contract::snapshot_history::SNAPSHOT_DEFAULT_KEEP,
                            )
                            .await
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
                self.editor.is_backing_up_snapshot = true;
                Task::perform(
                    async move {
                        crate::snapshot_application::application()
                            .await?
                            .create(&profile)
                            .await
                            .map(|_| ())
                            .map_err(|failure| InfiltratorError::Config(failure.message))
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
                self.editor.snapshot_prune_keep =
                    infiltrator_contract::snapshot_history::SnapshotHistorySnapshot::clamp_keep(
                        keep,
                    );
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
                self.editor.is_pruning_snapshots = true;
                Task::perform(
                    async move {
                        crate::snapshot_application::application()
                            .await?
                            .prune(
                                &profile,
                                keep,
                                infiltrator_contract::snapshot_history::SnapshotPruneSource::Manual,
                            )
                            .await
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
            Message::ArmRestoreProfileSnapshot(path) => {
                self.editor.pending_restore_snapshot = Some(path);
                Task::none()
            }
            Message::CancelRestoreProfileSnapshot => {
                self.editor.pending_restore_snapshot = None;
                Task::none()
            }
            Message::RestoreProfileSnapshot(path) => {
                // DUAL-09-09: the first click only arms; the restore executes
                // once the same snapshot has been confirmed.
                if self.editor.pending_restore_snapshot.as_deref() != Some(path.as_path()) {
                    self.editor.pending_restore_snapshot = Some(path);
                    return Task::none();
                }
                self.editor.pending_restore_snapshot = None;
                if self.editor.is_restoring_snapshot {
                    return Task::none();
                }
                let Some(editor_path) = self.editor.editor_path.clone() else {
                    return Task::none();
                };
                let Some(profile) = editor_path
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .map(str::to_string)
                else {
                    return Task::none();
                };
                let runtime = self.runtime.runtime.clone();
                self.editor.is_restoring_snapshot = true;
                Task::perform(
                    async move {
                        crate::snapshot_application::application()
                            .await?
                            .restore(runtime, &profile, &path)
                            .await
                            .map_err(|failure| InfiltratorError::Config(failure.message))
                    },
                    Message::ProfileSnapshotRestored,
                )
            }
            Message::ProfileSnapshotRestored(result) => {
                self.editor.is_restoring_snapshot = false;
                match result {
                    Ok(()) => {
                        if let Some(runtime) = self.runtime.runtime.clone() {
                            self.sync_runtime_slot(Some(runtime));
                        }
                        let reload_path = self.editor.editor_path.clone();
                        let mut tasks = vec![Task::done(Message::LoadProfileSnapshots)];
                        if let Some(path) = reload_path {
                            tasks.push(Task::done(Message::EditProfile(path)));
                        }
                        tasks.push(Task::done(Message::ShowToast(
                            "Profile snapshot restored".to_string(),
                            ToastStatus::Success,
                        )));
                        Task::batch(tasks)
                    }
                    Err(error) => {
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::SaveProfile => {
                if self.profile.is_saving_profile {
                    return Task::none();
                }
                let content = self.editor.editor_content.text();
                if let Err(diag) = infiltrator_domain::config::preflight_yaml_syntax(&content) {
                    self.editor.syntax_error = Some(diag.message.clone());
                    self.editor.syntax_error_line = Some(diag.line);
                    return Task::done(Message::ShowToast(
                        format!("YAML Syntax Error (line {}): {}", diag.line, diag.message),
                        ToastStatus::Error,
                    ));
                }
                if let Some(path) = self.editor.editor_path.clone() {
                    self.profile.is_saving_profile = true;
                    let runtime = self.runtime.runtime.clone();
                    // DUAL-09-12: the editor passes its explicit unlock; the
                    // application re-checks the stored subscription metadata.
                    let allow_protected = self.editor.profile_protection_override;
                    Task::perform(
                        async move {
                            let profile_name = path
                                .file_stem()
                                .and_then(|name| name.to_str())
                                .ok_or_else(|| {
                                    InfiltratorError::Config(
                                        "无法从配置路径确定配置名称".to_string(),
                                    )
                                })?
                                .to_string();
                            crate::update::core::profile_apply::save_edited_profile_content(
                                runtime,
                                profile_name,
                                content,
                                ApplyStrategy::PreferReload,
                                allow_protected,
                            )
                            .await
                        },
                        Message::ProfileSaved,
                    )
                } else {
                    Task::none()
                }
            }
            Message::ProfileSaved(result) => match result {
                Ok(_) => {
                    self.profile.is_saving_profile = false;
                    self.refresh_apply_transaction();
                    if let Some(runtime) = self.runtime.runtime.clone() {
                        self.sync_runtime_slot(Some(runtime));
                    }
                    self.invalidate_rules_dns_views();
                    Task::batch(vec![
                        Task::done(Message::LoadProfileSnapshots),
                        Task::done(Message::ShowToast(
                            "Profile saved".to_string(),
                            ToastStatus::Success,
                        )),
                    ])
                }
                Err(e) => {
                    self.profile.is_saving_profile = false;
                    self.refresh_apply_transaction();
                    self.set_error(&e);
                    Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                }
            },
            _ => Task::none(),
        }
    }
}
