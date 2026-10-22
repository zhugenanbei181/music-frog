//! The business panels of the overview workbench page.

use super::Panel;
use crate::surface::business::BusinessId;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::traffic_topology_projection::status_label;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;

/// The business panels the overview page carries.
pub(super) fn panels(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    let mask = snapshot.reconnect_mask.clone();
    let mask_status = format!("{:?}", mask.status);
    let topology = status_label(&snapshot.traffic_topology, catalog);
    let nodes = snapshot.traffic_topology.nodes.len();
    let links = snapshot.traffic_topology.links.len();

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
                format!("{nodes} · {links}"),
                catalog.text("overview_topology_label").into_owned(),
            ],
        ),
    ]
}
