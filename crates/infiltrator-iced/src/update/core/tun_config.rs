//! TUN advanced configuration: the form draft and its JSON editor twin,
//! validation (stack/MTU) and persistence.

use super::profile_apply::save_task_with_strategy;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::dns::{AdvancedEditMode, TunFormDraft};
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::runtime::RebuildFlowState;
use iced::Task;
use infiltrator_core::apply::ApplyStrategy;
use infiltrator_core::error::InfiltratorError;

impl AppState {
    pub(super) fn ensure_tun_editor_loaded(&mut self) {
        if self.editor.tun_editor_state == EditorLazyState::Loaded
            && self.editor.tun_json_content.text() == self.editor.tun_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.editor.tun_json_content =
            iced::widget::text_editor::Content::with_text(&self.editor.tun_json_cache);
        self.editor.tun_editor_state = EditorLazyState::Loaded;
        self.diag.perf_snapshot.dns_with_text_apply_ms = start.elapsed().as_millis();
    }

    pub(super) fn apply_tun_form_from_config(&mut self, config: &infiltrator_core::tun::TunConfig) {
        self.editor.tun_form = TunFormDraft {
            enable: config.enable.unwrap_or(false),
            stack: config.stack.clone().unwrap_or_else(|| "gvisor".to_string()),
            mtu: config
                .mtu
                .map(|value| value.to_string())
                .unwrap_or_default(),
            dns_hijack: Self::join_list_field(&config.dns_hijack),
            auto_route: config.auto_route.unwrap_or(false),
            auto_detect_interface: config.auto_detect_interface.unwrap_or(false),
            strict_route: config.strict_route.unwrap_or(false),
        };
    }

    fn tun_patch_from_form(
        &self,
    ) -> Result<infiltrator_core::tun::TunConfigPatch, InfiltratorError> {
        let stack = self.editor.tun_form.stack.trim().to_ascii_lowercase();
        if !stack.is_empty() && stack != "system" && stack != "gvisor" {
            return Err(InfiltratorError::Config(
                "stack must be system or gvisor".to_string(),
            ));
        }

        let mtu_text = self.editor.tun_form.mtu.trim();
        let mtu = if mtu_text.is_empty() {
            None
        } else {
            Some(mtu_text.parse::<u32>().map_err(|_| {
                InfiltratorError::Config("mtu must be a positive integer".to_string())
            })?)
        };
        if matches!(mtu, Some(0)) {
            return Err(InfiltratorError::Config(
                "mtu must be greater than 0".to_string(),
            ));
        }

        Ok(infiltrator_core::tun::TunConfigPatch {
            enable: Some(self.editor.tun_form.enable),
            stack: if stack.is_empty() { None } else { Some(stack) },
            mtu,
            dns_hijack: Some(Self::split_list_field(&self.editor.tun_form.dns_hijack)),
            auto_route: Some(self.editor.tun_form.auto_route),
            auto_detect_interface: Some(self.editor.tun_form.auto_detect_interface),
            strict_route: Some(self.editor.tun_form.strict_route),
        })
    }

    fn sync_tun_json_from_form(&mut self) -> Result<(), InfiltratorError> {
        let patch = self.tun_patch_from_form()?;
        self.editor.tun_json_cache = serde_json::to_string_pretty(&patch)
            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
        if self.editor.tun_editor_state == EditorLazyState::Loaded && !self.editor.tun_json_dirty {
            self.ensure_tun_editor_loaded();
        }
        Ok(())
    }

    fn mark_tun_form_dirty_and_sync(&mut self) {
        self.editor.tun_form_dirty = true;
        match self.sync_tun_json_from_form() {
            Ok(_) => self.editor.advanced_validation.tun = None,
            Err(e) => {
                self.editor.advanced_validation.tun = Some(Self::map_advanced_error_message(&e))
            }
        }
    }

