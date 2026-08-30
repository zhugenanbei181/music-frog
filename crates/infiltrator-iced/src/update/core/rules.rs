//! Rules domain: the custom rules list (filter, reorder, add, toggle),
//! rule/proxy provider refresh and the lazy JSON editors for rule
//! providers, proxy providers and the sniffer config.

use crate::state::AppState;
use crate::types::{
    EditorLazyState, InfiltratorError, Message, RebuildFlowState, RuleBadgeKind, RuleRenderItem,
    RulesJsonTab, RulesLoadBundle, RulesTab, ToastStatus,
};
use infiltrator_core::rules::RuleEntry;
use iced::Task;

impl AppState {
    fn split_rule_parts(rule: &str) -> (String, String, String) {
        let mut parts = rule.splitn(3, ',');
        let rule_type = parts.next().unwrap_or("").trim().to_string();
        let payload = parts.next().unwrap_or("").trim().to_string();
        let target = parts.next().unwrap_or("").trim().to_string();
        (rule_type, payload, target)
    }

    fn rule_badge_kind(rule_type: &str) -> RuleBadgeKind {
        match rule_type {
            "DOMAIN" | "DOMAIN-SUFFIX" | "DOMAIN-KEYWORD" => RuleBadgeKind::Domain,
            "IP-CIDR" | "IP-CIDR6" | "GEOIP" => RuleBadgeKind::Ip,
            _ => RuleBadgeKind::Other,
        }
    }

