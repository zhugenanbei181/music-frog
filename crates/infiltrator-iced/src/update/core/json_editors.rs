//! JSON sub-editors for Rules: Rule Providers, Proxy Providers and Sniffer JSON tabs.

use crate::state::AppState;
use crate::state::editor::{
    ProxyProvidersJsonSavePhase, RuleProvidersJsonSavePhase, SnifferJsonSavePhase,
};
use crate::types::app::ToastStatus;
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::runtime::ConfigurationSaveState;
use iced::Task;
use iced::widget::text_editor::Content;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure, InfiltratorError};
use infiltrator_contract::profile_configuration::{
    ProfileConfigurationRequest, ProfileConfigurationUpdate,
};
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::rules_workspace::RulesJsonSection;
use std::time::Instant;
use tokio::time;
use tokio::time::sleep;

impl AppState {
    pub(super) fn ensure_rule_providers_editor_loaded(&mut self) {
        if self.editor.rule_providers_editor_state == EditorLazyState::Loaded
            && self.editor.rule_providers_json_content.text()
                == self.editor.rule_providers_json_cache
        {
            return;
        }
        let start = Instant::now();
        self.editor.rule_providers_json_content =
            Content::with_text(&self.editor.rule_providers_json_cache);
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
        let start = Instant::now();
        self.editor.proxy_providers_json_content =
            Content::with_text(&self.editor.proxy_providers_json_cache);
        self.editor.proxy_providers_editor_state = EditorLazyState::Loaded;
        self.diag.perf_snapshot.rules_with_text_apply_ms = start.elapsed().as_millis();
    }

