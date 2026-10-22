//! The business panels of the connections workbench page.

use super::Panel;
use crate::surface::business::BusinessId;
use infiltrator_contract::surface_snapshot::{ConnectionSnapshot, SurfaceSnapshot};
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_presentation::connection_grouping::ConnectionGroupingState;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;

/// The business panels the connections page carries.
pub(super) fn panels(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    let page = snapshot.pages.connections.data.as_ref();
    let connections: Vec<ConnectionSnapshot> = page
        .map(|page| page.connections.clone())
        .unwrap_or_default();
    let total = page.map(|page| page.total_connections).unwrap_or(0);

    // The grouping fold is the shared reduction: the surface asks the shared
    // state which groups the published connections form and renders its rows.
    let mut grouping = ConnectionGroupingState::<ConnectionSnapshot>::default();
    grouping.observe(&connections);
    grouping.select(ConnectionGroupingMode::ByProcess);
    let group_rows = grouping
        .rows()
        .iter()
        .map(|row| format!("{} · {}", row.key, row.traffic))
        .collect::<Vec<_>>();

    let first = connections.first();
    // The drawer shows the facts of every published connection, so the
    // owner's order and the owner's removals reach the panel.
    let detail = connections
        .iter()
        .map(|connection| {
            format!(
                "{} · {} · {}",
                connection.host, connection.process, connection.rule
            )
        })
        .collect::<Vec<_>>();

    let failure = super::page_failure(&snapshot.pages.connections.status);
    let mut panels = vec![
        Panel::new(
            BusinessId::ConnectionsCloseAllConfirm,
            catalog.text("modal_confirm_disconnect_all_title"),
            format!("{total}"),
            vec![
                catalog
                    .text("modal_confirm_disconnect_all_desc")
                    .into_owned(),
                format!("{total}"),
            ],
        ),
        Panel::new(
            BusinessId::ConnectionsDetailsDrawer,
            catalog.text("runtime_connections_title"),
            first
                .map(|connection| connection.host.clone())
                .unwrap_or_default(),
            detail,
        ),
        Panel::new(
            BusinessId::ConnectionsGrouping,
            catalog.text("runtime_conn_sort"),
            format!("{}", grouping.rows().len()),
            group_rows,
        ),
    ];
    let empty = super::page_empty(&snapshot.pages.connections.status);
    for panel in &mut panels {
        panel.failure = failure.clone();
        panel.empty = empty;
    }
    panels
}