    /// Rebuild the rules render cache from `rules`. pub(crate) so the demo
    /// constructor can seed the cache for its fixture rules.
    pub(crate) fn rebuild_rules_render_cache(&mut self) {
        let start = std::time::Instant::now();
        self.rules_render_cache = self
            .rules
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let (rule_type, payload, target) = Self::split_rule_parts(&entry.rule);
                RuleRenderItem {
                    source_index: index,
                    search_key: entry.rule.to_lowercase(),
                    badge: Self::rule_badge_kind(&rule_type),
                    rule_type,
                    payload,
                    target,
                }
            })
            .collect();
        self.perf_snapshot.rules_cache_build_ms = start.elapsed().as_millis();
    }

    /// Recompute the filtered rules page indices. pub(crate) so the demo
    /// constructor can apply its empty filter once at boot.
    pub(crate) fn apply_rules_filter(&mut self) {
        let filter = self.rules_filter.trim().to_ascii_lowercase();
        self.rules_filtered_indices = if filter.is_empty() {
            (0..self.rules_render_cache.len()).collect()
        } else {
            self.rules_render_cache
                .iter()
                .enumerate()
                .filter_map(|(cache_index, item)| {
                    if item.search_key.contains(&filter) {
                        Some(cache_index)
                    } else {
                        None
                    }
                })
                .collect()
        };
        if self.rules_page_size == 0 {
            self.rules_page_size = 200;
        }
        let total_pages = if self.rules_filtered_indices.is_empty() {
            1
        } else {
            (self.rules_filtered_indices.len() - 1) / self.rules_page_size + 1
        };
        if self.rules_page >= total_pages {
            self.rules_page = total_pages.saturating_sub(1);
        }
        let start = self.rules_page.saturating_mul(self.rules_page_size);
        self.perf_snapshot.rules_visible_rows = self
            .rules_filtered_indices
            .len()
            .saturating_sub(start)
            .min(self.rules_page_size);
    }

    fn reset_rules_lazy_state(&mut self) {
        self.rule_providers_editor_state = EditorLazyState::Unloaded;
        self.proxy_providers_editor_state = EditorLazyState::Unloaded;
        self.sniffer_editor_state = EditorLazyState::Unloaded;
    }

    fn ensure_rule_providers_editor_loaded(&mut self) {
        if self.rule_providers_editor_state == EditorLazyState::Loaded
            && self.rule_providers_json_content.text() == self.rule_providers_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.rule_providers_json_content =
            iced::widget::text_editor::Content::with_text(&self.rule_providers_json_cache);
        self.rule_providers_editor_state = EditorLazyState::Loaded;
        self.perf_snapshot.rules_with_text_apply_ms = start.elapsed().as_millis();
    }

    fn ensure_proxy_providers_editor_loaded(&mut self) {
        if self.proxy_providers_editor_state == EditorLazyState::Loaded
            && self.proxy_providers_json_content.text() == self.proxy_providers_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.proxy_providers_json_content =
            iced::widget::text_editor::Content::with_text(&self.proxy_providers_json_cache);
        self.proxy_providers_editor_state = EditorLazyState::Loaded;
        self.perf_snapshot.rules_with_text_apply_ms = start.elapsed().as_millis();
    }

    fn ensure_sniffer_editor_loaded(&mut self) {
        if self.sniffer_editor_state == EditorLazyState::Loaded
            && self.sniffer_json_content.text() == self.sniffer_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.sniffer_json_content =
            iced::widget::text_editor::Content::with_text(&self.sniffer_json_cache);
        self.sniffer_editor_state = EditorLazyState::Loaded;
        self.perf_snapshot.rules_with_text_apply_ms = start.elapsed().as_millis();
    }

    /// Custom rules list plus rule/proxy provider and sniffer JSON editors.
    /// Unmatched messages fall through to the next domain in the
    /// `update_core` chain.
    pub(super) fn update_core_rules(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FilterRules(filter) => {
                self.rules_filter = filter;
                self.rules_page = 0;
                self.apply_rules_filter();
                Task::none()
            }
            Message::UpdateNewRuleType(t) => {
                self.new_rule_type = t;
                Task::none()
            }
            Message::UpdateNewRulePayload(p) => {
                self.new_rule_payload = p;
                Task::none()
            }
            Message::UpdateNewRuleTarget(t) => {
                self.new_rule_target = t;
                Task::none()
            }
            Message::AddCustomRule => {
                let payload = self.new_rule_payload.trim().to_string();
                if payload.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Payload cannot be empty".to_string(),
                        ToastStatus::Error,
                    ));
                }

                let entry = RuleEntry {
                    rule: format!(
                        "{},{},{}",
                        self.new_rule_type.clone(),
                        payload,
                        self.new_rule_target.clone()
                    ),
                    enabled: true,
                };
                self.is_adding_rule = true;
                Task::perform(
                    async move {
                        let mut rules = infiltrator_core::rules::load_rules()
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        rules.insert(0, entry);
                        infiltrator_core::rules::save_rules(rules)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::RuleAdded,
                )
            }
            Message::RuleAdded(result) => {
                self.is_adding_rule = false;
                match result {
                    Ok(_) => {
                        self.new_rule_payload.clear();
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            Task::done(Message::ShowToast(
                                "Rule added".to_string(),
                                ToastStatus::Success,
                            )),
                        ])
                    }
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::SetRulesTab(tab) => {
                self.rules_tab = tab;
                self.rules_page = 0;
                match tab {
                    RulesTab::JsonEditors => Task::done(match self.rules_json_tab {
                        RulesJsonTab::RuleProviders => Message::EnsureRuleProvidersEditorLoaded,
                        RulesJsonTab::ProxyProviders => Message::EnsureProxyProvidersEditorLoaded,
                        RulesJsonTab::Sniffer => Message::EnsureSnifferEditorLoaded,
                    }),
                    _ => Task::none(),
                }
            }
            Message::SetRulesJsonTab(tab) => {
                self.rules_json_tab = tab;
                Task::done(match tab {
                    RulesJsonTab::RuleProviders => Message::EnsureRuleProvidersEditorLoaded,
                    RulesJsonTab::ProxyProviders => Message::EnsureProxyProvidersEditorLoaded,
                    RulesJsonTab::Sniffer => Message::EnsureSnifferEditorLoaded,
                })
            }
            Message::ToggleRulesProvidersExpanded => {
                self.rules_providers_expanded = !self.rules_providers_expanded;
                Task::none()
            }
            Message::RulesPrevPage => {
                self.rules_page = self.rules_page.saturating_sub(1);
                Task::none()
            }
            Message::RulesNextPage => {
                let total_pages = if self.rules_filtered_indices.is_empty() {
                    1
                } else {
                    (self.rules_filtered_indices.len() - 1) / self.rules_page_size + 1
                };
                if self.rules_page + 1 < total_pages {
                    self.rules_page += 1;
                }
                Task::none()
            }
            Message::RulesSetPage(page) => {
                self.rules_page = page;
                self.apply_rules_filter();
                Task::none()
            }
            Message::EnsureRuleProvidersEditorLoaded => {
                self.ensure_rule_providers_editor_loaded();
                Task::none()
            }
            Message::EnsureProxyProvidersEditorLoaded => {
                self.ensure_proxy_providers_editor_loaded();
                Task::none()
            }
            Message::EnsureSnifferEditorLoaded => {
                self.ensure_sniffer_editor_loaded();
                Task::none()
            }
            Message::ActivateRulesHeavyView => {
                self.rules_heavy_ready = true;
                if self.rules_tab == RulesTab::JsonEditors {
                    Task::done(match self.rules_json_tab {
                        RulesJsonTab::RuleProviders => Message::EnsureRuleProvidersEditorLoaded,
                        RulesJsonTab::ProxyProviders => Message::EnsureProxyProvidersEditorLoaded,
                        RulesJsonTab::Sniffer => Message::EnsureSnifferEditorLoaded,
                    })
                } else {
                    Task::none()
                }
            }
            Message::LoadRules => {
                self.is_loading_rules = true;
                if !self.rules_loaded_once {
                    self.reset_rules_lazy_state();
                }
                let mut tasks = vec![Task::perform(
                    async {
                        let manager = mihomo_config::manager::ConfigManager::new()
                            .map_err(|e: mihomo_api::error::MihomoError| InfiltratorError::from(e))?;
                        let profile = manager
                            .get_current()
                            .await
                            .map_err(|e: mihomo_api::error::MihomoError| InfiltratorError::from(e))?;
                        let content = manager
                            .load(&profile)
                            .await
                            .map_err(|e: mihomo_api::error::MihomoError| InfiltratorError::from(e))?;
                        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        let rules = infiltrator_core::rules::extract_rules_from_doc(&doc)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let rule_providers =
                            infiltrator_core::rules::extract_rule_providers_from_doc(&doc)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let proxy_providers =
                            infiltrator_core::proxy_providers::extract_proxy_providers_from_doc(
                                &doc,
                            )
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let sniffer =
                            infiltrator_core::sniffer::extract_sniffer_config_from_doc(&doc)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        let rule_providers_json = serde_json::to_string_pretty(&rule_providers)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let proxy_providers_json =
                            serde_json::to_string_pretty(&proxy_providers)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let sniffer_json = serde_json::to_string_pretty(&sniffer)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        Ok(RulesLoadBundle {
                            rules,
                            rule_providers_json,
                            proxy_providers_json,
                            sniffer_json,
                        })
                    },
                    Message::RulesBundleLoaded,
                )];
                if let Some(rt) = self.runtime.clone() {
                    self.is_loading_providers = true;
                    tasks.push(Task::perform(
                        async move {
                            let proxies = rt
                                .client()
                                .get_proxy_providers()
                                .await
                                .map_err(InfiltratorError::from)?;
                            let rules = rt
                                .client()
                                .get_rule_providers()
                                .await
                                .map_err(InfiltratorError::from)?;
                            Ok((
                                proxies.into_values().collect(),
                                rules.into_values().collect(),
                            ))
                        },
                        Message::ProvidersLoaded,
                    ));
                } else {
                    self.is_loading_providers = false;
                }
                Task::batch(tasks)
            }
            Message::RulesBundleLoaded(result) => {
                self.is_loading_rules = false;
                match result {
                    Ok(bundle) => {
                        self.rules_loaded_once = true;
                        self.rules = bundle.rules;
                        self.rules_dirty = false;
                        self.rebuild_rules_render_cache();
                        self.apply_rules_filter();

                        self.rule_providers_json_cache = bundle.rule_providers_json;
                        if !self.rule_providers_json_dirty
                            && self.rule_providers_editor_state == EditorLazyState::Loaded
                            && self.rule_providers_json_content.text()
                                != self.rule_providers_json_cache
                        {
                            self.ensure_rule_providers_editor_loaded();
                            self.rule_providers_json_dirty = false;
                        }

                        self.proxy_providers_json_cache = bundle.proxy_providers_json;
                        if !self.proxy_providers_json_dirty
                            && self.proxy_providers_editor_state == EditorLazyState::Loaded
                            && self.proxy_providers_json_content.text()
                                != self.proxy_providers_json_cache
                        {
                            self.ensure_proxy_providers_editor_loaded();
                            self.proxy_providers_json_dirty = false;
                        }

                        self.sniffer_json_cache = bundle.sniffer_json;
                        if !self.sniffer_json_dirty
                            && self.sniffer_editor_state == EditorLazyState::Loaded
                            && self.sniffer_json_content.text() != self.sniffer_json_cache
                        {
                            self.ensure_sniffer_editor_loaded();
                            self.sniffer_json_dirty = false;
                        }
                    }
                    Err(e) => {
                        self.rules_loaded_once = false;
                        self.set_error(&e);
                    }
                }
                Task::none()
            }
            Message::RulesLoaded(result) => {
                self.is_loading_rules = false;
                match result {
                    Ok(rules) => {
                        self.rules_loaded_once = true;
                        self.rules = rules;
                        self.rules_dirty = false;
                        self.rebuild_rules_render_cache();
                        self.apply_rules_filter();
                    }
                    Err(e) => {
                        self.rules_loaded_once = false;
                        self.set_error(&e);
                    }
                }
                Task::none()
            }
            Message::RuleProvidersJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        self.rule_providers_json_cache = json;
                        if self.rule_providers_editor_state == EditorLazyState::Loaded {
                            self.ensure_rule_providers_editor_loaded();
                        }
                        self.rule_providers_json_dirty = false;
                    }
                    Err(e) => self.set_error(&e),
                }
                Task::none()
            }
            Message::ProxyProvidersJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        self.proxy_providers_json_cache = json;
                        if self.proxy_providers_editor_state == EditorLazyState::Loaded {
                            self.ensure_proxy_providers_editor_loaded();
                        }
                        self.proxy_providers_json_dirty = false;
                    }
                    Err(e) => self.set_error(&e),
                }
                Task::none()
            }
            Message::SnifferJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        self.sniffer_json_cache = json;
                        if self.sniffer_editor_state == EditorLazyState::Loaded {
                            self.ensure_sniffer_editor_loaded();
                        }
                        self.sniffer_json_dirty = false;
                    }
                    Err(e) => self.set_error(&e),
                }
                Task::none()
            }
            Message::ToggleRuleEnabled(index) => {
                if let Some(entry) = self.rules.get_mut(index) {
                    entry.enabled = !entry.enabled;
                    self.rules_dirty = true;
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::MoveRuleUp(index) => {
                if index > 0 && index < self.rules.len() {
                    self.rules.swap(index, index - 1);
                    self.rules_dirty = true;
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::MoveRuleDown(index) => {
                if index + 1 < self.rules.len() {
                    self.rules.swap(index, index + 1);
                    self.rules_dirty = true;
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::SaveRules => {
                let rules = self.rules.clone();
                self.is_saving_rules = true;
                self.begin_save_phase("Rules");
                Task::perform(
                    async move {
                        infiltrator_core::rules::save_rules(rules)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::RulesSaved,
                )
            }
            Message::RulesSaved(result) => {
                self.is_saving_rules = false;
                match result {
                    Ok(_) => {
                        self.rules_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.trigger_runtime_rebuild(),
                        ])
                    }
                    Err(e) => {
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "Rules".to_string(),
                            error: e.to_string(),
                        };
                        self.set_error(&e);
                        Task::batch(vec![
                            Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error)),
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
            Message::RuleProvidersEditorAction(action) => {
                self.ensure_rule_providers_editor_loaded();
                self.rule_providers_json_content.perform(action);
                self.rule_providers_json_dirty = true;
                Task::none()
            }
            Message::SaveRuleProvidersJson => {
                self.ensure_rule_providers_editor_loaded();
                let text = self.rule_providers_json_content.text();
                self.is_saving_rule_providers_json = true;
                self.begin_save_phase("Rule Providers");
                Task::perform(
                    async move {
                        let providers = serde_json::from_str::<
                            infiltrator_core::rules::RuleProviders,
                        >(&text)
                        .map_err(|e| {
                            InfiltratorError::Config(format!("Invalid rule providers JSON: {}", e))
                        })?;
                        infiltrator_core::rules::save_rule_providers(providers)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::RuleProvidersJsonSaved,
                )
            }
            Message::RuleProvidersJsonSaved(result) => {
                self.is_saving_rule_providers_json = false;
                match result {
                    Ok(_) => {
                        self.rule_providers_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.trigger_runtime_rebuild(),
                        ])
                    }
                    Err(e) => {
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "Rule Providers".to_string(),
                            error: e.to_string(),
                        };
                        self.set_error(&e);
                        Task::batch(vec![
                            Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error)),
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
            Message::ProxyProvidersEditorAction(action) => {
                self.ensure_proxy_providers_editor_loaded();
                self.proxy_providers_json_content.perform(action);
                self.proxy_providers_json_dirty = true;
                Task::none()
            }
            Message::SaveProxyProvidersJson => {
                self.ensure_proxy_providers_editor_loaded();
                let text = self.proxy_providers_json_content.text();
                self.is_saving_proxy_providers_json = true;
                self.begin_save_phase("Proxy Providers");
                Task::perform(
                    async move {
                        let providers = serde_json::from_str::<
                            infiltrator_core::proxy_providers::ProxyProviders,
                        >(&text)
                        .map_err(|e| {
                            InfiltratorError::Config(format!("Invalid proxy providers JSON: {}", e))
                        })?;
                        infiltrator_core::proxy_providers::save_proxy_providers(providers)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::ProxyProvidersJsonSaved,
                )
            }
            Message::ProxyProvidersJsonSaved(result) => {
                self.is_saving_proxy_providers_json = false;
                match result {
                    Ok(_) => {
                        self.proxy_providers_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.trigger_runtime_rebuild(),
                        ])
                    }
                    Err(e) => {
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "Proxy Providers".to_string(),
                            error: e.to_string(),
                        };
                        self.set_error(&e);
                        Task::batch(vec![
                            Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error)),
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
            Message::SnifferEditorAction(action) => {
                self.ensure_sniffer_editor_loaded();
                self.sniffer_json_content.perform(action);
                self.sniffer_json_dirty = true;
                Task::none()
            }
            Message::SaveSnifferJson => {
                self.ensure_sniffer_editor_loaded();
                let text = self.sniffer_json_content.text();
                self.is_saving_sniffer_json = true;
                self.begin_save_phase("Sniffer");
                Task::perform(
                    async move {
                        let config =
                            serde_json::from_str::<serde_json::Value>(&text).map_err(|e| {
                                InfiltratorError::Config(format!("Invalid sniffer JSON: {}", e))
                            })?;
                        infiltrator_core::sniffer::save_sniffer_config(config)
                            .await
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        Ok(())
                    },
                    Message::SnifferJsonSaved,
                )
            }
            Message::SnifferJsonSaved(result) => {
                self.is_saving_sniffer_json = false;
                match result {
                    Ok(_) => {
                        self.sniffer_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.trigger_runtime_rebuild(),
                        ])
                    }
                    Err(e) => {
                        self.rebuild_flow = RebuildFlowState::Failed {
                            label: "Sniffer".to_string(),
                            error: e.to_string(),
                        };
                        self.set_error(&e);
                        Task::batch(vec![
                            Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error)),
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
            Message::ProvidersLoaded(result) => {
                self.is_loading_providers = false;
                match result {
                    Ok((proxies, rules)) => {
                        self.proxy_providers = proxies;
                        self.rule_providers = rules;
                    }
                    Err(e) => self.set_error(&e),
                }
                Task::none()
            }
            Message::UpdateProxyProvider(name) => {
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .update_proxy_provider(&name)
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::UpdateRuleProvider(name) => {
                if let Some(rt) = self.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .update_rule_provider(&name)
                                .await
                                .map_err(InfiltratorError::from)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            other => self.update_core_advanced(other),
        }
    }
}
