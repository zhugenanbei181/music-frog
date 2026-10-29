//! The business panels of the proxies workbench page.

use super::Panel;
use crate::surface::business::BusinessId;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::proxies_page_model::ProxiesPageModel;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_ui_presentation::copy::TextCatalog;

/// The business panels the proxies page carries.
pub(super) fn panels(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    let groups = published_groups(snapshot);
    let node = published_node(snapshot);
    let nodes = published_nodes(snapshot);
    let mut model = ProxiesPageModel::default();
    if let Some(preferences) = snapshot.outbound_ui_preferences.data.as_ref() {
        model.observe_preferences(preferences);
    }
    let _query = model.search_text().to_owned();
    let inventory_revision = snapshot
        .outbound_inventory
        .data
        .as_ref()
        .map(|inventory| inventory.revision().value())
        .unwrap_or(0);

    let failure = super::page_failure(&snapshot.outbound_inventory.status);
    let mut panels = vec![
        Panel::new(
            BusinessId::ProxiesCustomNodeModal,
            catalog.text("custom_node_title"),
            catalog.text("custom_node_name").into_owned(),
            vec![
                catalog.text("custom_node_type").into_owned(),
                catalog.text("custom_node_server").into_owned(),
                catalog.text("custom_node_port").into_owned(),
            ],
        ),
        Panel::new(
            BusinessId::ProxiesGroupExpanded,
            catalog.text("proxy_groups"),
            groups.len().to_string(),
            groups
                .iter()
                .map(|group| group.value().to_owned())
                .collect(),
        ),
        Panel::new(
            BusinessId::ProxiesGroupReorder,
            catalog.text("proxies_reorder_title"),
            catalog.text("proxies_reorder_description").into_owned(),
            if groups.is_empty() {
                let revision_str = inventory_revision.to_string();
                vec![localize(
                    catalog.0,
                    "proxies_reorder_empty_revision",
                    &[("revision", revision_str)],
                )]
            } else {
                groups
                    .iter()
                    .enumerate()
                    .map(|(i, group)| format!("{}. {}", i + 1, group.value()))
                    .collect()
            },
        ),
        Panel::new(
            BusinessId::ProxiesNodeDetailDrawer,
            catalog.text("proxy_inspection_title"),
            node,
            {
                let mut facts = vec![
                    catalog.text("proxy_inspection_protocol").into_owned(),
                    catalog.text("proxy_inspection_delay").into_owned(),
                ];
                facts.extend(nodes.clone());
                facts
            },
        ),
        Panel::new(
            BusinessId::ProxiesProbeSettings,
            catalog.text("proxy_probe_settings_title"),
            catalog
                .text("proxy_probe_settings_description")
                .into_owned(),
            vec![
                catalog.text("runtime_probe_url_label").into_owned(),
                catalog.text("runtime_probe_timeout_label").into_owned(),
            ],
        ),
        Panel::new(
            BusinessId::ProxiesUriImportPreview,
            catalog.text("proxies_add_node_title"),
            "Imported-Node (VLESS · node.example.com:443)",
            vec![
                "vless://b831381d-6324-4d53-ad4f-8cda48b30811@node.example.com:443?security=tls#Imported-Node".into(),
                "VLESS · node.example.com:443 (TLS)".into(),
                catalog.text("custom_node_type").into_owned() + ": VLESS",
            ],
        ),
    ];
    let empty = super::page_empty(&snapshot.outbound_inventory.status);
    for panel in &mut panels {
        panel.failure = failure.clone();
        panel.empty = empty;
    }
    panels
}

/// The groups the published outbound inventory carries.
fn published_groups(snapshot: &SurfaceSnapshot) -> Vec<OutboundId> {
    let Some(inventory) = snapshot.outbound_inventory.data.as_ref() else {
        return Vec::new();
    };
    let OutboundInventoryState::Ready { collections, .. } = inventory.inventory() else {
        return Vec::new();
    };
    collections.iter().map(|group| group.id.clone()).collect()
}

/// The display names of every published outbound.
fn published_nodes(snapshot: &SurfaceSnapshot) -> Vec<String> {
    let Some(inventory) = snapshot.outbound_inventory.data.as_ref() else {
        return Vec::new();
    };
    let OutboundInventoryState::Ready { entries, .. } = inventory.inventory() else {
        return Vec::new();
    };
    entries
        .iter()
        .map(|entry| {
            entry
                .display_name
                .clone()
                .unwrap_or_else(|| entry.id.value().to_owned())
        })
        .collect()
}

/// The display name of one published outbound.
fn published_node(snapshot: &SurfaceSnapshot) -> String {
    let Some(inventory) = snapshot.outbound_inventory.data.as_ref() else {
        return String::new();
    };
    let OutboundInventoryState::Ready { entries, .. } = inventory.inventory() else {
        return String::new();
    };
    entries
        .first()
        .and_then(|entry| entry.display_name.clone())
        .unwrap_or_default()
}
