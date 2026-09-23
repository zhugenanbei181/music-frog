//! Rule-provider unpack, local cache purge and local-vs-controller diff
//! (DUAL-11-06/07).
//!
//! Every action here resolves through the shared application service
//! (`infiltrator_application::rule_provider_application`) and the shared
//! `infiltrator_domain::rules::provider_store` reductions, so the surface can
//! neither invent a provider payload nor claim a purge that never happened.
//! The Iced editor keeps its local `rules` draft dirty and persists it through
//! the existing `SaveRules` path, exactly like the other DUAL-11 rule edits.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::rule_provider_application::RuleProviderApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::provider_cache::ProviderCachePurge;
use infiltrator_domain::rules::RuleProviders;
use infiltrator_domain::rules::provider_store::{
    RuleProviderDeclaration, parse_rule_provider_declarations,
};

impl AppState {
    /// DUAL-11-06/07: the single entry point for every provider maintenance
    /// message. Unmatched messages continue down the core dispatch chain.
    pub(crate) fn update_rule_provider(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::UnpackRuleProvider(name) | Message::UnpackRuleProviderToCustom(name) => {
                self.start_rule_provider_unpack(name)
            }
            Message::PurgeRuleProviderCache => self.start_rule_provider_cache_purge(),
            Message::InspectRuleProviderDiff(None) => {
                self.editor.inspecting_rule_provider_diff = None;
                self.editor.is_loading_rule_provider_diff = false;
                Task::none()
            }
            Message::InspectRuleProviderDiff(Some(name)) => self.start_rule_provider_diff(name),
            Message::RuleProviderDiffLoaded(result) => {
                self.editor.is_loading_rule_provider_diff = false;
                match result {
                    Ok(diff) => {
                        self.editor.inspecting_rule_provider_diff = Some(diff);
                        Task::none()
                    }
                    Err(error) => {
                        self.editor.inspecting_rule_provider_diff = None;
                        self.set_provider_status(error.to_string(), ToastStatus::Error)
                    }
                }
            }
            Message::RuleProviderUnpacked(result) => self.finish_rule_provider_unpack(result),
            Message::RuleProviderCachePurged(result) => {
                self.finish_rule_provider_cache_purge(result)
            }
            other => self.update_core_json_editors(other),
        }
    }

    /// Report an honest provider-maintenance outcome on the card and as a
    /// toast. Nothing is reported as done that the host did not do.
    fn set_provider_status(&mut self, message: String, status: ToastStatus) -> Task<Message> {
        self.editor.provider_unpack.status_message = Some(message.clone());
        Task::done(Message::ShowToast(message, status))
    }

    fn start_rule_provider_unpack(&mut self, provider_name: String) -> Task<Message> {
        self.editor.provider_unpack.is_unpacking = true;
        let name = provider_name.trim().to_owned();
        if name.is_empty() {
            self.editor.provider_unpack.is_unpacking = false;
            return self
                .set_provider_status("Rule provider name is empty".to_owned(), ToastStatus::Error);
        }
        let Some(declaration) = self.declared_rule_provider(&name) else {
            self.editor.provider_unpack.is_unpacking = false;
            return self.set_provider_status(
                format!("Provider {name} is not declared in the loaded profile"),
                ToastStatus::Error,
            );
        };
        let cache = self.runtime.rule_provider_cache_port.clone();
        let gateway = self.runtime.runtime.clone();
        let target = infiltrator_domain::rules::edit::DEFAULT_RULE_TARGET.to_owned();
        Task::perform(
            async move {
                let application = RuleProviderApplication::new(cache);
                let view: Option<&dyn infiltrator_ports::runtime_gateway::RuntimeGateway> =
                    match gateway.as_ref() {
                        Some(runtime) => Some(runtime.as_ref()
                            as &dyn infiltrator_ports::runtime_gateway::RuntimeGateway),
                        None => None,
                    };
                application
                    .deconstruct(&declaration, &target, view)
                    .await
                    .map_err(|failure| InfiltratorError::Config(failure.message))
            },
            Message::RuleProviderUnpacked,
        )
    }

    fn finish_rule_provider_unpack(
        &mut self,
        result: Result<
            infiltrator_application::rule_provider_application::ProviderUnpackPlan,
            InfiltratorError,
        >,
    ) -> Task<Message> {
        self.editor.provider_unpack.is_unpacking = false;
        match result {
            Ok(plan) => {
                let imported = plan.imported();
                let origin = plan.origin.as_str();
                // Same shared reduction as the application path: unpacked
                // provider rules take the highest priority slot.
                infiltrator_domain::rules::edit::prepend_rules(
                    &mut self.editor.rules,
                    plan.entries.iter().cloned(),
                );
                self.editor.rules_dirty = true;
                self.rebuild_rules_render_cache();
                self.apply_rules_filter();
                self.editor.provider_unpack.unpacked_rules_count = self
                    .editor
                    .provider_unpack
                    .unpacked_rules_count
                    .saturating_add(imported);
                self.set_provider_status(
                    format!(
                        "{} · {imported} rules · origin {origin} · skipped {}",
                        plan.provider_name, plan.skipped
                    ),
                    ToastStatus::Success,
                )
            }
            Err(error) => self.set_provider_status(error.to_string(), ToastStatus::Error),
        }
    }

    fn start_rule_provider_cache_purge(&mut self) -> Task<Message> {
        let Some(cache) = self.runtime.rule_provider_cache_port.clone() else {
            self.editor.provider_unpack.is_purging_cache = false;
            return self.set_provider_status(
                "This host does not expose a rule-provider cache location".to_owned(),
                ToastStatus::Error,
            );
        };
        self.editor.provider_unpack.is_purging_cache = true;
        Task::perform(
            async move {
                cache
                    .purge()
                    .await
                    .map_err(|error| InfiltratorError::Config(error.to_string()))
            },
            Message::RuleProviderCachePurged,
        )
    }

    fn finish_rule_provider_cache_purge(
        &mut self,
        result: Result<ProviderCachePurge, InfiltratorError>,
    ) -> Task<Message> {
        self.editor.provider_unpack.is_purging_cache = false;
        match result {
            Ok(purge) => {
                let status = if purge.is_noop() {
                    ToastStatus::Warning
                } else {
                    ToastStatus::Success
                };
                self.set_provider_status(
                    format!(
                        "{} · {} files removed · {} bytes freed",
                        purge.directory.as_deref().unwrap_or("no cache directory"),
                        purge.files_removed,
                        purge.bytes_freed
                    ),
                    status,
                )
            }
            Err(error) => self.set_provider_status(error.to_string(), ToastStatus::Error),
        }
    }

    /// DUAL-11-04/06: compare the provider's real local file with the rule list
    /// the controller actually publishes. Neither side is ever fabricated: a
    /// provider with no local file and no published payload answers a typed
    /// failure instead of a sample diff.
    fn start_rule_provider_diff(&mut self, provider_name: String) -> Task<Message> {
        let Some(declaration) = self.declared_rule_provider(&provider_name) else {
            self.editor.is_loading_rule_provider_diff = false;
            return self.set_provider_status(
                format!("Provider {provider_name} is not declared in the loaded profile"),
                ToastStatus::Error,
            );
        };
        self.editor.is_loading_rule_provider_diff = true;
        let cache = self.runtime.rule_provider_cache_port.clone();
        let gateway = self.runtime.runtime.clone();
        Task::perform(
            async move {
                let local = match cache.as_ref() {
                    Some(cache) => match cache.read_provider(&declaration).await {
                        Ok(Some(entry)) => {
                            infiltrator_domain::rules::provider_store::deconstruct_provider_payload(
                                &entry.bytes,
                                &declaration,
                                infiltrator_domain::rules::edit::DEFAULT_RULE_TARGET,
                            )
                            .map(|deconstructed| {
                                deconstructed
                                    .entries
                                    .into_iter()
                                    .map(|entry| entry.rule)
                                    .collect::<Vec<String>>()
                            })
                            .map_err(|error| InfiltratorError::Config(error.to_string()))?
                        }
                        Ok(None) => Vec::new(),
                        Err(error) => return Err(InfiltratorError::Config(error.to_string())),
                    },
                    None => Vec::new(),
                };
                let remote = match gateway.as_ref() {
                    Some(runtime) => runtime
                        .rule_provider_payload(&declaration.name)
                        .await
                        .map_err(|error| InfiltratorError::Internal(error.to_string()))?,
                    None => None,
                };
                let Some(remote) = remote else {
                    return Err(InfiltratorError::Config(format!(
                        "the controller publishes no payload for {}; mihomo exposes rule-provider payloads only for inline providers",
                        declaration.name
                    )));
                };
                Ok(infiltrator_domain::rules::diff_rule_provider_contents(
                    &declaration.name,
                    &local,
                    &remote,
                ))
            },
            Message::RuleProviderDiffLoaded,
        )
    }

    /// The active profile's declared providers, parsed from the same JSON the
    /// rule-providers editor renders. An unloaded/empty cache honestly yields
    /// no declarations, so the caller reports "not declared" instead of
    /// guessing a provider.
    fn declared_rule_provider(&self, name: &str) -> Option<RuleProviderDeclaration> {
        self.declared_rule_providers()
            .into_iter()
            .find(|declaration| declaration.name == name)
    }

    fn declared_rule_providers(&self) -> Vec<RuleProviderDeclaration> {
        let cached = self.editor.rule_providers_json_cache.trim();
        if cached.is_empty() || cached == "{}" {
            return Vec::new();
        }
        let Ok(providers) = serde_json::from_str::<RuleProviders>(cached) else {
            return Vec::new();
        };
        parse_rule_provider_declarations(&providers)
    }
}

#[cfg(test)]
#[path = "../../../tests/gui/rules_provider_tests.rs"]
mod rules_provider_tests;
