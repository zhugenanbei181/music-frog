//! The business panels of the settings workbench page.

use super::Panel;
use crate::surface::business::BusinessId;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;

/// The business panels the settings page carries.
pub(super) fn panels(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    let page = snapshot.pages.settings.data.as_ref();
    let mixed_port = page
        .and_then(|page| page.lan_sharing.as_ref())
        .map(|lan| lan.mixed_port.to_string())
        .unwrap_or_default();
    let controller_port = page
        .and_then(|page| page.controller_port)
        .map(|port| port.to_string())
        .unwrap_or_default();
    let language = page.map(|page| page.language.clone()).unwrap_or_default();
    let lan = page.and_then(|page| page.lan_sharing.clone());
    let lan_enabled = lan.as_ref().map(|lan| lan.enabled).unwrap_or(false);
    let conflicts = snapshot.port_conflicts.conflicts.len();
    let version = snapshot
        .core
        .core_version
        .clone()
        .unwrap_or_else(|| "—".to_owned());
    let privileged = format!("{:?}", snapshot.privileged_network.state);
    let vpn = format!("{:?}", snapshot.vpn);

    let failure = super::page_failure(&snapshot.pages.settings.status);
    let mut panels = vec![
        Panel::new(
            BusinessId::CoreVersionRollbackConfirm,
            catalog.text("settings_core_rollback_title"),
            version,
            vec![
                catalog.text("core_rollback_action").into_owned(),
                catalog.text("core_rollback_none").into_owned(),
            ],
        ),
        Panel::new(
            BusinessId::SettingsLanSecurity,
            catalog.text("lan_security_title"),
            catalog.text("lan_security_desc").into_owned(),
            vec![
                catalog.text("lan_security_enabled").into_owned(),
                format!("{lan_enabled}"),
            ],
        ),
        Panel::new(
            BusinessId::SettingsLanguageChoice,
            catalog.text("settings_language_title"),
            language,
            vec![
                catalog.text("language_system").into_owned(),
                catalog.text("language_retry").into_owned(),
            ],
        ),
        Panel::new(
            BusinessId::SettingsPermissionPanel,
            catalog.text("privileged_network_title"),
            privileged,
            vec![
                catalog.text("privileged_network_run").into_owned(),
                catalog.text("privileged_network_desc").into_owned(),
            ],
        ),
        Panel::new(
            BusinessId::SettingsPortConflictRepair,
            catalog.text("settings_port_conflict_title"),
            format!("{conflicts}"),
            vec![
                catalog
                    .text("settings_port_conflict_probe_action")
                    .into_owned(),
                format!("{mixed_port} · {controller_port}"),
            ],
        ),
        Panel::new(
            BusinessId::SettingsTunPermission,
            catalog.text("settings_tun_permission_hint"),
            vpn,
            vec![
                catalog.text("settings_uac_request").into_owned(),
                catalog.text("settings_status_no_perm").into_owned(),
            ],
        ),
    ];
    let empty = super::page_empty(&snapshot.pages.settings.status);
    for panel in &mut panels {
        panel.failure = failure.clone();
        panel.empty = empty;
    }
    panels
}
