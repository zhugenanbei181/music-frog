//! Proxy mode submenu (rule / global / direct / script) reflecting live
//! runtime state.

use log::warn;
use tauri::{
    AppHandle, Wry,
    menu::{CheckMenuItem, Submenu},
};

use crate::{
    app_state::AppState,
    locales::{Lang, Localizer},
};

pub(super) struct ModeMenuItems {
    pub(super) rule: CheckMenuItem<Wry>,
    pub(super) global: CheckMenuItem<Wry>,
    pub(super) direct: CheckMenuItem<Wry>,
    pub(super) script: CheckMenuItem<Wry>,
}

pub(super) async fn build_mode_submenu(
    app: &AppHandle,
    state: &AppState,
    lang: &Lang<'_>,
) -> tauri::Result<(Submenu<Wry>, ModeMenuItems)> {
    let mut current_mode: Option<String> = None;
    let mut script_enabled = false;
    let mut menu_enabled = false;
    if let Ok(runtime) = state.runtime().await {
        match runtime.client().get_config().await {
            Ok(config) => {
                menu_enabled = true;
                let mode = config.mode.trim().to_ascii_lowercase();
                if !mode.is_empty() {
                    current_mode = Some(mode);
                }
                script_enabled = is_script_enabled(config.script.as_ref());
            }
            Err(err) => {
                warn!("failed to read config for mode: {err:#}");
            }
        }
    }
    state.set_current_mode(current_mode.clone()).await;

    let is_rule = current_mode.as_deref() == Some("rule");
    let is_global = current_mode.as_deref() == Some("global");
    let is_direct = current_mode.as_deref() == Some("direct");
    let is_script = current_mode.as_deref() == Some("script");

    let rule_item = CheckMenuItem::with_id(
        app,
        "mode-rule",
        lang.tr("mode_rule"),
        menu_enabled,
        is_rule,
        None::<&str>,
    )?;
    let global_item = CheckMenuItem::with_id(
        app,
        "mode-global",
        lang.tr("mode_global"),
        menu_enabled,
        is_global,
        None::<&str>,
    )?;
    let direct_item = CheckMenuItem::with_id(
        app,
        "mode-direct",
        lang.tr("mode_direct"),
        menu_enabled,
        is_direct,
        None::<&str>,
    )?;
    let script_label = if script_enabled {
        lang.tr("mode_script").into_owned()
    } else {
        format!("{} ({})", lang.tr("mode_script"), lang.tr("disabled"))
    };
    let script_item = CheckMenuItem::with_id(
        app,
        "mode-script",
        script_label,
        menu_enabled && script_enabled,
        is_script,
        None::<&str>,
    )?;

    let submenu = Submenu::with_items(
        app,
        lang.tr("proxy_mode"),
        true,
        &[&rule_item, &global_item, &direct_item, &script_item],
    )?;

    Ok((
        submenu,
        ModeMenuItems {
            rule: rule_item,
            global: global_item,
            direct: direct_item,
            script: script_item,
        },
    ))
}

pub(crate) fn is_script_enabled(script: Option<&serde_json::Value>) -> bool {
    match script {
        Some(value) => value
            .get("enable")
            .and_then(|v: &serde_json::Value| v.as_bool())
            .unwrap_or(true),
        None => false,
    }
}
