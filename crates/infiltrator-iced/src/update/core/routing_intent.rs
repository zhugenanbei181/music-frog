//! Routing transitions retain typed failures and accept only correlated readbacks.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_presentation::routing_intent_actions::RoutingIntentFinishOutcome;
use infiltrator_presentation::routing_intent_projection;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::routing_intent::RoutingIntent;

impl AppState {
    pub(super) fn update_routing_intent(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetRoutingIntent(intent) => self.submit_routing_intent(intent),
            Message::RetryRoutingIntent => {
                let Some(target) = self.runtime.routing_intent_actions.retry_target() else {
                    return Task::none();
                };
                self.submit_routing_intent(target)
            }
            Message::DismissRoutingIntentFailure => {
                self.runtime.routing_intent_actions.dismiss_failure();
                Task::none()
            }
            Message::RoutingIntentFinished { request, result } => {
                if request.generation != self.runtime.runtime_generation {
                    return Task::none();
                }
                let actual = result.as_ref().ok().cloned();
                match self.runtime.routing_intent_actions.finish(request, result) {
                    RoutingIntentFinishOutcome::Applied => {}
                    RoutingIntentFinishOutcome::IgnoredStale => return Task::none(),
                }
                self.runtime.routing_intent_state =
                    self.runtime.routing_intent_actions.render_snapshot();
                self.runtime.routing_intent = self.runtime.routing_intent_state.current.clone();
                if let Some(actual) = actual {
                    if let Some(snapshot) = self.surface.latest() {
                        let snapshot = routing_intent_projection::with_verified_intent(
                            snapshot,
                            actual.clone(),
                        );
                        self.apply_shared_surface_snapshot(snapshot);
                    }
                    return Task::none();
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn submit_routing_intent(&mut self, target: RoutingIntent) -> Task<Message> {
        if self.runtime.pending_runtime_patch.is_some()
            || self
                .runtime
                .routing_intent_actions
                .pending_change()
                .is_some()
        {
            return Task::none();
        }
        let (generation, revision) = self
            .surface
            .latest()
            .map(|snapshot| (snapshot.generation, snapshot.revision))
            .unwrap_or((self.runtime.runtime_generation, 0));
        self.runtime.routing_intent_actions.observe(
            generation,
            revision,
            self.runtime.routing_intent_state.clone(),
        );
        let Some(application) = self.command_ingress() else {
            self.runtime.routing_intent_actions.record_failure(
                target,
                Failure::new(
                    ErrorCode::NotReady,
                    "The application is unavailable for routing intent control",
                    true,
                ),
            );
            return Task::none();
        };
        let request = match self.runtime.routing_intent_actions.begin(target.clone()) {
            Ok(request) => request,
            Err(failure) => {
                self.runtime
                    .routing_intent_actions
                    .record_failure(target, failure);
                return Task::none();
            }
        };
        self.runtime.routing_intent_state = self.runtime.routing_intent_actions.render_snapshot();
        Task::perform(
            async move {
                let result = application
                    .execute(CommandIntent::SetRoutingIntent {
                        intent: target.clone(),
                    })
                    .await;
                routing_intent_receipt(target, result)
            },
            move |result| Message::RoutingIntentFinished { request, result },
        )
    }
}

fn routing_intent_receipt(
    target: RoutingIntent,
    result: CommandResult,
) -> Result<RoutingIntent, Failure> {
    match result.into_output()? {
        CommandOutput::RoutingIntentApplied { observed } if observed == target => Ok(observed),
        CommandOutput::RoutingIntentApplied { observed } => Err(Failure::new(
            ErrorCode::InvalidState,
            format!("Core observed {observed:?} after requesting {target:?}"),
            true,
        )),
        _ => Err(Failure::new(
            ErrorCode::InvalidState,
            "Core returned no routing intent readback",
            true,
        )),
    }
}
