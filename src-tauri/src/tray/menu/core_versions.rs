//! Core versions submenu: installed mihomo versions with use/delete actions,
//! kept fresh after core updates.

use log::warn;
use mihomo_version::manager::{VersionInfo, VersionManager};
use tauri::{
    AppHandle, Wry,
    menu::{IsMenuItem, MenuItem, Submenu},
};

use crate::{
    app_state::AppState,
    locales::{Lang, Localizer},
};

use super::helpers::{append_items_to_submenu, clear_submenu_items};

pub(super) fn build_core_versions_submenu(
    app: &AppHandle,
    lang: &Lang<'_>,
    versions: &[VersionInfo],
) -> tauri::Result<Submenu<Wry>> {
    let items = build_core_versions_items(app, lang, versions)?;
    let item_refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|item| item.as_ref()).collect();
    Submenu::with_items(
        app,
        lang.tr("downloaded_version"),
        true,
        item_refs.as_slice(),
    )
}

fn build_core_versions_items(
    app: &AppHandle,
    lang: &Lang<'_>,
    versions: &[VersionInfo],
) -> tauri::Result<Vec<Box<dyn IsMenuItem<Wry>>>> {
    if versions.is_empty() {
        let empty_versions_item =
            MenuItem::with_id(app, "core-empty", lang.tr("empty"), false, None::<&str>)?;
        return Ok(vec![Box::new(empty_versions_item)]);
    }

    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    for version in versions {
        let use_item = MenuItem::with_id(
            app,
            format!("core-use-{}", version.version),
            lang.tr("use"),
            true,
            None::<&str>,
        )?;
        let delete_item = MenuItem::with_id(
            app,
            format!("core-delete-{}", version.version),
            lang.tr("delete"),
            true,
            None::<&str>,
        )?;
        let submenu = Submenu::with_items(app, &version.version, true, &[&use_item, &delete_item])?;
        items.push(Box::new(submenu));
    }

    Ok(items)
}

pub(crate) async fn refresh_core_versions_submenu(
    app: &AppHandle,
    state: &AppState,
) -> anyhow::Result<()> {
    let Some(items) = state.tray_info_items().await else {
        return Ok(());
    };
    let lang_code = state.get_lang_code().await;
    let lang = Lang(lang_code.as_str());
    let versions = match VersionManager::new() {
        Ok(vm) => match vm.list_installed().await {
            Ok(list) => list,
            Err(err) => {
                warn!("failed to read installed versions: {err}");
                Vec::new()
            }
        },
        Err(err) => {
            warn!("failed to read installed versions: {err}");
            Vec::new()
        }
    };
    let menu_items = build_core_versions_items(app, &lang, &versions)?;
    clear_submenu_items(&items.core_versions)?;
    append_items_to_submenu(&items.core_versions, &menu_items)?;
    Ok(())
}
