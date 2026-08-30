//! Profile switch submenu: lists mihomo profiles (with subscription state) and
//! keeps the submenu fresh after profile changes.

use std::collections::HashMap;

use infiltrator_core::profiles as core_profiles;
use log::warn;
use tauri::{
    AppHandle, Wry,
    menu::{CheckMenuItem, IsMenuItem, MenuItem, PredefinedMenuItem, Submenu},
};
use tokio::time::Duration;

use crate::{
    app_state::AppState,
    locales::{Lang, Localizer},
};

use super::{
    helpers::{append_items_to_submenu, clear_submenu_items, truncate_label},
    ids::insert_profile_menu_id,
};

pub(super) async fn build_profile_switch_submenu(
    app: &AppHandle,
    profile_map: &mut HashMap<String, String>,
    lang: &Lang<'_>,
) -> tauri::Result<Submenu<Wry>> {
    let items = build_profile_switch_items(app, profile_map, lang).await?;
    let item_refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|i| i.as_ref()).collect();
    Submenu::with_items(app, lang.tr("profile_switch"), true, &item_refs)
}

async fn build_profile_switch_items(
    app: &AppHandle,
    profile_map: &mut HashMap<String, String>,
    lang: &Lang<'_>,
) -> tauri::Result<Vec<Box<dyn IsMenuItem<Wry>>>> {
    let mut profiles = match core_profiles::list_profile_infos().await {
        Ok(list) => list,
        Err(err) => {
            warn!("failed to list profiles: {err:#}");
            let failed_item = MenuItem::with_id(
                app,
                "profile-switch-error",
                lang.tr("profile_read_failed"),
                false,
                None::<&str>,
            )?;
            return Ok(vec![Box::new(failed_item)]);
        }
    };

    let active_profile = profiles.iter().find(|p| p.active).cloned();

    if profiles.is_empty() {
        let empty_item = MenuItem::with_id(
            app,
            "profile-switch-empty",
            lang.tr("profile_empty"),
            false,
            None::<&str>,
        )?;
        return Ok(vec![Box::new(empty_item)]);
    }

    profiles.sort_by(|a, b| b.active.cmp(&a.active).then_with(|| a.name.cmp(&b.name)));
    let has_subscription = profiles
        .iter()
        .any(|profile| profile.subscription_url.is_some());

    let max_visible = 10usize;
    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();

    for profile in profiles.iter().take(max_visible) {
        let label = if profile.subscription_url.is_some() {
            format!("{} ({})", profile.name, lang.tr("subscription"))
        } else {
            profile.name.clone()
        };
        let label = truncate_label(&label, 60);
        let menu_id = insert_profile_menu_id(profile_map, &profile.name);
        let item = CheckMenuItem::with_id(app, menu_id, label, true, profile.active, None::<&str>)?;
        items.push(Box::new(item));
    }

    if profiles.len() > max_visible {
        let mut overflow_items: Vec<CheckMenuItem<Wry>> = Vec::new();
        for profile in profiles.iter().skip(max_visible) {
            let menu_id = insert_profile_menu_id(profile_map, &profile.name);
            let item = CheckMenuItem::with_id(
                app,
                menu_id,
                truncate_label(&profile.name, 60),
                true,
                profile.active,
                None::<&str>,
            )?;
            overflow_items.push(item);
        }
        let overflow_refs: Vec<&dyn IsMenuItem<Wry>> = overflow_items
            .iter()
            .map(|item| item as &dyn IsMenuItem<Wry>)
            .collect();
        let overflow_submenu = Submenu::with_items(
            app,
            lang.tr("more_profiles"),
            true,
            overflow_refs.as_slice(),
        )?;
        items.push(Box::new(overflow_submenu));
    }

    items.push(Box::new(PredefinedMenuItem::separator(app)?));

    let update_all_item = MenuItem::with_id(
        app,
        "profile-update-all",
        lang.tr("update_all_subs"),
        has_subscription,
        None::<&str>,
    )?;
    items.push(Box::new(update_all_item));

    if let Some(active) = active_profile
        && active.subscription_url.is_some()
    {
        let auto_update_item = CheckMenuItem::with_id(
            app,
            format!("profile-auto-update-{}", active.name),
            lang.tr("auto_update_sub"),
            true,
            active.auto_update_enabled,
            None::<&str>,
        )?;
        items.push(Box::new(auto_update_item));
    }

    Ok(items)
}

pub(crate) async fn refresh_profile_switch_submenu(
    app: &AppHandle,
    state: &AppState,
) -> anyhow::Result<()> {
    let Some(items) = state.tray_info_items().await else {
        warn!("tray info items not available for profile switch submenu refresh");
        return Ok(());
    };

    let lang_code = state.get_lang_code().await;
    let lang = Lang(lang_code.as_str());
    let mut profile_map = HashMap::new();

    // Add retry logic with exponential backoff
    let max_attempts = 3;
    let mut attempt = 0;
    let mut delay = Duration::from_millis(100);

    loop {
        attempt += 1;

        let result = async {
            let menu_items = build_profile_switch_items(app, &mut profile_map, &lang).await?;
            clear_submenu_items(&items.profile_switch)?;
            append_items_to_submenu(&items.profile_switch, &menu_items)?;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        match result {
            Ok(()) => {
                state.set_tray_profile_map(profile_map).await;
                log::info!(
                    "profile switch submenu refreshed successfully (attempt {})",
                    attempt
                );
                return Ok(());
            }
            Err(err) => {
                if attempt >= max_attempts {
                    warn!(
                        "failed to refresh profile switch submenu after {} attempts: {:#}",
                        max_attempts, err
                    );
                    return Err(err);
                }
                warn!(
                    "profile switch submenu refresh failed (attempt {}/{}), retrying in {:?}: {:#}",
                    attempt, max_attempts, delay, err
                );
                tokio::time::sleep(delay).await;
                delay = delay.saturating_mul(2).min(Duration::from_secs(2));
            }
        }
    }
}
