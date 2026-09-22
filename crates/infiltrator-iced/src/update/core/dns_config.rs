//! DNS and Fake-IP advanced configuration: the form drafts and their JSON
//! editor twins, the quick-edit DNS/fallback server lists, persistence and
//! the Fake-IP cache flush.

use super::profile_apply::save_task;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::dns::{AdvancedEditMode, FakeIpFormDraft};
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::runtime::RebuildFlowState;
use iced::Task;
use infiltrator_contract::error::InfiltratorError;

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

    pub(super) fn apply_dns_form_from_config(
        &mut self,
        config: &infiltrator_domain::dns::DnsConfig,
    ) {
        self.editor.dns_form =
            infiltrator_application::dns_workbench_application::form_from_config(config);
    }

    pub(super) fn apply_fake_ip_form_from_config(
        &mut self,
        config: &infiltrator_domain::fake_ip::FakeIpConfig,
    ) {
        self.editor.fake_ip_form = FakeIpFormDraft {
            fake_ip_range: config.fake_ip_range.clone().unwrap_or_default(),
            fake_ip_filter: Self::join_list_field(&config.fake_ip_filter),
            store_fake_ip: config.store_fake_ip.unwrap_or(false),
        };
    }

    pub(crate) fn dns_patch_from_form(
        &self,
    ) -> Result<infiltrator_domain::dns::DnsConfigPatch, InfiltratorError> {
        Ok(
            infiltrator_application::configuration_application::dns_patch_from_settings(
                self.editor.dns_form.patch(),
            ),
        )
    }

    pub(crate) fn dns_form_issue(&self) -> Option<infiltrator_contract::dns_form::DnsFormIssue> {
        self.editor.dns_form.validate().into_iter().next()
    }

    fn dns_issue_message(issue: &infiltrator_contract::dns_form::DnsFormIssue) -> String {
        use infiltrator_contract::dns_form::DnsFormIssue;
        match issue {
            DnsFormIssue::UnsupportedScheme { field, entry } => {
                format!(
                    "{}: unsupported upstream scheme in '{}'",
                    field.key(),
                    entry
                )
            }
            DnsFormIssue::BootstrapNotIp { entry } => {
                format!("default-nameserver must be a pure IP: '{}'", entry)
            }
            DnsFormIssue::InvalidTriggerCidr { entry } => {
                format!("fallback-filter ipcidr is not a CIDR network: '{}'", entry)
            }
            DnsFormIssue::InvalidGeoipCode { value } => {
                format!(
                    "fallback-filter geoip-code must be a 2-letter code: '{}'",
                    value
                )
            }
        }
    }

    fn fake_ip_patch_from_form(
        &self,
    ) -> Result<infiltrator_domain::fake_ip::FakeIpConfigPatch, InfiltratorError> {
        let fake_ip_range = self.editor.fake_ip_form.fake_ip_range.trim();
        Ok(infiltrator_domain::fake_ip::FakeIpConfigPatch {
            fake_ip_range: if fake_ip_range.is_empty() {
                None
            } else {
                Some(fake_ip_range.to_string())
            },
            fake_ip_filter: Some(Self::split_list_field(
                &self.editor.fake_ip_form.fake_ip_filter,
            )),
            store_fake_ip: Some(self.editor.fake_ip_form.store_fake_ip),
            ..Default::default()
        })
    }

    /// Honest editor message for an invalid shared hosts row.
    pub(crate) fn hosts_issue_message(issue: &infiltrator_contract::dns::DnsHostsIssue) -> String {
        use infiltrator_contract::dns::DnsHostsIssue;
        match issue {
            DnsHostsIssue::InvalidAddress { address } => {
                format!("dns.hosts value must be an IP, 'lan' or an alias domain: '{address}'")
            }
            DnsHostsIssue::InvalidDomain { domain } => {
                format!("dns.hosts key is not a valid domain: '{domain}'")
            }
        }
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
                    let config = crate::configuration::application()
                        .await?
                        .load_dns_config()
                        .await
                        .map_err(|failure| InfiltratorError::Config(failure.message))?;
                    serde_json::to_string_pretty(&config)
                        .map_err(|e| InfiltratorError::Config(e.to_string()))
                },
                Message::DnsConfigJsonLoaded,
            ),
            Message::RefreshFakeIpOnly => Task::perform(
                async {
                    let config = crate::configuration::application()
                        .await?
                        .load_fake_ip_config()
                        .await
                        .map_err(|failure| InfiltratorError::Config(failure.message))?;
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
                        match serde_json::from_str::<infiltrator_domain::dns::DnsConfig>(&json) {
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
                        match serde_json::from_str::<infiltrator_domain::fake_ip::FakeIpConfig>(
                            &json,
                        ) {
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
                self.editor.dns_form.switches.enable = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormBootstrapNameserver(value) => {
                self.editor.dns_form.bootstrap_nameserver = value;
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
            Message::UpdateDnsFormFallbackGeoip(value) => {
                self.editor.dns_form.fallback_policy.geoip = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormFallbackGeoipCode(value) => {
                self.editor.dns_form.fallback_policy.geoip_code = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormFallbackTrigger(value) => {
                self.editor.dns_form.fallback_policy.trigger_ipcidr = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormEnhancedMode(value) => {
                self.editor.dns_form.enhanced_mode = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormFilterMode(value) => {
                self.editor.dns_form.filter_mode = value;
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
                self.editor.dns_form.switches.ipv6 = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormCache(value) => {
                self.editor.dns_form.switches.cache = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormUseHosts(value) => {
                self.editor.dns_form.switches.use_hosts = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormUseSystemHosts(value) => {
                self.editor.dns_form.switches.use_system_hosts = value;
                self.mark_dns_form_dirty_and_sync();
                Task::none()
            }
            Message::UpdateDnsFormRespectRules(value) => {
                self.editor.dns_form.switches.respect_rules = value;
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
            Message::UpdateDnsFakeIpQuery(query) => {
                self.editor.dns_fake_ip_query = query;
                Task::none()
            }
            Message::UpdateDnsHostsAddress(value) => {
                self.editor.dns_hosts_address = value;
                Task::none()
            }
            Message::UpdateDnsHostsDomain(value) => {
                self.editor.dns_hosts_domain = value;
                Task::none()
            }
            Message::AddDnsHostRow => {
                let address = self.editor.dns_hosts_address.trim().to_owned();
                let domain = self.editor.dns_hosts_domain.trim().to_owned();
                if !address.is_empty() && !domain.is_empty() {
                    let row = infiltrator_contract::dns::DnsHostEntry { domain, address };
                    if !self.editor.dns_hosts.contains(&row) {
                        self.editor.dns_hosts.push(row);
                    }
                    self.editor.dns_hosts_address.clear();
                    self.editor.dns_hosts_domain.clear();
                    self.editor.dns_hosts_dirty = true;
                }
                Task::none()
            }
            Message::RemoveDnsHostRow(index) => {
                if index < self.editor.dns_hosts.len() {
                    self.editor.dns_hosts.remove(index);
                    self.editor.dns_hosts_dirty = true;
                }
                Task::none()
            }
            Message::SaveDnsHosts => {
                self.editor.is_saving_dns_hosts = true;
                self.begin_save_phase("DNS Hosts");
                let issues = infiltrator_contract::dns::validate_hosts(&self.editor.dns_hosts);
                if let Some(issue) = issues.first() {
                    self.editor.is_saving_dns_hosts = false;
                    let message = Self::hosts_issue_message(issue);
                    self.editor.advanced_validation.dns_hosts = Some(message.clone());
                    self.runtime.rebuild_flow = RebuildFlowState::Failed {
                        label: "DNS Hosts".to_string(),
                        error: message.clone(),
                    };
                    self.set_error(&message);
                    return Task::batch(vec![
                        Task::done(Message::ShowToast(message, ToastStatus::Error)),
                        Task::perform(
                            async {
                                tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                            },
                            |_| Message::ClearRebuildFlow,
                        ),
                    ]);
                }
                let entries = self.editor.dns_hosts.clone();
                // The shared mapping is the single write path: both surfaces
                // turn the same shared patch into the same domain patch.
                let patch =
                    infiltrator_application::configuration_application::dns_patch_from_settings(
                        infiltrator_contract::dns::DnsSettingsPatch {
                            hosts: Some(entries),
                            ..infiltrator_contract::dns::DnsSettingsPatch::default()
                        },
                    );
                save_task(
                    self.runtime.runtime.clone(),
                    move |content| infiltrator_domain::dns::apply_dns_patch_to_yaml(content, patch),
                    Message::DnsHostsSaved,
                )
            }
            Message::DnsHostsSaved(result) => {
                self.editor.is_saving_dns_hosts = false;
                match result {
                    Ok(()) => {
                        self.editor.dns_hosts_dirty = false;
                        self.editor.advanced_validation.dns_hosts = None;
                        Task::batch(vec![
                            Task::done(Message::RefreshDnsOnly),
                            self.finish_without_rebuild("DNS Hosts".to_string()),
                        ])
                    }
                    Err(error) => {
                        let mapped = Self::map_advanced_error_message(&error);
                        self.editor.advanced_validation.dns_hosts = Some(mapped.clone());
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
                            label: "DNS Hosts".to_string(),
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
                    match self.dns_form_issue() {
                        Some(issue) => {
                            Err(InfiltratorError::Config(Self::dns_issue_message(&issue)))
                        }
                        None => self.dns_patch_from_form(),
                    }
                } else {
                    self.ensure_dns_editor_loaded();
                    let text = self.editor.dns_json_content.text();
                    self.editor.dns_json_cache = text.clone();
                    serde_json::from_str::<infiltrator_domain::dns::DnsConfigPatch>(&text)
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
                    move |content| infiltrator_domain::dns::apply_dns_patch_to_yaml(content, patch),
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
                    serde_json::from_str::<infiltrator_domain::fake_ip::FakeIpConfigPatch>(&text)
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
                        infiltrator_domain::fake_ip::apply_fake_ip_patch_to_yaml(content, patch)
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
                            let gateway: std::sync::Arc<
                                dyn infiltrator_ports::runtime_gateway::RuntimeGateway,
                            > = rt.clone();
                            let application =
                                infiltrator_application::dns_cache_application::DnsCacheApplication::new(
                                    Some(gateway),
                                    rt.system_dns_cache_port(),
                                );
                            application
                                .flush_all()
                                .await
                                .map_err(|failure| failure.message)
                        },
                        Message::DnsCacheFlushed,
                    )
                } else {
                    Task::none()
                }
            }
            Message::DnsCacheFlushed(result) => match result {
                Ok(report) => {
                    self.diag.dns_cache_flush = report;
                    Task::none()
                }
                Err(message) => {
                    self.set_error(&message);
                    Task::done(Message::ShowToast(message, ToastStatus::Error))
                }
            },
            other => self.update_core_tun_config(other),
        }
    }
}