    /// TUN advanced config editing and persistence. Unmatched messages fall
    /// through to the next domain in the `update_core` chain.
    pub(super) fn update_core_tun_config(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RefreshTunOnly => Task::perform(
                async {
                    let config = infiltrator_core::tun::load_tun_config()
                        .await
                        .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                    serde_json::to_string_pretty(&config)
                        .map_err(|e| InfiltratorError::Config(e.to_string()))
                },
                Message::TunConfigJsonLoaded,
            ),
            Message::EnsureTunEditorLoaded => {
                self.ensure_tun_editor_loaded();
                Task::none()
            }
            Message::TunConfigJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        match serde_json::from_str::<infiltrator_core::tun::TunConfig>(&json) {
                            Ok(config) => {
                                self.editor.advanced_configs_loaded_once = true;
                                self.editor.tun_json_cache = json;
                                self.apply_tun_form_from_config(&config);
                                if self.editor.tun_editor_state == EditorLazyState::Loaded {
                                    self.ensure_tun_editor_loaded();
                                }
                                self.editor.tun_json_dirty = false;
                                self.editor.tun_form_dirty = false;
                                self.editor.advanced_validation.tun = None;
                            }
                            Err(e) => {
                                self.set_error(&e);
                            }
                        }
                    }
                    Err(e) => self.set_error(&e),
                }
                Task::none()
            }
            Message::UpdateTunFormEnable(value) => {
                self.editor.tun_form.enable = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormStack(value) => {
                self.editor.tun_form.stack = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormMtu(value) => {
                self.editor.tun_form.mtu = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormDnsHijack(value) => {
                self.editor.tun_form.dns_hijack = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormAutoRoute(value) => {
                self.editor.tun_form.auto_route = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormAutoDetectInterface(value) => {
                self.editor.tun_form.auto_detect_interface = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateTunFormStrictRoute(value) => {
                self.editor.tun_form.strict_route = value;
                self.mark_tun_form_dirty_and_sync();
                Task::none()
            }
            Message::TunConfigEditorAction(action) => {
                self.ensure_tun_editor_loaded();
                self.editor.tun_json_content.perform(action);
                self.editor.tun_json_cache = self.editor.tun_json_content.text();
                self.editor.tun_json_dirty = true;
                self.editor.advanced_validation.tun = None;
                Task::none()
            }
            Message::SaveTunConfig => {
                self.editor.is_saving_tun = true;
                self.begin_save_phase("TUN");
                let patch = if self.editor.tun_mode == AdvancedEditMode::Form {
                    self.tun_patch_from_form()
                } else {
                    self.ensure_tun_editor_loaded();
                    let text = self.editor.tun_json_content.text();
                    self.editor.tun_json_cache = text.clone();
                    serde_json::from_str::<infiltrator_core::tun::TunConfigPatch>(&text)
                        .map_err(|e| InfiltratorError::Config(format!("Invalid TUN JSON: {}", e)))
                };
                let patch = match patch {
                    Ok(value) => value,
                    Err(error) => {
                        self.editor.is_saving_tun = false;
                        let mapped = Self::map_advanced_error_message(&error);
                        self.editor.advanced_validation.tun = Some(mapped.clone());
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
                            label: "TUN".to_string(),
                            error: mapped.clone(),
                        };
                        self.set_error(&mapped);
                        return Task::batch(vec![
                            Task::done(Message::ShowToast(mapped, ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ]);
                    }
                };
                save_task_with_strategy(
                    self.runtime.runtime.clone(),
                    ApplyStrategy::AlwaysRestart,
                    move |content| infiltrator_core::tun::apply_tun_patch_to_yaml(content, patch),
                    Message::TunConfigSaved,
                )
            }
            Message::TunConfigSaved(result) => {
                self.editor.is_saving_tun = false;
                match result {
                    Ok(_) => {
                        self.editor.tun_form_dirty = false;
                        self.editor.tun_json_dirty = false;
                        self.editor.advanced_validation.tun = None;
                        Task::batch(vec![
                            Task::done(Message::RefreshTunOnly),
                            self.finish_without_rebuild("TUN".to_string()),
                        ])
                    }
                    Err(e) => {
                        let mapped = Self::map_advanced_error_message(&e);
                        self.editor.advanced_validation.tun = Some(mapped.clone());
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
                            label: "TUN".to_string(),
                            error: mapped.clone(),
                        };
                        self.set_error(&mapped);
                        Task::batch(vec![
                            Task::done(Message::ShowToast(mapped, ToastStatus::Error)),
                            Task::perform(
                                async {
                                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearRebuildFlow,
                            ),
                        ])
                    }
                }
            }
            other => self.update_core_rebuild(other),
        }
    }
}
