//! Shared machinery for the advanced config editors (DNS / Fake-IP / TUN):
//! form/JSON mode switching, list-field parsing helpers, the shared
//! validation error mapper and the combined config bundle loader. The
//! per-config editing lives in [`super::dns_config`] and
//! [`super::tun_config`].

use crate::state::AppState;
use crate::types::dns::{AdvancedConfigsBundle, AdvancedEditMode, DnsTab};
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_core::error::InfiltratorError;

impl AppState {
    pub(super) fn reset_dns_lazy_state(&mut self) {
        self.editor.dns_editor_state = EditorLazyState::Unloaded;
        self.editor.fake_ip_editor_state = EditorLazyState::Unloaded;
        self.editor.tun_editor_state = EditorLazyState::Unloaded;
    }

    pub(super) fn split_list_field(raw: &str) -> Vec<String> {
        raw.split([',', '\n', '\r'])
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect()
    }

    pub(super) fn join_list_field(values: &Option<Vec<String>>) -> String {
        values
            .as_ref()
            .map(|items| items.join(", "))
            .unwrap_or_default()
    }

    pub(super) fn map_advanced_error_message(error: &InfiltratorError) -> String {
        let message = error.to_string();
        if message.contains("unsupported tun stack") {
            return "TUN stack must be 'system' or 'gvisor'".to_string();
        }
        if message.contains("unsupported enhanced-mode") {
            return "DNS enhanced mode must be 'fake-ip' or 'redir-host'".to_string();
        }
        if message.contains("mtu must be greater than 0") {
            return "TUN MTU must be greater than 0".to_string();
        }
        if message.contains("contains empty entry") {
            return "List fields cannot contain empty items".to_string();
        }
        message
    }

    fn active_advanced_mode(&self, tab: DnsTab) -> AdvancedEditMode {
        match tab {
            DnsTab::Dns => self.editor.dns_mode,
            DnsTab::FakeIp => self.editor.fake_ip_mode,
            DnsTab::Tun => self.editor.tun_mode,
        }
    }

    /// Advanced-config tab/mode switching and the combined DNS/Fake-IP/TUN
    /// bundle loader. Unmatched messages fall through to the next domain in
    /// the `update_core` chain.
    pub(super) fn update_core_advanced(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetDnsTab(tab) => {
                self.editor.dns_tab = tab;
                if self.active_advanced_mode(tab) == AdvancedEditMode::Json {
                    Task::done(match tab {
                        DnsTab::Dns => Message::EnsureDnsEditorLoaded,
                        DnsTab::FakeIp => Message::EnsureFakeIpEditorLoaded,
                        DnsTab::Tun => Message::EnsureTunEditorLoaded,
                    })
                } else {
                    Task::none()
                }
            }
            Message::SetAdvancedMode(tab, mode) => {
                match tab {
                    DnsTab::Dns => self.editor.dns_mode = mode,
                    DnsTab::FakeIp => self.editor.fake_ip_mode = mode,
                    DnsTab::Tun => self.editor.tun_mode = mode,
                }
                if mode == AdvancedEditMode::Json {
                    Task::done(match tab {
                        DnsTab::Dns => Message::EnsureDnsEditorLoaded,
                        DnsTab::FakeIp => Message::EnsureFakeIpEditorLoaded,
                        DnsTab::Tun => Message::EnsureTunEditorLoaded,
                    })
                } else {
                    Task::none()
                }
            }
            Message::ActivateDnsHeavyView => {
                self.editor.dns_heavy_ready = true;
                if self.active_advanced_mode(self.editor.dns_tab) == AdvancedEditMode::Json {
                    Task::done(match self.editor.dns_tab {
                        DnsTab::Dns => Message::EnsureDnsEditorLoaded,
                        DnsTab::FakeIp => Message::EnsureFakeIpEditorLoaded,
                        DnsTab::Tun => Message::EnsureTunEditorLoaded,
                    })
                } else {
                    Task::none()
                }
            }
            Message::LoadAdvancedConfigs => {
                if !self.editor.advanced_configs_loaded_once {
                    self.reset_dns_lazy_state();
                }
                Task::perform(
                    async {
                        let manager = crate::configs_dir::config_manager().await?;
                        let profile = manager.get_current().await.map_err(
                            |e: mihomo_api::error::MihomoError| InfiltratorError::from(e),
                        )?;
                        let content = manager.load(&profile).await.map_err(
                            |e: mihomo_api::error::MihomoError| InfiltratorError::from(e),
                        )?;
                        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        let dns = infiltrator_domain::dns::extract_dns_config_from_doc(&doc)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let fake_ip =
                            infiltrator_domain::fake_ip::extract_fake_ip_config_from_doc(&doc)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let tun = infiltrator_domain::tun::extract_tun_config_from_doc(&doc)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        Ok(Box::new(AdvancedConfigsBundle {
                            dns_json: serde_json::to_string_pretty(&dns)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?,
                            fake_ip_json: serde_json::to_string_pretty(&fake_ip)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?,
                            tun_json: serde_json::to_string_pretty(&tun)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?,
                            dns,
                            fake_ip,
                            tun,
                        }))
                    },
                    Message::AdvancedConfigsBundleLoaded,
                )
            }
            Message::AdvancedConfigsBundleLoaded(result) => {
                match result {
                    Ok(bundle) => {
                        self.editor.advanced_configs_loaded_once = true;
                        if !self.editor.dns_json_dirty && !self.editor.dns_form_dirty {
                            self.editor.dns_json_cache = bundle.dns_json;
                            self.apply_dns_form_from_config(&bundle.dns);
                            if self.editor.dns_editor_state == EditorLazyState::Loaded {
                                self.ensure_dns_editor_loaded();
                            }
                            self.editor.dns_json_dirty = false;
                            self.editor.dns_form_dirty = false;
                            self.editor.advanced_validation.dns = None;
                        }
                        if !self.editor.fake_ip_json_dirty && !self.editor.fake_ip_form_dirty {
                            self.editor.fake_ip_json_cache = bundle.fake_ip_json;
                            self.apply_fake_ip_form_from_config(&bundle.fake_ip);
                            if self.editor.fake_ip_editor_state == EditorLazyState::Loaded {
                                self.ensure_fake_ip_editor_loaded();
                            }
                            self.editor.fake_ip_json_dirty = false;
                            self.editor.fake_ip_form_dirty = false;
                            self.editor.advanced_validation.fake_ip = None;
                        }
                        if !self.editor.tun_json_dirty && !self.editor.tun_form_dirty {
                            self.editor.tun_json_cache = bundle.tun_json;
                            self.apply_tun_form_from_config(&bundle.tun);
                            if self.editor.tun_editor_state == EditorLazyState::Loaded {
                                self.ensure_tun_editor_loaded();
                            }
                            self.editor.tun_json_dirty = false;
                            self.editor.tun_form_dirty = false;
                            self.editor.advanced_validation.tun = None;
                        }
                    }
                    Err(e) => {
                        self.editor.advanced_configs_loaded_once = false;
                        self.set_error(&e);
                    }
                }
                Task::none()
            }
            other => self.update_core_dns_config(other),
        }
    }
}
