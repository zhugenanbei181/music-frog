//! The business panels of the overview workbench page.

use super::Panel;
use crate::surface::business::BusinessId;
use infiltrator_contract::reconnect_mask::ReconnectMaskStatus;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::traffic_topology_projection::status_label;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;

/// The business panels the overview page carries.
pub(super) fn panels(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    let mask = snapshot.reconnect_mask.clone();
    let mask_status = match mask.status {
        ReconnectMaskStatus::Normal => catalog
            .text("overview_reconnect_status_normal")
            .into_owned(),
        ReconnectMaskStatus::Reloading => catalog
            .text("overview_reconnect_status_reloading")
            .into_owned(),
        ReconnectMaskStatus::Reconnecting => catalog
            .text("overview_reconnect_status_reconnecting")
            .into_owned(),
        ReconnectMaskStatus::Degraded => catalog
            .text("overview_reconnect_status_degraded")
            .into_owned(),
    };
    let topology = status_label(&snapshot.traffic_topology, catalog);
    let nodes = snapshot.traffic_topology.nodes.len().to_string();
    let links = snapshot.traffic_topology.links.len().to_string();
    let topology_summary = localize(
        catalog.0,
        "overview_topology_summary_pattern",
        &[("nodes", nodes), ("links", links)],
    );

    vec![
        Panel::new(
            BusinessId::OverviewReconnectMask,
            catalog.text("overview_core"),
            mask_status,
            vec![
                catalog.text("overview_toggle_pending").into_owned(),
                mask.message.clone().unwrap_or_default(),
            ],
        ),
        Panel::new(
            BusinessId::OverviewTopologyDrilldown,
            catalog.text("overview_topology_title"),
            topology,
            vec![
                topology_summary,
                catalog.text("overview_topology_label").into_owned(),
            ],
        ),
    ]
}
