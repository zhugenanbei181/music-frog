//! Shared labels for system controls; failures retain their reported reason.
use infiltrator_contract::system_toggle::SystemToggleState;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

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
mod tests {
    use super::{compact_label, compact_status_line};
    use infiltrator_contract::error::Failure;
    use infiltrator_contract::system_toggle::SystemToggleState;
    #[test]
    fn compact_status_localizes_actual_states_and_never_fakes_failed_or_unknown_as_off() {
        let enabled = SystemToggleState::Enabled;
        let pending = SystemToggleState::Pending { desired: true };
        assert_eq!(
            compact_status_line(&enabled, &pending, "zh-CN"),
            "系统代理: 开 · TUN: …"
        );
        assert_eq!(
            compact_status_line(&enabled, &pending, "en-US"),
            "Proxy: On · TUN: …"
        );
        assert_eq!(compact_label(&SystemToggleState::Disabled, "en-US"), "Off");
        for state in [
            SystemToggleState::Unknown,
            SystemToggleState::Unsupported {
                failure: Failure::unsupported("host absent"),
            },
            SystemToggleState::Failed {
                failure: Failure::unsupported("read denied"),
            },
        ] {
            assert_eq!(compact_label(&state, "zh-CN"), "—");
            assert_eq!(compact_label(&state, "en-US"), "—");
        }
    }
}
