//! Proxy groups submenu: selectable groups and nodes with delay labels, kept
//! fresh after proxy switches.

use std::collections::HashMap;

use log::warn;
use mihomo_api::proxy::Proxy;
use tauri::{
    AppHandle, Wry,
    menu::{CheckMenuItem, IsMenuItem, MenuItem, Submenu},
};

use crate::{
    app_state::AppState,
    locales::{Lang, Localizer},
};

use super::{
    helpers::{append_items_to_submenu, clear_submenu_items, truncate_label},
    ids::insert_proxy_menu_id,
};

pub(super) async fn build_proxy_groups_submenu(
    app: &AppHandle,
    state: &AppState,
    proxy_map: &mut HashMap<String, (String, String)>,
    lang: &Lang<'_>,
) -> tauri::Result<Submenu<Wry>> {
    let items = build_proxy_groups_items(app, state, proxy_map, lang).await?;
    let item_refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|i| i.as_ref()).collect();
    Submenu::with_items(app, lang.tr("proxy_groups"), true, item_refs.as_slice())
}

async fn build_proxy_groups_items(
    app: &AppHandle,
    state: &AppState,
    proxy_map: &mut HashMap<String, (String, String)>,
    lang: &Lang<'_>,
) -> tauri::Result<Vec<Box<dyn IsMenuItem<Wry>>>> {
    let proxies = match state.refresh_proxy_groups().await {
        Ok(proxies) => proxies,
        Err(err) => {
            warn!("failed to refresh proxies: {err:#}");
            let failed_item = MenuItem::with_id(
                app,
                "proxy-groups-error",
                lang.tr("proxy_groups_read_failed"),
                false,
                None::<&str>,
            )?;
            return Ok(vec![Box::new(failed_item)]);
        }
    };

    let mut groups: Vec<(String, Proxy)> = proxies
        .iter()
        .filter(|(_, info)| is_selectable_group(info))
        .map(|(name, info)| (name.clone(), info.clone()))
        .collect();
    if groups.is_empty() {
        let empty_item = MenuItem::with_id(
            app,
            "proxy-groups-empty",
            lang.tr("proxy_groups_empty"),
            false,
            None::<&str>,
        )?;
        return Ok(vec![Box::new(empty_item)]);
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));

    let max_groups = 5usize;
    let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::new();
    for (name, info) in groups.iter().take(max_groups) {
        let submenu = build_proxy_group_submenu(app, &proxies, name, info, proxy_map, lang)?;
        items.push(Box::new(submenu));
    }

    if groups.len() > max_groups {
        let mut overflow_submenus: Vec<Submenu<Wry>> = Vec::new();
        let mut overflow_items: Vec<&dyn IsMenuItem<Wry>> = Vec::new();
        for (name, info) in groups.iter().skip(max_groups) {
            let submenu = build_proxy_group_submenu(app, &proxies, name, info, proxy_map, lang)?;
            overflow_submenus.push(submenu);
        }
        for submenu in &overflow_submenus {
            overflow_items.push(submenu);
        }
        let overflow_submenu =
            Submenu::with_items(app, lang.tr("more_groups"), true, overflow_items.as_slice())?;
        items.push(Box::new(overflow_submenu));
    }

    Ok(items)
}

fn build_proxy_group_submenu(
    app: &AppHandle,
    proxies: &std::collections::HashMap<String, Proxy>,
    group_name: &str,
    group_info: &Proxy,
    proxy_map: &mut HashMap<String, (String, String)>,
    lang: &Lang<'_>,
) -> tauri::Result<Submenu<Wry>> {
    let nodes = group_info.all().map(|all| all.to_vec()).unwrap_or_default();
    if nodes.is_empty() {
        let empty_item = MenuItem::with_id(
            app,
            format!("proxy-empty-{group_name}"),
            lang.tr("no_nodes"),
            false,
            None::<&str>,
        )?;
        return Submenu::with_items(app, group_name, true, &[&empty_item]);
    }

    let current = group_info.now().unwrap_or("");
    let max_nodes = 20usize;

    let mut node_items: Vec<CheckMenuItem<Wry>> = Vec::new();
    for node in nodes.iter().take(max_nodes) {
        let label = truncate_label(&build_proxy_node_label(proxies, node), 60);
        let menu_id = insert_proxy_menu_id(proxy_map, group_name, node);
        let item =
            CheckMenuItem::with_id(app, menu_id, label, true, current == node, None::<&str>)?;
        node_items.push(item);
    }
    let mut item_refs: Vec<&dyn IsMenuItem<Wry>> = node_items
        .iter()
        .map(|item| item as &dyn IsMenuItem<Wry>)
        .collect();

    let mut overflow_submenus: Vec<Submenu<Wry>> = Vec::new();
    if nodes.len() > max_nodes {
        let mut overflow_items: Vec<CheckMenuItem<Wry>> = Vec::new();
        for node in nodes.iter().skip(max_nodes) {
            let label = truncate_label(&build_proxy_node_label(proxies, node), 60);
            let menu_id = insert_proxy_menu_id(proxy_map, group_name, node);
            let item =
                CheckMenuItem::with_id(app, menu_id, label, true, current == node, None::<&str>)?;
            overflow_items.push(item);
        }
        let overflow_refs: Vec<&dyn IsMenuItem<Wry>> = overflow_items
            .iter()
            .map(|item| item as &dyn IsMenuItem<Wry>)
            .collect();
        let overflow_submenu =
            Submenu::with_items(app, lang.tr("more_nodes"), true, overflow_refs.as_slice())?;
        overflow_submenus.push(overflow_submenu);
        if let Some(submenu) = overflow_submenus.last() {
            item_refs.push(submenu);
        }
    }

    Submenu::with_items(app, group_name, true, item_refs.as_slice())
}

pub(crate) fn build_proxy_node_label(
    proxies: &std::collections::HashMap<String, Proxy>,
    node: &str,
) -> String {
    if let Some(delay) = proxies
        .get(node)
        .and_then(|info| info.history().last().map(|entry| entry.delay))
    {
        format!("{node} ({delay}ms)")
    } else {
        node.to_string()
    }
}

pub(crate) fn is_selectable_group(info: &Proxy) -> bool {
    info.is_group()
}

pub(crate) async fn refresh_proxy_groups_submenu(
    app: &AppHandle,
    state: &AppState,
) -> anyhow::Result<()> {
    let Some(items) = state.tray_info_items().await else {
        return Ok(());
    };
    let lang_code = state.get_lang_code().await;
    let lang = Lang(lang_code.as_str());
    let mut proxy_map = HashMap::new();
    {
        let menu_items = build_proxy_groups_items(app, state, &mut proxy_map, &lang).await?;
        clear_submenu_items(&items.proxy_groups)?;
        append_items_to_submenu(&items.proxy_groups, &menu_items)?;
    }
    state.set_tray_proxy_map(proxy_map).await;
    Ok(())
}
