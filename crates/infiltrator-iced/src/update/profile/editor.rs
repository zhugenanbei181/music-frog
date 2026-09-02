//! Profile YAML editor handlers: load profile content into the editor,
//! editor actions and saving back to disk.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use iced::widget::text_editor;
use infiltrator_core::apply::ApplyStrategy;
use infiltrator_core::error::InfiltratorError;

impl AppState {
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
                        crate::types::options::EditorPane::Profile => {}
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
                match infiltrator_core::config::preflight_yaml_syntax(&text) {
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
                let Some(path) = self.editor.editor_path.clone() else {
                    self.editor.profile_snapshots.clear();
                    return Task::none();
                };
                let Some(profile) = path.file_stem().and_then(|name| name.to_str()) else {
                    self.editor.profile_snapshots.clear();
                    return Task::none();
                };
                let profile = profile.to_string();
                self.editor.is_loading_snapshots = true;
                Task::perform(
                    async move {
                        let configs_dir = crate::configs_dir::configs_dir().await?;
                        infiltrator_core::history::list_snapshots(&configs_dir, &profile)
                            .await
                            .map_err(|error| InfiltratorError::Config(error.to_string()))
                    },
                    Message::ProfileSnapshotsLoaded,
                )
            }
            Message::ProfileSnapshotsLoaded(result) => {
                self.editor.is_loading_snapshots = false;
                match result {
                    Ok(snapshots) => self.editor.profile_snapshots = snapshots,
                    Err(error) => self.set_error(&error),
                }
                Task::none()
            }
            Message::RestoreProfileSnapshot(path) => {
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
                        let home = mihomo_platform::paths::get_home_dir()
                            .map_err(InfiltratorError::from)?;
                        let snapshot_root = tokio::fs::canonicalize(home.join("configs/snapshots"))
                            .await
                            .map_err(InfiltratorError::from)?;
                        let snapshot_path = tokio::fs::canonicalize(&path)
                            .await
                            .map_err(InfiltratorError::from)?;
                        if !snapshot_path.starts_with(&snapshot_root) {
                            return Err(InfiltratorError::Config(
                                "拒绝从配置快照目录之外恢复文件".to_string(),
                            ));
                        }
                        let content = infiltrator_core::history::read_snapshot(&snapshot_path)
                            .await
                            .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                        crate::update::core::profile_apply::save_profile_content(
                            runtime,
                            profile,
                            content,
                            ApplyStrategy::PreferReload,
                        )
                        .await
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
                if let Err(diag) = infiltrator_core::config::preflight_yaml_syntax(&content) {
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
                            crate::update::core::profile_apply::save_profile_content(
                                runtime,
                                profile_name,
                                content,
                                ApplyStrategy::PreferReload,
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
                    self.set_error(&e);
                    Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                }
            },
            _ => Task::none(),
        }
    }
}
