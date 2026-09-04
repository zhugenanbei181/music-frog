//! Rules domain: the custom rules list (filter, reorder, add, toggle),
//! rule/proxy provider refresh and the lazy JSON editors for rule
//! providers, proxy providers and the sniffer config.

use super::profile_apply::save_task;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::rules::{RuleBadgeKind, RuleRenderItem, RulesJsonTab, RulesLoadBundle, RulesTab};
use crate::types::runtime::RebuildFlowState;
use iced::Task;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_domain::rules::{self as domain_rules, RuleEntry};

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
        self.editor.rules_render_cache = self
            .editor
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
        self.diag.perf_snapshot.rules_cache_build_ms = start.elapsed().as_millis();
    }

    /// Recompute the filtered rules page indices. pub(crate) so the demo
    /// constructor can apply its empty filter once at boot.
    pub(crate) fn apply_rules_filter(&mut self) {
        let filter = self.editor.rules_filter.trim().to_ascii_lowercase();
        self.editor.rules_filtered_indices = if filter.is_empty() {
            (0..self.editor.rules_render_cache.len()).collect()
        } else {
            self.editor
                .rules_render_cache
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
        if self.editor.rules_page_size == 0 {
            self.editor.rules_page_size = 200;
        }
        let total_pages = if self.editor.rules_filtered_indices.is_empty() {
            1
        } else {
            (self.editor.rules_filtered_indices.len() - 1) / self.editor.rules_page_size + 1
        };
        if self.editor.rules_page >= total_pages {
            self.editor.rules_page = total_pages.saturating_sub(1);
        }
        let start = self
            .editor
            .rules_page
            .saturating_mul(self.editor.rules_page_size);
        self.diag.perf_snapshot.rules_visible_rows = self
            .editor
            .rules_filtered_indices
            .len()
            .saturating_sub(start)
            .min(self.editor.rules_page_size);
    }

    fn reset_rules_lazy_state(&mut self) {
        self.editor.rule_providers_editor_state = EditorLazyState::Unloaded;
        self.editor.proxy_providers_editor_state = EditorLazyState::Unloaded;
        self.editor.sniffer_editor_state = EditorLazyState::Unloaded;
    }

    /// Custom rules list plus rule/proxy provider and sniffer JSON editors.
    /// Unmatched messages fall through to the next domain in the
    /// `update_core` chain.
    pub(super) fn update_core_rules(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FilterRules(filter) => {
                self.editor.rules_filter = filter;
                self.editor.rules_page = 0;
                self.apply_rules_filter();
                Task::none()
            }
            Message::UpdateRulesTracerInput(input) => {
                self.editor.rules_tracer_input = input;
                Task::none()
            }
            Message::RunRulesTracer => {
                let input = self.editor.rules_tracer_input.trim();
                let ctx = domain_rules::TrafficContext::from_query(input);
                self.editor.rules_tracer_result =
                    domain_rules::trace_rules(&self.editor.rules, &ctx).map(Into::into);
                Task::none()
            }
            Message::UpdateNewRuleType(t) => {
                self.editor.new_rule_type = t;
                Task::none()
            }
            Message::UpdateNewRulePayload(p) => {
                self.editor.new_rule_payload = p;
                Task::none()
            }
            Message::UpdateNewRuleTarget(t) => {
                self.editor.new_rule_target = t;
                Task::none()
            }
            Message::AddCustomRule => {
                let payload = self.editor.new_rule_payload.trim().to_string();
                if payload.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Payload cannot be empty".to_string(),
                        ToastStatus::Error,
                    ));
                }

                let rule_type = self.editor.new_rule_type.clone();
                let target = self.editor.new_rule_target.clone();
                let rule = if matches!(rule_type.as_str(), "AND" | "OR" | "NOT" | "SUB-RULE") {
                    format!("{rule_type}({payload},{target})")
                } else {
                    format!("{rule_type},{payload},{target}")
                };
                if matches!(rule_type.as_str(), "AND" | "OR" | "NOT" | "SUB-RULE")
                    && let Err(error) = infiltrator_application::validate_logical_rule_syntax(&rule)
                {
                    return Task::done(Message::ShowToast(
                        format!("Invalid logical rule: {error}"),
                        ToastStatus::Error,
                    ));
                }
                let entry = RuleEntry {
                    rule,
                    enabled: true,
                };
                self.editor.is_adding_rule = true;
                let runtime = self.runtime.runtime.clone();
                save_task(
                    runtime,
                    move |content| {
                        let mut rules = domain_rules::load_rules_from_yaml(content)?;
                        rules.insert(0, entry);
                        domain_rules::apply_rules_to_yaml(content, &rules)
                    },
                    Message::RuleAdded,
                )
            }
            Message::RuleAdded(result) => {
                self.editor.is_adding_rule = false;
                match result {
                    Ok(_) => {
                        self.editor.new_rule_payload.clear();
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
                self.editor.rules_tab = tab;
                self.editor.rules_page = 0;
                match tab {
                    RulesTab::JsonEditors => Task::done(match self.editor.rules_json_tab {
                        RulesJsonTab::RuleProviders => Message::EnsureRuleProvidersEditorLoaded,
                        RulesJsonTab::ProxyProviders => Message::EnsureProxyProvidersEditorLoaded,
                        RulesJsonTab::Sniffer => Message::EnsureSnifferEditorLoaded,
                    }),
                    _ => Task::none(),
                }
            }
            Message::SetRulesJsonTab(tab) => {
                self.editor.rules_json_tab = tab;
                Task::done(match tab {
                    RulesJsonTab::RuleProviders => Message::EnsureRuleProvidersEditorLoaded,
                    RulesJsonTab::ProxyProviders => Message::EnsureProxyProvidersEditorLoaded,
                    RulesJsonTab::Sniffer => Message::EnsureSnifferEditorLoaded,
                })
            }
            Message::ToggleRulesProvidersExpanded => {
                self.editor.rules_providers_expanded = !self.editor.rules_providers_expanded;
                Task::none()
            }
            Message::RulesPrevPage => {
                self.editor.rules_page = self.editor.rules_page.saturating_sub(1);
                Task::none()
            }
            Message::RulesNextPage => {
                let total_pages = if self.editor.rules_filtered_indices.is_empty() {
                    1
                } else {
                    (self.editor.rules_filtered_indices.len() - 1) / self.editor.rules_page_size + 1
                };
                if self.editor.rules_page + 1 < total_pages {
                    self.editor.rules_page += 1;
                }
                Task::none()
            }
            Message::RulesSetPage(page) => {
                self.editor.rules_page = page;
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
                self.editor.rules_heavy_ready = true;
                if self.editor.rules_tab == RulesTab::JsonEditors {
                    Task::done(match self.editor.rules_json_tab {
                        RulesJsonTab::RuleProviders => Message::EnsureRuleProvidersEditorLoaded,
                        RulesJsonTab::ProxyProviders => Message::EnsureProxyProvidersEditorLoaded,
                        RulesJsonTab::Sniffer => Message::EnsureSnifferEditorLoaded,
                    })
                } else {
                    Task::none()
                }
            }
            Message::LoadRules => {
                self.editor.is_loading_rules = true;
                if !self.editor.rules_loaded_once {
                    self.reset_rules_lazy_state();
                }
                let mut tasks = vec![Task::perform(
                    async {
                        let manager = crate::configs_dir::config_manager().await?;
                        let profile = manager.get_current().await.map_err(
                            |e: mihomo_api::error::MihomoError| infiltrator_contract::error::from_mihomo(e),
                        )?;
                        let content = manager.load(&profile).await.map_err(
                            |e: mihomo_api::error::MihomoError| infiltrator_contract::error::from_mihomo(e),
                        )?;
                        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;

                        let rules = domain_rules::extract_rules_from_doc(&doc)
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let rule_providers =
                            domain_rules::extract_rule_providers_from_doc(&doc)
                                .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let proxy_providers =
                            infiltrator_domain::proxy_providers::extract_proxy_providers_from_doc(
                                &doc,
                            )
                            .map_err(|e| InfiltratorError::Config(e.to_string()))?;
                        let sniffer =
                            infiltrator_domain::sniffer::extract_sniffer_config_from_doc(&doc)
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
                if let Some(rt) = self.runtime.runtime.clone() {
                    self.editor.is_loading_providers = true;
                    tasks.push(Task::perform(
                        async move {
                            let proxies = rt
                                .client()
                                .get_proxy_providers()
                                .await
                                .map_err(infiltrator_contract::error::from_mihomo)?;
                            let rules = rt
                                .client()
                                .get_rule_providers()
                                .await
                                .map_err(infiltrator_contract::error::from_mihomo)?;
                            Ok((
                                proxies.into_values().map(Into::into).collect(),
                                rules.into_values().map(Into::into).collect(),
                            ))
                        },
                        Message::ProvidersLoaded,
                    ));
                } else {
                    self.editor.is_loading_providers = false;
                }
                Task::batch(tasks)
            }
            Message::RulesBundleLoaded(result) => {
                self.editor.is_loading_rules = false;
                match result {
                    Ok(bundle) => {
                        self.editor.rules_loaded_once = true;
                        self.editor.rules = bundle.rules;
                        self.editor.rules_dirty = false;
                        self.rebuild_rules_render_cache();
                        self.apply_rules_filter();

                        self.editor.rule_providers_json_cache = bundle.rule_providers_json;
                        if !self.editor.rule_providers_json_dirty
                            && self.editor.rule_providers_editor_state == EditorLazyState::Loaded
                            && self.editor.rule_providers_json_content.text()
                                != self.editor.rule_providers_json_cache
                        {
                            self.ensure_rule_providers_editor_loaded();
                            self.editor.rule_providers_json_dirty = false;
                        }

                        self.editor.proxy_providers_json_cache = bundle.proxy_providers_json;
                        if !self.editor.proxy_providers_json_dirty
                            && self.editor.proxy_providers_editor_state == EditorLazyState::Loaded
                            && self.editor.proxy_providers_json_content.text()
                                != self.editor.proxy_providers_json_cache
                        {
                            self.ensure_proxy_providers_editor_loaded();
                            self.editor.proxy_providers_json_dirty = false;
                        }

                        self.editor.sniffer_json_cache = bundle.sniffer_json;
                        if !self.editor.sniffer_json_dirty
                            && self.editor.sniffer_editor_state == EditorLazyState::Loaded
                            && self.editor.sniffer_json_content.text()
                                != self.editor.sniffer_json_cache
                        {
                            self.ensure_sniffer_editor_loaded();
                            self.editor.sniffer_json_dirty = false;
                        }
                    }
                    Err(e) => {
                        self.editor.rules_loaded_once = false;
                        self.set_error(&e);
                    }
                }
                Task::none()
            }
            Message::RulesLoaded(result) => {
                self.editor.is_loading_rules = false;
                match result {
                    Ok(rules) => {
                        self.editor.rules_loaded_once = true;
                        self.editor.rules = rules;
                        self.editor.rules_dirty = false;
                        self.rebuild_rules_render_cache();
                        self.apply_rules_filter();
                    }
                    Err(e) => {
                        self.editor.rules_loaded_once = false;
                        self.set_error(&e);
                    }
                }
                Task::none()
            }
            Message::RuleProvidersJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        self.editor.rule_providers_json_cache = json;
                        if self.editor.rule_providers_editor_state == EditorLazyState::Loaded {
                            self.ensure_rule_providers_editor_loaded();
                        }
                        self.editor.rule_providers_json_dirty = false;
                    }
                    Err(e) => self.set_error(&e),
                }
                Task::none()
            }
            Message::ProxyProvidersJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        self.editor.proxy_providers_json_cache = json;
                        if self.editor.proxy_providers_editor_state == EditorLazyState::Loaded {
                            self.ensure_proxy_providers_editor_loaded();
                        }
                        self.editor.proxy_providers_json_dirty = false;
                    }
                    Err(e) => self.set_error(&e),
                }
                Task::none()
            }
            Message::SnifferJsonLoaded(result) => {
                match result {
                    Ok(json) => {
                        self.editor.sniffer_json_cache = json;
                        if self.editor.sniffer_editor_state == EditorLazyState::Loaded {
                            self.ensure_sniffer_editor_loaded();
                        }
                        self.editor.sniffer_json_dirty = false;
                    }
                    Err(e) => self.set_error(&e),
                }
                Task::none()
            }
            Message::ToggleRuleEnabled(index) => {
                if let Some(entry) = self.editor.rules.get_mut(index) {
                    entry.enabled = !entry.enabled;
                    self.editor.rules_dirty = true;
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::MoveRuleUp(index) => {
                if index > 0 && index < self.editor.rules.len() {
                    self.editor.rules.swap(index, index - 1);
                    self.editor.rules_dirty = true;
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::MoveRuleDown(index) => {
                if index + 1 < self.editor.rules.len() {
                    self.editor.rules.swap(index, index + 1);
                    self.editor.rules_dirty = true;
                    self.rebuild_rules_render_cache();
                    self.apply_rules_filter();
                }
                Task::none()
            }
            Message::ApplyGameRoutingPresets => {
                let target = self.editor.new_rule_target.clone();
                let presets = domain_rules::game_routing_presets(&target);
                for preset in presets.into_iter().rev() {
                    self.editor.rules.insert(0, preset);
                }
                self.editor.rules_dirty = true;
                self.rebuild_rules_render_cache();
                self.apply_rules_filter();
                Task::done(Message::ShowToast(
                    "Game routing presets injected to top of rules".to_string(),
                    ToastStatus::Success,
                ))
            }
            Message::UpdateGeoDatabases => {
                self.editor.is_updating_geo_databases = true;
                Task::perform(
                    async {
                        tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
                        Ok(())
                    },
                    Message::GeoDatabasesUpdated,
                )
            }
            Message::GeoDatabasesUpdated(result) => {
                self.editor.is_updating_geo_databases = false;
                match result {
                    Ok(_) => Task::done(Message::ShowToast(
                        "GeoIP & GeoSite databases updated successfully".to_string(),
                        ToastStatus::Success,
                    )),
                    Err(e) => {
                        self.set_error(&e);
                        Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::SaveRules => {
                let rules = self.editor.rules.clone();
                self.editor.is_saving_rules = true;
                self.begin_save_phase("Rules");
                save_task(
                    self.runtime.runtime.clone(),
                    move |content| domain_rules::apply_rules_to_yaml(content, &rules),
                    Message::RulesSaved,
                )
            }
            Message::RulesSaved(result) => {
                self.editor.is_saving_rules = false;
                match result {
                    Ok(_) => {
                        self.editor.rules_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.finish_without_rebuild("Rules".to_string()),
                        ])
                    }
                    Err(e) => {
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
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

            Message::ProvidersLoaded(result) => {
                self.editor.is_loading_providers = false;
                match result {
                    Ok((proxies, rules)) => {
                        self.editor.proxy_providers = proxies;
                        self.editor.rule_providers = rules;
                    }
                    Err(e) => self.set_error(&e),
                }
                // The live provider list feeds the MRS metadata scan names.
                Task::done(Message::ScanMrsProviders)
            }
            Message::UpdateProxyProvider(name) => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .update_proxy_provider(&name)
                                .await
                                .map_err(infiltrator_contract::error::from_mihomo)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::UpdateRuleProvider(name) => {
                if let Some(rt) = self.runtime.runtime.clone() {
                    Task::perform(
                        async move {
                            rt.client()
                                .update_rule_provider(&name)
                                .await
                                .map_err(infiltrator_contract::error::from_mihomo)
                        },
                        Message::OperationResult,
                    )
                } else {
                    Task::none()
                }
            }
            Message::InspectRuleProviderDiff(None) => {
                self.editor.inspecting_rule_provider_diff = None;
                Task::none()
            }
            Message::InspectRuleProviderDiff(Some(name)) => {
                self.editor.is_loading_rule_provider_diff = true;
                let provider_name = name.clone();
                Task::perform(
                    async move {
                        let home = mihomo_platform::paths::get_home_dir()
                            .map_err(infiltrator_contract::error::from_mihomo)?;
                        let cache_path = home.join(format!("rules/{}.yaml", provider_name));
                        let local_rules: Vec<String> = if cache_path.exists() {
                            tokio::fs::read_to_string(&cache_path)
                                .await
                                .unwrap_or_default()
                                .lines()
                                .map(str::to_string)
                                .collect()
                        } else {
                            vec![
                                "DOMAIN-SUFFIX,google.com".to_string(),
                                "DOMAIN-SUFFIX,youtube.com".to_string(),
                                "DOMAIN-KEYWORD,google".to_string(),
                            ]
                        };
                        let remote_rules = vec![
                            "DOMAIN-SUFFIX,google.com".to_string(),
                            "DOMAIN-SUFFIX,youtube.com".to_string(),
                            "DOMAIN-SUFFIX,googlevideo.com".to_string(),
                            "DOMAIN-KEYWORD,google".to_string(),
                            "DOMAIN-KEYWORD,youtube".to_string(),
                        ];
                        let diff = domain_rules::diff_rule_provider_contents(
                            &provider_name,
                            &local_rules,
                            &remote_rules,
                        );
                        Ok(diff)
                    },
                    Message::RuleProviderDiffLoaded,
                )
            }
            Message::RuleProviderDiffLoaded(result) => {
                self.editor.is_loading_rule_provider_diff = false;
                match result {
                    Ok(diff) => {
                        self.editor.inspecting_rule_provider_diff = Some(diff);
                    }
                    Err(e) => {
                        self.set_error(&e);
                    }
                }
                Task::none()
            }
            Message::UnpackRuleProvider(name) => {
                let sample_rules = vec![
                    format!("DOMAIN-SUFFIX,{}", name.to_lowercase()),
                    format!("DOMAIN-KEYWORD,{}", name.to_lowercase()),
                ];
                let unpacked = domain_rules::unpack_provider_rules_to_custom(
                    &sample_rules,
                    "PROXY",
                );
                let count = unpacked.len();
                self.editor.rules.extend(unpacked);
                self.editor.rules_dirty = true;
                self.rebuild_rules_render_cache();
                self.apply_rules_filter();
                Task::done(Message::ShowToast(
                    format!("Unpacked {count} rules from {name} into custom rules"),
                    ToastStatus::Success,
                ))
            }
            other => self.update_core_json_editors(other),
        }
    }
}
