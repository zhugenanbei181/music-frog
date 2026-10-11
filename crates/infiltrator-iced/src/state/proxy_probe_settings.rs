//! Iced adapter for editing and persisting Core-owned runtime probe settings.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::runtime_settings::{RuntimeProbeSettings, RuntimeProbeSettingsState};

impl AppState {
    pub(crate) fn applied_runtime_probe_settings(&self) -> Result<RuntimeProbeSettings, Failure> {
        self.runtime
            .runtime_probe_settings_editor
            .applied()
            .cloned()
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "runtime probe settings are unavailable",
                    true,
                )
            })
    }

    pub(crate) fn observe_probe_settings(&mut self, state: &RuntimeProbeSettingsState) {
        self.runtime.runtime_probe_settings_editor.observe(state);
        self.sync_probe_draft_fields();
    }

    pub(crate) fn sync_probe_draft_fields(&mut self) {
        match self.runtime.runtime_probe_settings_editor.draft() {
            Some(draft) => {
                self.runtime.runtime_probe_url_draft = draft.url.clone();
                self.runtime.runtime_probe_timeout_draft = draft.timeout_ms.clone();
            }
            None => {
                self.runtime.runtime_probe_url_draft.clear();
                self.runtime.runtime_probe_timeout_draft.clear();
            }
        }
    }

    pub(crate) fn apply_runtime_probe_settings(&mut self) -> Task<Message> {
        let Some(application) = self.command_ingress() else {
            self.runtime
                .runtime_probe_settings_editor
                .record_failure(Failure::new(
                    ErrorCode::NotReady,
                    "runtime probe settings command service is not composed",
                    true,
                ));
            return Task::none();
        };
        let pending = match self.runtime.runtime_probe_settings_editor.begin() {
            Ok(pending) => pending,
            Err(failure) => {
                self.runtime
                    .runtime_probe_settings_editor
                    .record_failure(failure);
                return Task::none();
            }
        };
        Task::perform(
            async move {
                match application
                    .execute(CommandIntent::SetRuntimeProbeSettings {
                        settings: pending.settings,
                    })
                    .await
                    .into_unit()
                {
                    Ok(()) => Ok(()),
                    Err(failure) => Err(failure),
                }
            },
            move |result| Message::RuntimeProbeSettingsApplied {
                token: pending.token,
                result,
            },
        )
    }
}
