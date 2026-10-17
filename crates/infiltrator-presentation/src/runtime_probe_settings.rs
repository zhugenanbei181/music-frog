//! Validation and projection for the Core-owned latency-probe settings.

use crate::runtime_probe_settings_editor::RuntimeProbeSettingsDraft;
use infiltrator_contract::runtime_settings::{RuntimeProbeSettings, RuntimeProbeSettingsState};
use infiltrator_domain::settings::AppSettings;
use lurker_core_contract::error::{ErrorCode, Failure};

fn invalid(message: &str) -> Failure {
    Failure::new(ErrorCode::InvalidInput, message, false)
}

pub fn parse_draft(draft: &RuntimeProbeSettingsDraft) -> Result<RuntimeProbeSettings, Failure> {
    let timeout_ms =
        draft.timeout_ms.trim().parse::<u32>().map_err(|_| {
            invalid("probe timeout must be a positive whole number of milliseconds")
        })?;
    RuntimeProbeSettings::new(draft.url.clone(), timeout_ms)
        .map_err(|error| invalid(&error.to_string()))
}

pub fn project_settings(
    settings: Option<&Result<AppSettings, Failure>>,
) -> RuntimeProbeSettingsState {
    match settings {
        None => RuntimeProbeSettingsState::Unavailable {
            failure: Failure::unsupported("runtime probe settings store is not configured"),
        },
        Some(Err(failure)) => RuntimeProbeSettingsState::Unavailable {
            failure: failure.clone(),
        },
        Some(Ok(settings)) => RuntimeProbeSettingsState::Available {
            settings: settings.runtime_panel.probe_settings.clone(),
        },
    }
}
