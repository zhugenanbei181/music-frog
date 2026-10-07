//! Replay shared status text when either facts or locale change, preserving the native tree.
use super::settings_core::{SettingsLine, SettingsLineKind, SettingsProjection};
use super::settings_pac::PacStatusLine;
use super::{CoreRollbackButtonLabel, LastSettingsProjection, ServiceModeButtonLabel};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Query, Res};
use bevy::ui::widget::Text;
use infiltrator_application::host_network_projection::pac;
use infiltrator_application::network_status_projection::system_proxy_status;
use infiltrator_application::settings_status_projection::{
    format_controller_auth, format_core_resources, format_core_versions, format_integrity,
    format_ipv6, format_lan_auth, format_mtu, format_offline_startup, format_port_conflicts,
    format_rollback_target, format_runtime_status, format_service_mode, optional_copy,
};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::service_mode::ServiceModeState;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn line(kind: SettingsLineKind, projection: &SettingsProjection, code: &str) -> Option<String> {
    Some(match kind {
        SettingsLineKind::Summary => Lang(code).tr("settings_summary").into_owned(),
        SettingsLineKind::RuntimeStatus => format_runtime_status(&projection.runtime_status, code),
        SettingsLineKind::OfflineStartup => {
            format_offline_startup(&projection.offline_startup, code)
        }
        SettingsLineKind::MixedPort => localize(
            code,
            "settings_port_value",
            &[("port", optional_copy(projection.mixed_port.as_ref(), code))],
        ),
        SettingsLineKind::LanBindAddress => {
            optional_copy(projection.lan_bind_address.as_ref(), code)
        }
        SettingsLineKind::LanSecurity => format_lan_auth(projection.lan_security.as_ref(), code),
        SettingsLineKind::Ipv6Routing => format_ipv6(projection.ipv6_routing.as_ref(), code),
        SettingsLineKind::TunStack => optional_copy(projection.tun_stack.as_ref(), code),
        SettingsLineKind::ControllerPort => projection
            .controller_port
            .map(|port| format!("127.0.0.1:{port}"))
            .unwrap_or_else(|| Lang(code).tr("shell_readout_unknown").into_owned()),
        SettingsLineKind::LogLevel => localize(
            code,
            "settings_log_level_value",
            &[("level", optional_copy(projection.log_level.as_ref(), code))],
        ),
        SettingsLineKind::CoreChannel => localize(
            code,
            "settings_channel_value",
            &[("channel", projection.core_channel.clone())],
        ),
        SettingsLineKind::CoreVersions => format_core_versions(&projection.core_versions, code),
        SettingsLineKind::CoreIntegrity => format_integrity(&projection.core_integrity, code),
        SettingsLineKind::CoreRollback => format_rollback_target(&projection.core_versions, code),
        SettingsLineKind::ControllerAuth => {
            format_controller_auth(&projection.controller_auth, code)
        }
        SettingsLineKind::SystemProxy => system_proxy_status(
            &projection.system_proxy_snapshot,
            &projection.system_proxy_recovery,
            code,
        ),
        SettingsLineKind::ServiceMode => format_service_mode(&projection.service_mode, code),
        SettingsLineKind::PortConflicts => format_port_conflicts(&projection.port_conflicts, code),
        SettingsLineKind::CoreResources => format_core_resources(&projection.core_resources, code),
        SettingsLineKind::Mtu => format_mtu(&projection.mtu, code),
    })
}
pub fn rollback_label(projection: &SettingsProjection, code: &str) -> String {
    Lang(code)
        .tr(if projection.core_versions.rollback.target.is_some() {
            "core_rollback_action"
        } else {
            "common_unavailable"
        })
        .into_owned()
}
pub fn service_label(projection: &SettingsProjection, code: &str) -> String {
    Lang(code)
        .tr(
            if projection.service_mode.state == ServiceModeState::Ready {
                "core_service_ready"
            } else {
                "core_prepare_service_action"
            },
        )
        .into_owned()
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct StatusText {
    text: &'static mut Text,
    line: Option<&'static SettingsLine>,
    rollback: Option<&'static CoreRollbackButtonLabel>,
    service: Option<&'static ServiceModeButtonLabel>,
    pac: Option<&'static PacStatusLine>,
}
pub fn replay_locale(
    locale: Res<UiLocale>,
    last: Res<LastSettingsProjection>,
    mut texts: Query<StatusText>,
) {
    if !locale.is_changed() {
        return;
    }
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    for mut parts in &mut texts {
        let value = if let Some(marker) = parts.line {
            line(marker.0, projection, locale.code())
        } else if parts.rollback.is_some() {
            Some(rollback_label(projection, locale.code()))
        } else if parts.pac.is_some() {
            Some(pac(&projection.pac, locale.code()))
        } else if parts.service.is_some() {
            Some(service_label(projection, locale.code()))
        } else {
            None
        };
        if let Some(value) = value
            && parts.text.0 != value
        {
            parts.text.0 = value
        }
    }
}
