//! TUN mode menu item: construction and live refresh.

use log::warn;
use tauri::{
    AppHandle, Wry,
    menu::CheckMenuItem,
};

use crate::{
    app_state::AppState,
    locales::{Lang, Localizer},
    platform::is_running_as_admin,
};

pub(super) async fn build_tun_menu_item(
    app: &AppHandle,
    state: &AppState,
    lang: &Lang<'_>,
) -> tauri::Result<CheckMenuItem<Wry>> {
    let is_admin = is_running_as_admin();
    let (available, enabled) = match state.refresh_tun_state().await {
        Ok(result) => result,
        Err(err) => {
            warn!("failed to refresh tun state: {err:#}");
            (false, false)
        }
    };
    let label = if !is_admin {
        lang.tr("tun_mode_admin_required")
    } else if !available {
        lang.tr("tun_mode_disabled")
    } else {
        lang.tr("tun_mode")
    };
    CheckMenuItem::with_id(
        app,
        "tun-mode",
        label,
        is_admin && available,
        enabled,
        None::<&str>,
    )
}

pub(crate) async fn refresh_tun_menu_item(state: &AppState) -> anyhow::Result<()> {
    let Some(items) = state.tray_info_items().await else {
        return Ok(());
    };
    let lang_code = state.get_lang_code().await;
    let lang = Lang(lang_code.as_str());

    let is_admin = is_running_as_admin();
    let (available, enabled) = match state.refresh_tun_state().await {
        Ok(result) => result,
        Err(err) => {
            warn!("failed to refresh tun state: {err:#}");
            (false, false)
        }
    };

    let label = if !is_admin {
        lang.tr("tun_mode_admin_required")
    } else if !available {
        lang.tr("tun_mode_disabled")
    } else {
        lang.tr("tun_mode")
    };

    items.tun_mode.set_text(label)?;
    items.tun_mode.set_checked(enabled)?;
    items.tun_mode.set_enabled(is_admin && available)?;
    Ok(())
}
