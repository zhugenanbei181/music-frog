//! Mode transitions retain typed failures and accept only correlated controller readbacks.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::proxy_mode_application::ProxyModeApplication;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxy_mode::ProxyModeStatus;

impl AppState {
    pub(super) fn update_proxy_mode(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetProxyMode(mode) => {
                let Some(target) = ProxyMode::from_wire(&mode) else {
                    return Task::none();
                };
                self.submit_proxy_mode(target)
            }
            Message::RetryProxyMode => {
                let Some(target) = self.runtime.mode_actions.retry_target() else {
                    return Task::none();
                };
                self.submit_proxy_mode(target)
            }
            Message::DismissProxyModeFailure => {
                self.runtime.mode_actions.dismiss_failure();
                Task::none()
            }
            Message::ProxyModeFinished { request, result } => {
                if request.generation != self.runtime.runtime_generation {
                    return Task::none();
                }
                let actual = result.as_ref().ok().copied();
                if !self.runtime.mode_actions.finish(request, result) {
                    return Task::none();
                }
                self.runtime.proxy_mode_state = self.runtime.mode_actions.render_snapshot();
                self.runtime.proxy_mode = self
                    .runtime
                    .proxy_mode_state
                    .current
                    .map(|mode| mode.to_wire().to_owned());
                if let Some(actual) = actual {
                    if let Some(snapshot) = self.surface.latest() {
                        let snapshot = ProxyModeApplication::with_verified_mode(snapshot, actual);
                        self.apply_shared_surface_snapshot(snapshot);
                    }
                    self.runtime.runtime_control.mode = Some(actual);
                    return self.update_core(Message::FetchRuntimeConfig);
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn submit_proxy_mode(&mut self, target: ProxyMode) -> Task<Message> {
        if self.runtime.pending_runtime_patch.is_some()
            || self.runtime.mode_actions.pending.is_some()
        {
            return Task::none();
        }
        let (generation, revision) = self
            .surface
            .latest()
            .map(|snapshot| (snapshot.generation, snapshot.revision))
            .unwrap_or((
                self.runtime.runtime_generation,
                self.runtime.mode_read_revision,
            ));
        self.runtime.mode_actions.observe(
            generation,
            revision,
            self.runtime.proxy_mode_state.clone(),
        );
        if self.runtime.mode_actions.observed.status == ProxyModeStatus::Ready
            && self.runtime.mode_actions.observed.current == Some(target)
        {
            return Task::none();
        }
        if let Err(failure) =
            ProxyModeApplication::intent(&self.runtime.mode_actions.observed, target)
        {
            self.runtime.mode_actions.failure = Some(failure);
            self.runtime.mode_actions.requested = Some(target);
            return Task::none();
        }
        let Some(application) = self.commands.clone() else {
            self.runtime.mode_actions.failure = Some(Failure::new(
                ErrorCode::NotReady,
                "The application is unavailable for proxy mode control",
                true,
            ));
            self.runtime.mode_actions.requested = Some(target);
            return Task::none();
        };
        let request = match self.runtime.mode_actions.begin(target) {
            Ok(request) => request,
            Err(failure) => {
                self.runtime.mode_actions.failure = Some(failure);
                self.runtime.mode_actions.requested = Some(target);
                return Task::none();
            }
        };
        self.runtime.proxy_mode_state = self.runtime.mode_actions.render_snapshot();
        Task::perform(
            async move { ProxyModeApplication::change(&application, target).await },
            move |result| Message::ProxyModeFinished { request, result },
        )
    }
}
