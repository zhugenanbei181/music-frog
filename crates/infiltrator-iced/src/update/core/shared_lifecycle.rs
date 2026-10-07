//! Native TEA adapter for the application-owned lifecycle command boundary.
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use iced::Task;
use infiltrator_contract::core_control::CoreControlAction;
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
        let Some(application) = self.commands.clone() else {
            return Task::none();
        };
        self.cancel_all_tasks();
        self.runtime.lifecycle_pending = Some(action);
        self.runtime.lifecycle_failure = None;
        self.shell.error_msg = None;
        let token = self.runtime.lifecycle_token;
        Task::perform(
            async move {
                let result = match (application.execute(action.intent()).await).into_unit() {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                };
                (result, application.snapshot())
            },
            move |(result, snapshot)| Message::CoreControlFinished {
                action,
                token,
                result,
                snapshot: Box::new(snapshot),
            },
        )
    }

    pub(super) fn finish_shared_core_control(
        &mut self,
        action: CoreControlAction,
        token: u64,
        result: Result<(), Failure>,
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
        if let Err(failure) = result {
            self.runtime.lifecycle_failure = Some(failure.clone());
            self.set_error(InfiltratorError::Mihomo(failure.message));
            return Task::none();
        }
        self.runtime.lifecycle_failure = None;
        if action == CoreControlAction::Stop {
            // Clear transient views while keeping the composed host and reader alive.
            return self.update_core(Message::ProxyStopped);
        }
        self.refresh_tray();
        Task::batch([
            Task::done(Message::FetchRuntimeConfig),
            Task::done(Message::LoadProxies),
            Task::done(Message::RefreshRuntimeNow),
        ])
    }
}