    pub(super) fn ensure_sniffer_editor_loaded(&mut self) {
        if self.editor.sniffer_editor_state == EditorLazyState::Loaded
            && self.editor.sniffer_json_content.text() == self.editor.sniffer_json_cache
        {
            return;
        }
        let start = Instant::now();
        self.editor.sniffer_json_content = Content::with_text(&self.editor.sniffer_json_cache);
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
                self.editor.rule_providers_json_save_phase = RuleProvidersJsonSavePhase::Saving;
                self.begin_configuration_save("Rule Providers");
                self.save_rules_configuration(
                    RulesJsonSection::RuleProviders,
                    text,
                    Message::RuleProvidersJsonSaved,
                )
            }
            Message::RuleProvidersJsonSaved(result) => {
                self.editor.rule_providers_json_save_phase = RuleProvidersJsonSavePhase::Idle;
                match result {
                    Ok(_) => {
                        self.editor.rule_providers_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.finish_configuration_save("Rule Providers".to_string()),
                        ])
                    }
                    Err(e) => {
                        self.runtime.configuration_save_state = ConfigurationSaveState::Failed {
                            label: "Rule Providers".to_string(),
                            error: e.to_string(),
                        };
                        self.set_error(&e);
                        Task::batch(vec![
                            Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error)),
                            Task::perform(
                                async {
                                    sleep(time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearConfigurationSaveState,
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
                self.editor.proxy_providers_json_save_phase = ProxyProvidersJsonSavePhase::Saving;
                self.begin_configuration_save("Proxy Providers");
                self.save_rules_configuration(
                    RulesJsonSection::ProxyProviders,
                    text,
                    Message::ProxyProvidersJsonSaved,
                )
            }
            Message::ProxyProvidersJsonSaved(result) => {
                self.editor.proxy_providers_json_save_phase = ProxyProvidersJsonSavePhase::Idle;
                match result {
                    Ok(_) => {
                        self.editor.proxy_providers_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.finish_configuration_save("Proxy Providers".to_string()),
                        ])
                    }
                    Err(e) => {
                        self.runtime.configuration_save_state = ConfigurationSaveState::Failed {
                            label: "Proxy Providers".to_string(),
                            error: e.to_string(),
                        };
                        self.set_error(&e);
                        Task::batch(vec![
                            Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error)),
                            Task::perform(
                                async {
                                    sleep(time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearConfigurationSaveState,
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
                self.editor.sniffer_json_save_phase = SnifferJsonSavePhase::Saving;
                self.begin_configuration_save("Sniffer");
                self.save_rules_configuration(
                    RulesJsonSection::Sniffer,
                    text,
                    Message::SnifferJsonSaved,
                )
            }
            Message::SnifferJsonSaved(result) => {
                self.editor.sniffer_json_save_phase = SnifferJsonSavePhase::Idle;
                match result {
                    Ok(_) => {
                        self.editor.sniffer_json_dirty = false;
                        Task::batch(vec![
                            Task::done(Message::LoadRules),
                            self.finish_configuration_save("Sniffer".to_string()),
                        ])
                    }
                    Err(e) => {
                        self.runtime.configuration_save_state = ConfigurationSaveState::Failed {
                            label: "Sniffer".to_string(),
                            error: e.to_string(),
                        };
                        self.set_error(&e);
                        Task::batch(vec![
                            Task::done(Message::ShowToast(e.to_string(), ToastStatus::Error)),
                            Task::perform(
                                async {
                                    sleep(time::Duration::from_secs(4)).await;
                                },
                                |_| Message::ClearConfigurationSaveState,
                            ),
                        ])
                    }
                }
            }
            other => self.update_core_mrs(other),
        }
    }
}

impl AppState {
    fn save_rules_configuration(
        &self,
        section: RulesJsonSection,
        text: String,
        result_message: fn(Result<(), InfiltratorError>) -> Message,
    ) -> Task<Message> {
        let identity = match self.rules_json_identity(section) {
            Some(identity) => identity,
            None => {
                return Task::done(result_message(Err(InfiltratorError::Config(
                    "The rules editor has no observed profile revision".to_owned(),
                ))));
            }
        };
        let update = match typed_rules_update(section, &text) {
            Ok(update) => update,
            Err(error) => return Task::done(result_message(Err(error))),
        };
        let Some(commands) = self.command_ingress() else {
            return Task::done(result_message(Err(InfiltratorError::Config(
                Failure::new(
                    ErrorCode::NotReady,
                    "The profile configuration command service is unavailable",
                    true,
                )
                .message,
            ))));
        };
        Task::perform(
            async move {
                commands
                    .execute(CommandIntent::ApplyProfileConfiguration {
                        request: Box::new(ProfileConfigurationRequest { identity, update }),
                    })
                    .await
                    .into_unit()
                    .map_err(|failure| InfiltratorError::Config(failure.message))
            },
            result_message,
        )
    }

    fn rules_json_identity(&self, section: RulesJsonSection) -> Option<ProfileDocumentIdentity> {
        match section {
            RulesJsonSection::RuleProviders => self.editor.rule_providers_json_identity.clone(),
            RulesJsonSection::ProxyProviders => self.editor.proxy_providers_json_identity.clone(),
            RulesJsonSection::Sniffer => self.editor.sniffer_json_identity.clone(),
        }
    }
}

fn typed_rules_update(
    section: RulesJsonSection,
    text: &str,
) -> Result<ProfileConfigurationUpdate, InfiltratorError> {
    match section {
        RulesJsonSection::RuleProviders => serde_json::from_str(text)
            .map(ProfileConfigurationUpdate::RuleProviders)
            .map_err(|error| {
                InfiltratorError::Config(format!("Invalid rule providers JSON: {error}"))
            }),
        RulesJsonSection::ProxyProviders => serde_json::from_str(text)
            .map(ProfileConfigurationUpdate::ProxyProviders)
            .map_err(|error| {
                InfiltratorError::Config(format!("Invalid proxy providers JSON: {error}"))
            }),
        RulesJsonSection::Sniffer => serde_json::from_str(text)
            .map(ProfileConfigurationUpdate::Sniffer)
            .map_err(|error| InfiltratorError::Config(format!("Invalid sniffer JSON: {error}"))),
    }
}
