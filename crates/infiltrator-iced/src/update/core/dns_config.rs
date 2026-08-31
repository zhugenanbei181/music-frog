//! DNS and Fake-IP advanced configuration: the form drafts and their JSON
//! editor twins, the quick-edit DNS/fallback server lists, persistence and
//! the Fake-IP cache flush.

use super::profile_apply::save_task;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::dns::{AdvancedEditMode, DnsFormDraft, FakeIpFormDraft};
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::runtime::RebuildFlowState;
use iced::Task;
use infiltrator_core::error::InfiltratorError;

impl AppState {
    pub(super) fn ensure_dns_editor_loaded(&mut self) {
        if self.editor.dns_editor_state == EditorLazyState::Loaded
            && self.editor.dns_json_content.text() == self.editor.dns_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.editor.dns_json_content =
            iced::widget::text_editor::Content::with_text(&self.editor.dns_json_cache);
        self.editor.dns_editor_state = EditorLazyState::Loaded;
        self.diag.perf_snapshot.dns_with_text_apply_ms = start.elapsed().as_millis();
    }

    pub(super) fn ensure_fake_ip_editor_loaded(&mut self) {
        if self.editor.fake_ip_editor_state == EditorLazyState::Loaded
            && self.editor.fake_ip_json_content.text() == self.editor.fake_ip_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.editor.fake_ip_json_content =
            iced::widget::text_editor::Content::with_text(&self.editor.fake_ip_json_cache);
        self.editor.fake_ip_editor_state = EditorLazyState::Loaded;
        self.diag.perf_snapshot.dns_with_text_apply_ms = start.elapsed().as_millis();
    }

    pub(super) fn apply_dns_form_from_config(&mut self, config: &infiltrator_core::dns::DnsConfig) {
        self.editor.dns_form = DnsFormDraft {
            enable: config.enable.unwrap_or(false),
            nameserver: Self::join_list_field(&config.nameserver),
            fallback: Self::join_list_field(&config.fallback),
            enhanced_mode: config
                .enhanced_mode
                .clone()
                .unwrap_or_else(|| "fake-ip".to_string()),
            fake_ip_range: config.fake_ip_range.clone().unwrap_or_default(),
            fake_ip_filter: Self::join_list_field(&config.fake_ip_filter),
            ipv6: config.ipv6.unwrap_or(false),
            cache: config.cache.unwrap_or(false),
            use_hosts: config.use_hosts.unwrap_or(false),
            use_system_hosts: config.use_system_hosts.unwrap_or(false),
            respect_rules: config.respect_rules.unwrap_or(false),
            proxy_server_nameserver: Self::join_list_field(&config.proxy_server_nameserver),
            direct_nameserver: Self::join_list_field(&config.direct_nameserver),
        };
    }

    pub(super) fn apply_fake_ip_form_from_config(
        &mut self,
        config: &infiltrator_core::fake_ip::FakeIpConfig,
    ) {
        self.editor.fake_ip_form = FakeIpFormDraft {
            fake_ip_range: config.fake_ip_range.clone().unwrap_or_default(),
            fake_ip_filter: Self::join_list_field(&config.fake_ip_filter),
            store_fake_ip: config.store_fake_ip.unwrap_or(false),
        };
    }

    fn dns_patch_from_form(
        &self,
    ) -> Result<infiltrator_core::dns::DnsConfigPatch, InfiltratorError> {
        let enhanced_mode = self
            .editor
            .dns_form
            .enhanced_mode
            .trim()
            .to_ascii_lowercase();
        if !enhanced_mode.is_empty() && enhanced_mode != "fake-ip" && enhanced_mode != "redir-host"
        {
            return Err(InfiltratorError::Config(
                "enhanced_mode must be fake-ip or redir-host".to_string(),
            ));
        }

        let fake_ip_range = self.editor.dns_form.fake_ip_range.trim();
        Ok(infiltrator_core::dns::DnsConfigPatch {
            enable: Some(self.editor.dns_form.enable),
            nameserver: Some(Self::split_list_field(&self.editor.dns_form.nameserver)),
            fallback: Some(Self::split_list_field(&self.editor.dns_form.fallback)),
            enhanced_mode: if enhanced_mode.is_empty() {
                None
            } else {
                Some(enhanced_mode)
            },
            fake_ip_range: if fake_ip_range.is_empty() {
                None
            } else {
                Some(fake_ip_range.to_string())
            },
            fake_ip_filter: Some(Self::split_list_field(&self.editor.dns_form.fake_ip_filter)),
            ipv6: Some(self.editor.dns_form.ipv6),
            cache: Some(self.editor.dns_form.cache),
            use_hosts: Some(self.editor.dns_form.use_hosts),
            use_system_hosts: Some(self.editor.dns_form.use_system_hosts),
            respect_rules: Some(self.editor.dns_form.respect_rules),
            proxy_server_nameserver: Some(Self::split_list_field(
                &self.editor.dns_form.proxy_server_nameserver,
            )),
            direct_nameserver: Some(Self::split_list_field(
                &self.editor.dns_form.direct_nameserver,
            )),
            ..infiltrator_core::dns::DnsConfigPatch::default()
        })
    }

