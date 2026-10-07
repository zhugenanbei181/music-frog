//! TEA adapter for the shared parameter editor and durable command acknowledgment.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::proxy_probe_options_projection::validate_options;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxy_probe_options::{ProxyProbeOptions, ProxyProbeSettingsSnapshot};

impl AppState {
    pub(crate) fn applied_probe_options(&self) -> Result<ProxyProbeOptions, Failure> {
        let options = self
            .runtime
            .probe_options_editor
            .applied
            .clone()
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "saved probe parameters are unavailable",
                    true,
                )
            })?;
        validate_options(options)
    }
    pub(crate) fn probe_proxy_by_name(&mut self, name: String) -> Task<Message> {
        let Some(application) = self.commands.clone() else {
            return Task::none();
        };
        self.runtime.runtime_testing_delay_proxy = name.clone();
        let node = name.clone();
        let options = self.applied_probe_options().ok();
        let (url, timeout_ms) = match options {
            Some(opt) => (Some(opt.test_url), Some(opt.timeout_ms)),
            None => (None, None),
        };
        Task::perform(
            async move {
                match (application
                    .execute(CommandIntent::TestNodeDelay {
                        node,
                        url,
                        timeout_ms,
                    })
                    .await)
                    .into_unit()
                {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::ProxyDelayCompleted {
                name: name.clone(),
                result,
            },
        )
    }

    pub(crate) fn observe_probe_settings(&mut self, snapshot: &ProxyProbeSettingsSnapshot) {
        self.runtime.probe_options_editor.observe(snapshot);
        self.sync_probe_draft_fields();
    }
    pub(crate) fn sync_probe_draft_fields(&mut self) {
        self.runtime.runtime_delay_test_url =
            self.runtime.probe_options_editor.draft.test_url.clone();
        self.runtime.runtime_delay_timeout_ms =
            self.runtime.probe_options_editor.draft.timeout_ms.clone();
    }
    pub(crate) fn apply_probe_options(&mut self) -> Task<Message> {
        let Some(application) = self.commands.clone() else {
            self.runtime.probe_options_editor.failure = Some(Failure::new(
                ErrorCode::NotReady,
                "probe settings command service is not composed",
                true,
            ));
            return Task::none();
        };
        let pending = match self.runtime.probe_options_editor.begin() {
            Ok(pending) => pending,
            Err(failure) => {
                self.runtime.probe_options_editor.failure = Some(failure);
                return Task::none();
            }
        };
        Task::perform(
            async move {
                match (application
                    .execute(CommandIntent::SetProxyProbeOptions {
                        options: pending.options,
                    })
                    .await)
                    .into_unit()
                {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::ProxyProbeOptionsApplied {
                token: pending.token,
                result,
            },
        )
    }
}
