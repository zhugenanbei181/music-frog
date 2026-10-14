//! Apply revision-fenced outbound presentation preferences through the shared owner.

use crate::state::AppState;
use crate::state::runtime::ProxyGroupOrderDialogState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::preferences::{
    OutboundUiPreferenceIntent, OutboundUiPreferenceRequest, OutboundUiPreferencesSnapshot,
};
use infiltrator_contract::surface_snapshot::{PageData, PageStatus};
use infiltrator_presentation::proxy_group_order_editor::GroupOrderFinish;
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

#[derive(Clone, Debug, Default)]
pub enum ProxyPreferencePhase {
    #[default]
    Idle,
    Applying(ApplyingOutboundPreference),
    Failed(Failure),
}

#[derive(Default)]
pub struct ProxyPreferenceState {
    queue: VecDeque<QueuedOutboundPreference>,
    pub phase: ProxyPreferencePhase,
    next_token: u64,
}

impl ProxyPreferenceState {
    pub fn failure(&self) -> Option<&Failure> {
        match &self.phase {
            ProxyPreferencePhase::Failed(failure) => Some(failure),
            ProxyPreferencePhase::Idle | ProxyPreferencePhase::Applying(_) => None,
        }
    }
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
        match &self.runtime.proxy_preferences.phase {
            ProxyPreferencePhase::Applying(_) => return Task::none(),
            ProxyPreferencePhase::Idle | ProxyPreferencePhase::Failed(_) => {}
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
        self.runtime.proxy_preferences.next_token =
            self.runtime.proxy_preferences.next_token.wrapping_add(1);
        let token = self.runtime.proxy_preferences.next_token;
        self.runtime.proxy_preferences.phase =
            ProxyPreferencePhase::Applying(ApplyingOutboundPreference {
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
        let active = match &self.runtime.proxy_preferences.phase {
            ProxyPreferencePhase::Applying(active) if active.token == token => active.clone(),
            ProxyPreferencePhase::Idle
            | ProxyPreferencePhase::Applying(_)
            | ProxyPreferencePhase::Failed(_) => return Task::none(),
        };
        if active.scope != scope || active.request != request {
            return Task::none();
        }
        self.runtime.proxy_preferences.phase = ProxyPreferencePhase::Idle;
        let group_order_token = active.group_order_token;
        if self
            .command_ingress()
            .and_then(|ingress| ingress.observed_scope())
            .as_ref()
            != Some(&scope)
            || terminal_scope.as_ref() != Some(&scope)
        {
            self.runtime.proxy_search.preference_cancelled(token);
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
                self.runtime.proxy_preferences.phase = ProxyPreferencePhase::Idle;
                self.apply_outbound_ui_preferences_observation(PageData::ready(snapshot));
                Some(Ok(()))
            }
            Ok(_) => {
                let failure = Failure::new(
                    ErrorCode::InvalidState,
                    "outbound preference readback did not confirm the request",
                    false,
                );
                self.fail_outbound_ui_preference(failure.clone());
                if matches!(
                    request.intent(),
                    OutboundUiPreferenceIntent::SetSearchQuery(_)
                ) {
                    self.runtime.proxy_search.preference_cancelled(token);
                }
                Some(Err(failure))
            }
            Err(failure) => {
                self.fail_outbound_ui_preference(failure.clone());
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

    fn fail_outbound_ui_preference(&mut self, failure: Failure) {
        self.runtime.proxy_preferences.phase = ProxyPreferencePhase::Failed(failure);
    }
}
