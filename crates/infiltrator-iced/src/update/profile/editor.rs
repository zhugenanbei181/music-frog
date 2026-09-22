//! Profile YAML editor handlers: load profile content into the editor,
//! editor actions and saving back to disk.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use iced::widget::text_editor;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_domain::apply::ApplyStrategy;

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
                self.editor.editor_content.perform(action);
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
                Task::none()
            }
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