    fn fake_ip_patch_from_form(
        &self,
    ) -> Result<infiltrator_core::fake_ip::FakeIpConfigPatch, InfiltratorError> {
        let fake_ip_range = self.editor.fake_ip_form.fake_ip_range.trim();
        Ok(infiltrator_core::fake_ip::FakeIpConfigPatch {
            fake_ip_range: if fake_ip_range.is_empty() {
                None
            } else {
                Some(fake_ip_range.to_string())
            },
            fake_ip_filter: Some(Self::split_list_field(
                &self.editor.fake_ip_form.fake_ip_filter,
            )),
            store_fake_ip: Some(self.editor.fake_ip_form.store_fake_ip),
        })
    }

    fn sync_dns_json_from_form(&mut self) -> Result<(), InfiltratorError> {
        let patch = self.dns_patch_from_form()?;
        self.editor.dns_json_cache = serde_json::to_string_pretty(&patch)
            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
        if self.editor.dns_editor_state == EditorLazyState::Loaded && !self.editor.dns_json_dirty {
            self.ensure_dns_editor_loaded();
        }
        Ok(())
    }

    fn sync_fake_ip_json_from_form(&mut self) -> Result<(), InfiltratorError> {
        let patch = self.fake_ip_patch_from_form()?;
        self.editor.fake_ip_json_cache = serde_json::to_string_pretty(&patch)
            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
        if self.editor.fake_ip_editor_state == EditorLazyState::Loaded
            && !self.editor.fake_ip_json_dirty
        {
            self.ensure_fake_ip_editor_loaded();
        }
        Ok(())
    }

    fn mark_dns_form_dirty_and_sync(&mut self) {
        self.editor.dns_form_dirty = true;
        match self.sync_dns_json_from_form() {
            Ok(_) => self.editor.advanced_validation.dns = None,
            Err(e) => {
                self.editor.advanced_validation.dns = Some(Self::map_advanced_error_message(&e))
            }
        }
    }

    fn mark_fake_ip_form_dirty_and_sync(&mut self) {
        self.editor.fake_ip_form_dirty = true;
        match self.sync_fake_ip_json_from_form() {
            Ok(_) => self.editor.advanced_validation.fake_ip = None,
            Err(e) => {
                self.editor.advanced_validation.fake_ip = Some(Self::map_advanced_error_message(&e))
            }
        }
    }

