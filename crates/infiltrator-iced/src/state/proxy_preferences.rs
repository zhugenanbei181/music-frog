//! Apply revision-fenced outbound presentation preferences through the shared owner.

use crate::state::AppState;
use crate::state::runtime::ProxyGroupOrderDialogState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::preferences::{
    OutboundUiPreferenceIntent, OutboundUiPreferenceRequest, OutboundUiPreferencesSnapshot,
};
use lurker_core_contract::page::{PageData, PageStatus};
use lurker_ui_presentation::proxy_group_order_editor::GroupOrderFinish;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
struct QueuedOutboundPreference {
    scope: CoreScope,
    intent: OutboundUiPreferenceIntent,
}

#[derive(Clone, Debug)]
pub struct ApplyingOutboundPreference {
    pub token: u64,
    pub scope: CoreScope,
    pub request: OutboundUiPreferenceRequest,
    group_order_token: Option<u64>,
}

/// The request context of the outbound preference write.
///
/// The phase of the write is the shared page model's
/// (`ProxiesPageModel::preference_write_phase`); this typed value only
/// carries the request context the owner needs to fence the answer.
#[derive(Clone, Debug, Default)]
pub enum OutboundPreferenceWrite {
    /// No preference write was submitted.
    #[default]
    Idle,
    /// A preference write is in flight.
    Writing(ApplyingOutboundPreference),
}

/// The outbound preference writer: the queued intents and the request
/// context of the write in flight.
#[derive(Default)]
pub struct ProxyPreferenceState {
    queue: VecDeque<QueuedOutboundPreference>,
    /// The request context of the write in flight.
    pub write: OutboundPreferenceWrite,
}

impl AppState {
    pub(crate) fn submit_current_outbound_ui_preference(
        &mut self,
        intent: OutboundUiPreferenceIntent,
    ) -> Task<Message> {
        let Some(scope) = self
            .is_product_command_ingress()
            .then(|| self.command_ingress())
            .flatten()
            .and_then(|ingress| ingress.observed_scope())
        else {
            self.fail_outbound_ui_preference(Failure::new(
                ErrorCode::NotReady,
                "outbound preference has no accepted core scope",
                true,
            ));
            return Task::none();
        };
        self.submit_outbound_ui_preference(scope, intent)
    }

    pub(crate) fn submit_outbound_ui_preference(
        &mut self,
        scope: CoreScope,
        intent: OutboundUiPreferenceIntent,
    ) -> Task<Message> {
        self.runtime
            .proxy_preferences
            .queue
            .push_back(QueuedOutboundPreference { scope, intent });
        self.start_next_outbound_ui_preference()
    }

    fn start_next_outbound_ui_preference(&mut self) -> Task<Message> {
        if matches!(
            &self.runtime.proxy_preferences.write,
            OutboundPreferenceWrite::Writing(_)
        ) {
            return Task::none();
        }
        let Some(queued) = self.runtime.proxy_preferences.queue.pop_front() else {
            return Task::none();
        };
        let Some(ingress) = self.command_ingress() else {
            self.fail_outbound_ui_preference(Failure::new(
                ErrorCode::NotReady,
                "outbound preference service is not composed",
                true,
            ));
            self.runtime.proxy_preferences.queue.clear();
            return Task::none();
        };
        if !self.is_product_command_ingress()
            || ingress.observed_scope().as_ref() != Some(&queued.scope)
        {
            return self.start_next_outbound_ui_preference();
        }
        let Some(snapshot) = self
            .runtime
            .outbound_ui_preferences
            .data
            .as_ref()
            .filter(|snapshot| snapshot.scope() == &queued.scope)
            .filter(|_| {
                matches!(
                    self.runtime.outbound_ui_preferences.status,
                    PageStatus::Ready
                )
            })
        else {
            self.fail_outbound_ui_preference(Failure::new(
                ErrorCode::NotReady,
                "outbound UI preferences are not available for the active scope",
                true,
            ));
            return self.start_next_outbound_ui_preference();
        };
        let request =
            match OutboundUiPreferenceRequest::new(snapshot.revision(), queued.intent.clone()) {
                Ok(request) => request,
                Err(error) => {
                    self.fail_outbound_ui_preference(Failure::new(
                        ErrorCode::InvalidInput,
                        error.to_string(),
                        false,
                    ));
                    return self.start_next_outbound_ui_preference();
                }
            };
        let group_order_token = matches!(
            &queued.intent,
            OutboundUiPreferenceIntent::ReorderGroups { .. }
                | OutboundUiPreferenceIntent::ResetGroupOrder { .. }
        )
        .then(|| {
            self.runtime
                .group_order_editor
                .pending()
                .map(|pending| pending.token)
        })
        .flatten();
        let token = self
            .runtime
            .proxies_model
            .preference_write_phase_mut()
            .begin();
        self.runtime.proxy_preferences.write =
            OutboundPreferenceWrite::Writing(ApplyingOutboundPreference {
                token,
                scope: queued.scope.clone(),
                request: request.clone(),
                group_order_token,
            });
        if let OutboundUiPreferenceIntent::SetSearchQuery(query) = &queued.intent {
            self.runtime.proxy_search.mark_pending(token, query.clone());
        }
        let command_request = request.clone();
        Task::perform(
            async move {
                let outcome = ingress
                    .execute_outcome(CommandIntent::ApplyOutboundUiPreference {
                        request: command_request,
                    })
                    .await;
                let terminal_scope = outcome.scope;
                let result = outcome
                    .result
                    .into_output()
                    .and_then(|output| match output {
                        CommandOutput::OutboundUiPreferencesApplied(snapshot) => Ok(snapshot),
                        _ => Err(Failure::new(
                            ErrorCode::InvalidState,
                            "outbound preference command returned another output type",
                            false,
                        )),
                    });
                (terminal_scope, result)
            },
            move |(terminal_scope, result)| Message::OutboundUiPreferenceFinished {
                token,
                scope: queued.scope,
                request,
                terminal_scope,
                result,
            },
        )
    }

