//! JSON sub-editors for Rules: Rule Providers, Proxy Providers and Sniffer JSON tabs.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::runtime::RebuildFlowState;
use crate::update::core::profile_apply::save_task;
use iced::Task;

impl AppState {
    pub(super) fn ensure_rule_providers_editor_loaded(&mut self) {
        if self.editor.rule_providers_editor_state == EditorLazyState::Loaded
            && self.editor.rule_providers_json_content.text()
                == self.editor.rule_providers_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.editor.rule_providers_json_content =
            iced::widget::text_editor::Content::with_text(&self.editor.rule_providers_json_cache);
        self.editor.rule_providers_editor_state = EditorLazyState::Loaded;
        self.diag.perf_snapshot.rules_with_text_apply_ms = start.elapsed().as_millis();
    }

    pub(super) fn ensure_proxy_providers_editor_loaded(&mut self) {
        if self.editor.proxy_providers_editor_state == EditorLazyState::Loaded
            && self.editor.proxy_providers_json_content.text()
                == self.editor.proxy_providers_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.editor.proxy_providers_json_content =
            iced::widget::text_editor::Content::with_text(&self.editor.proxy_providers_json_cache);
        self.editor.proxy_providers_editor_state = EditorLazyState::Loaded;
        self.diag.perf_snapshot.rules_with_text_apply_ms = start.elapsed().as_millis();
    }

    pub(super) fn ensure_sniffer_editor_loaded(&mut self) {
        if self.editor.sniffer_editor_state == EditorLazyState::Loaded
            && self.editor.sniffer_json_content.text() == self.editor.sniffer_json_cache
        {
            return;
        }
        let start = std::time::Instant::now();
        self.editor.sniffer_json_content =
            iced::widget::text_editor::Content::with_text(&self.editor.sniffer_json_cache);
        self.editor.sniffer_editor_state = EditorLazyState::Loaded;
        self.diag.perf_snapshot.rules_with_text_apply_ms = start.elapsed().as_millis();
    }

    pub(super) fn update_core_json_editors(&mut self, message: Message) -> Task<Message> {
        match message {
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
            Message::RuleProvidersEditorAction(action) => {
                self.ensure_rule_providers_editor_loaded();
                self.editor.rule_providers_json_content.perform(action);
                self.editor.rule_providers_json_dirty = true;
                Task::none()
            }
            Message::SaveRuleProvidersJson => {
                self.ensure_rule_providers_editor_loaded();
                let text = self.editor.rule_providers_json_content.text();
                self.editor.is_saving_rule_providers_json = true;
                self.begin_save_phase("Rule Providers");
                save_task(
                    self.runtime.runtime.clone(),
                    move |content| {
                        let providers =
                            serde_json::from_str::<infiltrator_domain::rules::RuleProviders>(&text)
                                .map_err(|e| anyhow::anyhow!("Invalid rule providers JSON: {e}"))?;
                        infiltrator_domain::rules::apply_rule_providers_to_yaml(content, &providers)
                    },
                    Message::RuleProvidersJsonSaved,
                )
            }
            Message::RuleProvidersJsonSaved(result) => {
                self.editor.is_saving_rule_providers_json = false;
                match result {
                    Ok(_) => {
                        self.editor.rule_providers_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.finish_without_rebuild("Rule Providers".to_string()),
                        ])
                    }
                    Err(e) => {
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
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
                self.editor.proxy_providers_json_content.perform(action);
                self.editor.proxy_providers_json_dirty = true;
                Task::none()
            }
            Message::SaveProxyProvidersJson => {
                self.ensure_proxy_providers_editor_loaded();
                let text = self.editor.proxy_providers_json_content.text();
                self.editor.is_saving_proxy_providers_json = true;
                self.begin_save_phase("Proxy Providers");
                save_task(
                    self.runtime.runtime.clone(),
                    move |content| {
                        let providers = serde_json::from_str::<
                            infiltrator_domain::proxy_providers::ProxyProviders,
                        >(&text)
                        .map_err(|e| anyhow::anyhow!("Invalid proxy providers JSON: {e}"))?;
                        infiltrator_domain::proxy_providers::apply_proxy_providers_to_yaml(
                            content, &providers,
                        )
                    },
                    Message::ProxyProvidersJsonSaved,
                )
            }
            Message::ProxyProvidersJsonSaved(result) => {
                self.editor.is_saving_proxy_providers_json = false;
                match result {
                    Ok(_) => {
                        self.editor.proxy_providers_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.finish_without_rebuild("Proxy Providers".to_string()),
                        ])
                    }
                    Err(e) => {
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
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
                self.editor.sniffer_json_content.perform(action);
                self.editor.sniffer_json_dirty = true;
                Task::none()
            }
            Message::SaveSnifferJson => {
                self.ensure_sniffer_editor_loaded();
                let text = self.editor.sniffer_json_content.text();
                self.editor.is_saving_sniffer_json = true;
                self.begin_save_phase("Sniffer");
                save_task(
                    self.runtime.runtime.clone(),
                    move |content| {
                        let config = serde_json::from_str::<serde_json::Value>(&text)
                            .map_err(|e| anyhow::anyhow!("Invalid sniffer JSON: {e}"))?;
                        infiltrator_domain::sniffer::apply_sniffer_to_yaml(content, &config)
                    },
                    Message::SnifferJsonSaved,
                )
            }
            Message::SnifferJsonSaved(result) => {
                self.editor.is_saving_sniffer_json = false;
                match result {
                    Ok(_) => {
                        self.editor.sniffer_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.finish_without_rebuild("Sniffer".to_string()),
                        ])
                    }
                    Err(e) => {
                        self.runtime.rebuild_flow = RebuildFlowState::Failed {
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
            other => self.update_core_mrs(other),
        }
    }
}
