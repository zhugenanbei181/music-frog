//! Native TEA adapter for the application-owned lifecycle command boundary.
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use iced::Task;
use infiltrator_contract::core_control::CoreControlAction;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure, InfiltratorError};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};

impl AppState {
    pub(super) fn submit_shared_core_control(
        &mut self,
        action: CoreControlAction,
    ) -> Task<Message> {
        if self.core_control_projection().action != Some(action) {
            return Task::none();
        }
        let Some(application) = self.command_ingress() else {
            return Task::none();
        };
        self.cancel_all_tasks();
        self.runtime.lifecycle_pending = Some(action);
        self.runtime.lifecycle_failure = None;
        self.shell.error_msg = None;
        let token = self.runtime.lifecycle_token;
        let requested_scope = application.observed_scope();
        Task::perform(
            async move {
                let outcome = application.execute_outcome(action.intent()).await;
                let result = match outcome.result.into_unit() {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                };
                (
                    result,
                    application.snapshot(),
                    requested_scope,
                    outcome.scope,
                )
            },
            move |(result, snapshot, requested_scope, terminal_scope)| {
                Message::CoreControlFinished {
                    action,
                    token,
                    result,
                    requested_scope,
                    terminal_scope,
                    snapshot: Box::new(snapshot),
                }
            },
        )
    }

    pub(super) fn finish_shared_core_control(
        &mut self,
        action: CoreControlAction,
        token: u64,
        result: Result<(), Failure>,
        requested_scope: Option<CoreScope>,
        terminal_scope: Option<CoreScope>,
        snapshot: CoreSnapshot,
    ) -> Task<Message> {
        if token != self.runtime.lifecycle_token || self.runtime.lifecycle_pending != Some(action) {
            return Task::none();
        }
        self.runtime.lifecycle_pending = None;
        let confirmed = match action {
            CoreControlAction::Start => {
                matches!(
                    snapshot.lifecycle,
                    CoreLifecycle::Ready | CoreLifecycle::Running
                )
            }
            CoreControlAction::Stop => snapshot.lifecycle == CoreLifecycle::Stopped,
        };
        let result = result.and_then(|()| {
            if confirmed {
                Ok(())
            } else {
                Err(snapshot.failure.clone().unwrap_or_else(|| {
                    Failure::new(
                        ErrorCode::NotReady,
                        "lifecycle command completed without the requested process state",
                        true,
                    )
                }))
            }
        });
        self.runtime.core_lifecycle = snapshot.lifecycle_snapshot();
        self.runtime.status = RuntimeStatus::from_core_snapshot(&snapshot);
        if let (Some(requested), Some(terminal)) = (requested_scope, terminal_scope)
            && snapshot.scope().as_ref() == Some(&terminal)
            && terminal.identity.engine == requested.identity.engine
            && terminal.identity.instance == requested.identity.instance
            && terminal.generation >= requested.generation
        {
            self.rebind_product_commands_to_scope(Some(terminal));
        }
        if let Err(failure) = result {
            self.runtime.lifecycle_failure = Some(failure.clone());
            self.set_error(InfiltratorError::Backend(failure.message));
            return Task::none();
        }
        self.runtime.lifecycle_failure = None;
        if action == CoreControlAction::Stop {
            // Clear transient views while keeping the composed host and reader alive.
            return self.update_core(Message::CoreStopped);
        }
        self.refresh_tray();
        Task::batch([
            Task::done(Message::FetchRuntimeConfig),
            Task::done(Message::RefreshOutboundInventory),
            Task::done(Message::RefreshRuntimeNow),
        ])
    }
}
