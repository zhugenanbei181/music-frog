//! The connections page projection of the Slint surface.
//!
//! The active connection list and the closed connection history are rendered
//! from the shared connections page model: the query fold, the sort key and
//! the row order are the shared reduction, so the widget's sort vocabulary
//! only names a shared typed key.
//!
//! The closed record implements the shared connection view, so the history
//! joins the very same model as the open list. A closed record carries no
//! process, so the shared reduction groups it under the honest "unknown"
//! bucket instead of inventing a process name.

use crate::widget_vocabulary;
use crate::{AppWindow, ObservationRow, refresh_model};
use infiltrator_contract::surface_snapshot::{ConnectionSnapshot, SurfaceSnapshot};
use infiltrator_contract::traffic_history::ClosedConnectionSnapshot;
use infiltrator_domain::connection_view::ConnectionView;
use infiltrator_presentation::connection_page_model::ConnectionsPageModel;
use infiltrator_shared::byte_units::format_bytes;

/// Projects the connections page from the shared page model.
pub(super) fn project_connections(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    if window.get_connection_show_closed() {
        window.set_connections(refresh_model(&window.get_connections(), closed_connection_rows(window, snapshot)));
        return;
    }
    let mut model = ConnectionsPageModel::<ConnectionSnapshot>::default();
    model.edit_query(window.get_connection_search().as_str());
    model.select_sort(widget_vocabulary::connection_sort_key(
        window.get_connection_sort(),
    ));
    if let Some(page) = snapshot.pages.connections.data.as_ref() {
        model.observe(&page.connections);
    }
    let mut rows = model
        .visible_rows()
        .into_iter()
        .cloned()
        .collect::<Vec<ConnectionSnapshot>>();
    // The row order is the shared reduction for the model's own sort key.
    model.sort_rows(&mut rows);
    window.set_connections(refresh_model(&window.get_connections(), 
        rows.into_iter().map(connection_row).collect::<Vec<_>>(),
    ));
}

/// The active connection row the surface renders.
fn connection_row(connection: ConnectionSnapshot) -> ObservationRow {
    ObservationRow {
        id: connection.id.clone().into(),
        title: connection.host.clone().into(),
        accessible_title: connection.host.clone().into(),
        flag_code: "".into(),
        flag_index: -1,
        delay_ms: -1,
        subtitle: format!("{} · {}", connection.network, connection.process).into(),
        trailing: format!(
            "↑ {} · ↓ {}",
            format_bytes(connection.upload_total),
            format_bytes(connection.download_total)
        )
        .into(),
        active: true,
    }
}

/// The closed connection history, projected through the shared page model.
///
/// The closed record is a connection view, so the history joins the shared
/// model: the query fold, the sort key and the row order are the shared
/// reduction and the surface keeps no projection of its own.
fn closed_connection_rows(window: &AppWindow, snapshot: &SurfaceSnapshot) -> Vec<ObservationRow> {
    let mut model = ConnectionsPageModel::<ClosedConnectionSnapshot>::default();
    model.edit_query(window.get_connection_search().as_str());
    model.select_sort(widget_vocabulary::connection_sort_key(
        window.get_connection_sort(),
    ));
    model.observe(&snapshot.closed_connections.records);
    let mut rows = model
        .visible_rows()
        .into_iter()
        .cloned()
        .collect::<Vec<ClosedConnectionSnapshot>>();
    model.sort_rows(&mut rows);
    rows.into_iter().map(closed_connection_row).collect()
}

/// The closed connection row the surface renders.
///
/// The closed record carries no process path, so the row shows the outbound
/// and the matched rule it really carries and never a process it never had.
fn closed_connection_row(connection: ClosedConnectionSnapshot) -> ObservationRow {
    ObservationRow {
        id: connection.view_id().to_owned().into(),
        title: connection.view_host().to_owned().into(),
        accessible_title: connection.view_host().to_owned().into(),
        flag_code: "".into(),
        flag_index: -1,
        delay_ms: -1,
        subtitle: format!(
            "{} · {}",
            connection.view_joined_chain(),
            connection.view_rule()
        )
        .into(),
        trailing: format!(
            "↑ {} · ↓ {}",
            format_bytes(connection.view_upload_total()),
            format_bytes(connection.view_download_total())
        )
        .into(),
        active: false,
    }
}