    pub(crate) fn finish_outbound_ui_preference(
        &mut self,
        token: u64,
        scope: CoreScope,
        request: OutboundUiPreferenceRequest,
        terminal_scope: Option<CoreScope>,
        result: Result<OutboundUiPreferencesSnapshot, Failure>,
    ) -> Task<Message> {
        let active = match &self.runtime.proxy_preferences.write {
            OutboundPreferenceWrite::Writing(active) if active.token == token => active.clone(),
            OutboundPreferenceWrite::Idle | OutboundPreferenceWrite::Writing(_) => {
                return Task::none();
            }
        };
        if active.scope != scope || active.request != request {
            return Task::none();
        }
        self.runtime.proxy_preferences.write = OutboundPreferenceWrite::Idle;
        let group_order_token = active.group_order_token;
        if self
            .command_ingress()
            .and_then(|ingress| ingress.observed_scope())
            .as_ref()
            != Some(&scope)
            || terminal_scope.as_ref() != Some(&scope)
        {
            self.runtime.proxy_search.preference_cancelled(token);
            self.runtime
                .proxies_model
                .preference_write_phase_mut()
                .reset();
            if let Some(editor_token) = group_order_token {
                self.runtime.group_order_editor.finish(
                    editor_token,
                    Err(Failure::new(
                        ErrorCode::NotReady,
                        "core scope changed while applying outbound preferences",
                        true,
                    )),
                );
            }
            return self.start_next_outbound_ui_preference();
        }
        let group_order_result = match result {
            Ok(snapshot)
                if snapshot.scope() == &scope
                    && request.is_confirmed_by(&snapshot)
                    && self
                        .runtime
                        .outbound_ui_preferences
                        .data
                        .as_ref()
                        .filter(|current| current.scope() == &scope)
                        .is_none_or(|current| current.revision() <= snapshot.revision()) =>
            {
                if matches!(
                    request.intent(),
                    OutboundUiPreferenceIntent::SetSearchQuery(_)
                ) {
                    self.runtime.proxy_search.preference_applied(token);
                }
                self.runtime
                    .proxies_model
                    .preference_write_phase_mut()
                    .succeed(token);
                self.apply_outbound_ui_preferences_observation(PageData::ready(snapshot));
                Some(Ok(()))
            }
            Ok(_) => {
                let failure = Failure::new(
                    ErrorCode::InvalidState,
                    "outbound preference readback did not confirm the request",
                    false,
                );
                self.fail_inflight_outbound_ui_preference(token, failure.clone());
                if matches!(
                    request.intent(),
                    OutboundUiPreferenceIntent::SetSearchQuery(_)
                ) {
                    self.runtime.proxy_search.preference_cancelled(token);
                }
                Some(Err(failure))
            }
            Err(failure) => {
                self.fail_inflight_outbound_ui_preference(token, failure.clone());
                if matches!(
                    request.intent(),
                    OutboundUiPreferenceIntent::SetSearchQuery(_)
                ) {
                    self.runtime
                        .proxy_search
                        .preference_failed(token, failure.clone());
                }
                Some(Err(failure))
            }
        };
        if let (Some(editor_token), Some(result)) = (group_order_token, group_order_result) {
            match self.finish_group_order_preference(editor_token, result) {
                GroupOrderFinish::Applied => {
                    self.runtime.group_order_dialog = ProxyGroupOrderDialogState::Closed;
                }
                GroupOrderFinish::Failed | GroupOrderFinish::Stale => {}
            }
        }
        self.start_next_outbound_ui_preference()
    }

    fn finish_group_order_preference(
        &mut self,
        editor_token: u64,
        result: Result<(), Failure>,
    ) -> GroupOrderFinish {
        self.runtime.group_order_editor.finish(editor_token, result)
    }

    /// Record a submission that never reached the owner.
    ///
    /// The attempt still owns a submission token, so the shared phase can
    /// carry the failure.
    fn fail_outbound_ui_preference(&mut self, failure: Failure) {
        let phase = self.runtime.proxies_model.preference_write_phase_mut();
        let token = phase.begin();
        phase.fail(token, failure);
    }

    /// Record the failure of the write that is in flight.
    fn fail_inflight_outbound_ui_preference(&mut self, token: u64, failure: Failure) {
        self.runtime
            .proxies_model
            .preference_write_phase_mut()
            .fail(token, failure);
    }
}
