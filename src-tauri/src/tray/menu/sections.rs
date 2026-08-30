//! Static tray menu sections without dynamic maps: About, Advanced settings
//! launchers, and WebDAV sync & backup.

use tauri::{
    AppHandle, Wry,
    menu::{MenuItem, Submenu},
};

use crate::{
    app_state::AppState,
    locales::{Lang, Localizer},
};

pub(super) async fn build_about_submenu(
    app: &AppHandle,
    state: &AppState,
    lang: &Lang<'_>,
) -> tauri::Result<Submenu<Wry>> {
    let app_version = format!(
        "MusicFrog-Despicable-Infiltrator v{}",
        env!("CARGO_PKG_VERSION")
    );
    let sdk_version = "mihomo-sdk (workspace)";

    let core_version = if let Ok(runtime) = state.runtime().await {
        match runtime.client().get_version().await {
            Ok(v) => format!("{} {}", lang.tr("core_service"), v.version),
            Err(_) => format!("{} ({})", lang.tr("core_service"), lang.tr("unknown")),
        }
    } else {
        format!("{} ({})", lang.tr("core_service"), lang.tr("not_started"))
    };

    let app_item = MenuItem::with_id(app, "about-app", &app_version, false, None::<&str>)?;
    let sdk_item = MenuItem::with_id(app, "about-sdk", sdk_version, false, None::<&str>)?;
    let core_item = MenuItem::with_id(app, "about-core", &core_version, false, None::<&str>)?;

    Submenu::with_items(
        app,
        lang.tr("about"),
        true,
        &[&app_item, &sdk_item, &core_item],
    )
}

pub(super) fn build_advanced_submenu(
    app: &AppHandle,
    lang: &Lang<'_>,
    admin_ready: bool,
    core_ready: bool,
) -> tauri::Result<Submenu<Wry>> {
    let enabled = admin_ready && core_ready;
    let dns_item = MenuItem::with_id(
        app,
        "dns-open-settings",
        lang.tr("dns_settings"),
        enabled,
        None::<&str>,
    )?;
    let fake_ip_item = MenuItem::with_id(
        app,
        "fake-ip-open-settings",
        lang.tr("fake_ip_settings"),
        enabled,
        None::<&str>,
    )?;
    let fake_ip_flush_item = MenuItem::with_id(
        app,
        "fake-ip-flush",
        lang.tr("fake_ip_flush"),
        enabled,
        None::<&str>,
    )?;
    let rules_item = MenuItem::with_id(
        app,
        "rules-open-settings",
        lang.tr("rules_settings"),
        enabled,
        None::<&str>,
    )?;
    let tun_item = MenuItem::with_id(
        app,
        "tun-open-settings",
        lang.tr("tun_settings"),
        enabled,
        None::<&str>,
    )?;

    Submenu::with_items(
        app,
        lang.tr("advanced_settings"),
        true,
        &[
            &dns_item,
            &fake_ip_item,
            &fake_ip_flush_item,
            &rules_item,
            &tun_item,
        ],
    )
}

pub(super) async fn build_sync_submenu(
    app: &AppHandle,
    state: &AppState,
    lang: &Lang<'_>,
) -> tauri::Result<Submenu<Wry>> {
    let settings = state.get_app_settings().await;
    let enabled = settings.webdav.enabled;

    let status_label = if enabled {
        format!("{}: {}", lang.tr("webdav_sync"), lang.tr("enabled"))
    } else {
        format!("{}: {}", lang.tr("webdav_sync"), lang.tr("disabled"))
    };

    let status_item = MenuItem::with_id(app, "sync-status", &status_label, false, None::<&str>)?;
    let sync_now_item = MenuItem::with_id(
        app,
        "webdav-sync-now",
        lang.tr("sync_now"),
        enabled,
        None::<&str>,
    )?;
    let sync_settings_item = MenuItem::with_id(
        app,
        "webdav-sync-settings",
        lang.tr("sync_settings"),
        true,
        None::<&str>,
    )?;

    Submenu::with_items(
        app,
        lang.tr("sync_and_backup"),
        true,
        &[&status_item, &sync_now_item, &sync_settings_item],
    )
}