    /// DNS and Fake-IP advanced config editing and persistence. Unmatched
    /// messages fall through to the next domain in the `update_core` chain.
    pub(super) fn update_core_dns_config(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RefreshDnsOnly => Task::perform(
                async {
                    let config = infiltrator_core::dns::load_dns_config()
                        .await
                        .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                    serde_json::to_string_pretty(&config)
                        .map_err(|e| InfiltratorError::Config(e.to_string()))
                },
                Message::DnsConfigJsonLoaded,
            ),
            Message::RefreshFakeIpOnly => Task::perform(
                async {
                    let config = infiltrator_core::fake_ip::load_fake_ip_config()
                        .await
                        .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                    serde_json::to_string_pretty(&config)
                        .map_err(|e| InfiltratorError::Config(e.to_string()))
                },
                Message::FakeIpConfigJsonLoaded,
            ),
            Message::EnsureDnsEditorLoaded => {
                self.ensure_dns_editor_loaded();
                Task::none()
            }
            Message::EnsureFakeIpEditorLoaded => {
                self.ensure_fake_ip_editor_loaded();
                Task::none()
            }
            Message::DnsConfigJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        match serde_json::from_str::<infiltrator_core::dns::DnsConfig>(&json) {
                            Ok(config) => {
                                self.editor.advanced_configs_loaded_once = true;
                                self.editor.dns_json_cache = json;
                                self.apply_dns_form_from_config(&config);
                                if self.editor.dns_editor_state == EditorLazyState::Loaded {
                                    self.ensure_dns_editor_loaded();
                                }
                                self.editor.dns_json_dirty = false;
                                self.editor.dns_form_dirty = false;
                                self.editor.advanced_validation.dns = None;
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
            Message::FakeIpConfigJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        match serde_json::from_str::<infiltrator_core::fake_ip::FakeIpConfig>(&json)
                        {
                            Ok(config) => {
                                self.editor.advanced_configs_loaded_once = true;
                                self.editor.fake_ip_json_cache = json;
                                self.apply_fake_ip_form_from_config(&config);
                                if self.editor.fake_ip_editor_state == EditorLazyState::Loaded {
                                    self.ensure_fake_ip_editor_loaded();
                                }
                                self.editor.fake_ip_json_dirty = false;
                                self.editor.fake_ip_form_dirty = false;
                                self.editor.advanced_validation.fake_ip = None;
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
            Message::UpdateDnsFormEnable(value) => {
                self.editor.dns_form.enable = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormNameserver(value) => {
                self.editor.dns_form.nameserver = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormFallback(value) => {
                self.editor.dns_form.fallback = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormEnhancedMode(value) => {
                self.editor.dns_form.enhanced_mode = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormFakeIpRange(value) => {
                self.editor.dns_form.fake_ip_range = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormFakeIpFilter(value) => {
                self.editor.dns_form.fake_ip_filter = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormIpv6(value) => {
                self.editor.dns_form.ipv6 = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormCache(value) => {
                self.editor.dns_form.cache = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormUseHosts(value) => {
                self.editor.dns_form.use_hosts = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormUseSystemHosts(value) => {
                self.editor.dns_form.use_system_hosts = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormRespectRules(value) => {
                self.editor.dns_form.respect_rules = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormProxyServerNameserver(value) => {
                self.editor.dns_form.proxy_server_nameserver = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormDirectNameserver(value) => {
                self.editor.dns_form.direct_nameserver = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateFakeIpFormRange(value) => {
                self.editor.fake_ip_form.fake_ip_range = value;
                self.mark_fake_ip_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateFakeIpFormFilter(value) => {
                self.editor.fake_ip_form.fake_ip_filter = value;
                self.mark_fake_ip_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateFakeIpFormStore(value) => {
                self.editor.fake_ip_form.store_fake_ip = value;
                self.mark_fake_ip_form_dirty_and_sync();
                Task::none()
            }
            Message::DnsConfigEditorAction(action) => {
                self.ensure_dns_editor_loaded();
                self.editor.dns_json_content.perform(action);
                self.editor.dns_json_cache = self.editor.dns_json_content.text();
                self.editor.dns_json_dirty = true;
                self.editor.advanced_validation.dns = None;
                Task::none()
            }
            Message::FakeIpConfigEditorAction(action) => {
                self.ensure_fake_ip_editor_loaded();
                self.editor.fake_ip_json_content.perform(action);
                self.editor.fake_ip_json_cache = self.editor.fake_ip_json_content.text();
                self.editor.fake_ip_json_dirty = true;
                self.editor.advanced_validation.fake_ip = None;
                Task::none()
            }
            Message::UpdateDnsServer(index, server) => {
                if let Some(target) = self.editor.dns_nameservers.get_mut(index) {
                    *target = server;
                }
                Task::none()
            }
            Message::UpdateDnsEnhancedMode(mode) => {
                self.editor.dns_enhanced_mode = mode;
                Task::none()
            }
            Message::AddDnsServer => {
                self.editor.dns_nameservers.push(String::new());
                Task::none()
            }
            Message::AddDnsServerTemplate(server) => {
                self.editor.dns_nameservers.push(server);
                Task::none()
            }
            Message::RemoveDnsServer(index) => {
                if self.editor.dns_nameservers.len() > index {
                    self.editor.dns_nameservers.remove(index);
                }
                Task::none()
            }
            Message::UpdateFallbackDnsServer(index, value) => {
                if let Some(server) = self.editor.dns_fallback_servers.get_mut(index) {
                    *server = value;
                }
                Task::none()
            }
            Message::AddFallbackDnsServer => {
                self.editor.dns_fallback_servers.push(String::new());
                Task::none()
            }
            Message::RemoveFallbackDnsServer(index) => {
                if self.editor.dns_fallback_servers.len() > index {
                    self.editor.dns_fallback_servers.remove(index);
                }
                Task::none()
            }
            Message::SaveDns => {
                self.editor.is_saving_dns = true;
                self.begin_save_phase("DNS");
                let patch = if self.editor.dns_mode == AdvancedEditMode::Form {
                    self.dns_patch_from_form()
                } else {
                    self.ensure_dns_editor_loaded();
                    let text = self.editor.dns_json_content.text();
                    self.editor.dns_json_cache = text.clone();
                    serde_json::from_str::<infiltrator_core::dns::DnsConfigPatch>(&text)
                        .map_err(|e| InfiltratorError::Config(format!("Invalid DNS JSON: {}", e)))
                };
                let patch = match patch {
                    Ok(value) => value,
                    Err(error) => {
                        self.editor.is_saving_dns = false;
                        let mapped = Self::map_advanced_error_message(&error);
                        self.editor.advanced_validation.dns = Some(mapped.clone());
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
                            label: "DNS".to_string(),
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
                save_task(
                    self.runtime.runtime.clone(),
                    move |content| infiltrator_core::dns::apply_dns_patch_to_yaml(content, patch),
                    Message::DnsSaved,
                )
            }
            Message::DnsSaved(result) => {
                self.editor.is_saving_dns = false;
                match result {
                    Ok(_) => {
                        self.editor.dns_form_dirty = false;
                        self.editor.dns_json_dirty = false;
                        self.editor.advanced_validation.dns = None;
                        Task::batch(vec![
                            Task::done(Message::RefreshDnsOnly),
                            self.finish_without_rebuild("DNS".to_string()),
                        ])
                    }
                    Err(e) => {
                        let mapped = Self::map_advanced_error_message(&e);
                        self.editor.advanced_validation.dns = Some(mapped.clone());
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
                            label: "DNS".to_string(),
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
            Message::SaveFakeIpConfig => {
                self.editor.is_saving_fake_ip = true;
                self.begin_save_phase("Fake-IP");
                let patch = if self.editor.fake_ip_mode == AdvancedEditMode::Form {
                    self.fake_ip_patch_from_form()
                } else {
                    self.ensure_fake_ip_editor_loaded();
                    let text = self.editor.fake_ip_json_content.text();
                    self.editor.fake_ip_json_cache = text.clone();
                    serde_json::from_str::<infiltrator_core::fake_ip::FakeIpConfigPatch>(&text)
                        .map_err(|e| {
                            InfiltratorError::Config(format!("Invalid Fake-IP JSON: {}", e))
                        })
                };
                let patch = match patch {
                    Ok(value) => value,
                    Err(error) => {
                        self.editor.is_saving_fake_ip = false;
                        let mapped = Self::map_advanced_error_message(&error);
                        self.editor.advanced_validation.fake_ip = Some(mapped.clone());
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
                            label: "Fake-IP".to_string(),
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
                save_task(
                    self.runtime.runtime.clone(),
                    move |content| {
                        infiltrator_core::fake_ip::apply_fake_ip_patch_to_yaml(content, patch)
                    },
                    Message::FakeIpConfigSaved,
                )
            }
            Message::FakeIpConfigSaved(result) => {
                self.editor.is_saving_fake_ip = false;
                match result {
                    Ok(_) => {
                        self.editor.fake_ip_form_dirty = false;
                        self.editor.fake_ip_json_dirty = false;
                        self.editor.advanced_validation.fake_ip = None;
                        Task::batch(vec![
                            Task::done(Message::RefreshFakeIpOnly),
                            self.finish_without_rebuild("Fake-IP".to_string()),
                        ])
                    }
                    Err(e) => {
                        let mapped = Self::map_advanced_error_message(&e);
                        self.editor.advanced_validation.fake_ip = Some(mapped.clone());
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
                            label: "Fake-IP".to_string(),
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
            Message::FlushFakeIpCache => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .flush_fakeip_cache()
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            other => self.update_core_tun_config(other),
        }
    }
}
