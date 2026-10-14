//! Shared labels for system controls; failures retain their reported reason.
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleSnapshot, SystemToggleState};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

/// Fold only verified host observations into the compact toggle read model.
pub fn from_surface(snapshot: &SurfaceSnapshot) -> SystemToggleSnapshot {
    let system_proxy = match &snapshot.system_proxy.status {
        SystemProxyStatus::Unknown => SystemToggleState::Unknown,
        SystemProxyStatus::Disabled => SystemToggleState::Disabled,
        SystemProxyStatus::Enabled => SystemToggleState::Enabled,
        SystemProxyStatus::Unsupported { failure } => SystemToggleState::Unsupported {
            failure: failure.clone(),
        },
        SystemProxyStatus::Failed { failure } => SystemToggleState::Failed {
            failure: failure.clone(),
        },
    };
    let tun = match &snapshot.runtime_control.status {
        RuntimeControlStatus::Ready => snapshot
            .runtime_control
            .tun_enabled
            .map_or(SystemToggleState::Unknown, SystemToggleState::from_enabled),
        RuntimeControlStatus::Unsupported { failure } => SystemToggleState::Unsupported {
            failure: failure.clone(),
        },
        RuntimeControlStatus::Failed { failure } => SystemToggleState::Failed {
            failure: failure.clone(),
        },
        RuntimeControlStatus::Unobserved => SystemToggleState::Unknown,
    };
    SystemToggleSnapshot {
        system_proxy,
        tun,
        revision: snapshot.revision,
    }
}

/// Build an intent only from an actionable owner readback state.
pub fn intent(
    snapshot: &SystemToggleSnapshot,
    toggle: SystemToggle,
    desired: bool,
) -> Result<CommandIntent, Failure> {
    match snapshot.state(toggle) {
        SystemToggleState::Enabled | SystemToggleState::Disabled => {}
        SystemToggleState::Pending { .. } => {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "system toggle action is already in flight",
                false,
            ));
        }
        SystemToggleState::Unknown => {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "system toggle has no authoritative host readback",
                true,
            ));
        }
        SystemToggleState::Unsupported { failure } | SystemToggleState::Failed { failure } => {
            return Err(failure.clone());
        }
    }
    Ok(match toggle {
        SystemToggle::SystemProxy => CommandIntent::SetSystemProxy { enabled: desired },
        SystemToggle::Tun => CommandIntent::ToggleTun { enabled: desired },
    })
}

pub fn status_label(state: &SystemToggleState, locale: &str) -> String {
    let key = match state {
        SystemToggleState::Enabled => "overview_toggle_enabled",
        SystemToggleState::Disabled => "overview_toggle_disabled",
        SystemToggleState::Pending { .. } => "overview_toggle_pending",
        SystemToggleState::Unknown => "overview_toggle_unknown",
        SystemToggleState::Unsupported { failure } | SystemToggleState::Failed { failure } => {
            let key = if matches!(state, SystemToggleState::Unsupported { .. }) {
                "overview_toggle_unavailable"
            } else {
                "overview_toggle_failed"
            };
            return localize(
                locale,
                "network_status_detail",
                &[
                    ("state", Lang(locale).tr(key).into_owned()),
                    ("reason", failure.message.clone()),
                ],
            );
        }
    };
    Lang(locale).tr(key).into_owned()
}

pub fn action_label(state: &SystemToggleState, locale: &str) -> String {
    Lang(locale)
        .tr(match state {
            SystemToggleState::Enabled => "overview_toggle_disable",
            SystemToggleState::Disabled => "overview_toggle_enable",
            SystemToggleState::Pending { .. } => "overview_toggle_pending",
            SystemToggleState::Unknown
            | SystemToggleState::Unsupported { .. }
            | SystemToggleState::Failed { .. } => "overview_toggle_unavailable",
        })
        .into_owned()
}

/// The compact renderer uses short translated states and neutral pending/unknown symbols.
pub fn compact_label(state: &SystemToggleState, locale: &str) -> String {
    match state {
        SystemToggleState::Enabled => Lang(locale).tr("system_toggle_on_short").into_owned(),
        SystemToggleState::Disabled => Lang(locale).tr("system_toggle_off_short").into_owned(),
        SystemToggleState::Pending { .. } => "…".into(),
        SystemToggleState::Unknown
        | SystemToggleState::Unsupported { .. }
        | SystemToggleState::Failed { .. } => "—".into(),
    }
}

pub fn compact_status_line(
    proxy: &SystemToggleState,
    tun: &SystemToggleState,
    locale: &str,
) -> String {
    localize(
        locale,
        "system_toggle_compact_line",
        &[
            ("proxy", compact_label(proxy, locale)),
            ("tun", compact_label(tun, locale)),
        ],
    )
}

#[cfg(test)]
#[path = "system_toggle_projection_tests.rs"]
mod tests;
