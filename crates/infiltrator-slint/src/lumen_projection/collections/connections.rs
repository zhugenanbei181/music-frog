//! The connections page projection of the Slint surface.
//!
//! The active connection list is rendered from the shared connections page
//! model: the query fold, the sort key and the row order are the shared
//! reduction, so the widget's sort vocabulary only names a shared typed key.
//!
//! The closed connection history is a different observation with no shared
//! reduction yet, so its order stays a surface-local projection of the same
//! shared typed key. The seam that would let it join the shared model is
//! named at the projection that needs it.

use crate::widget_vocabulary;
use crate::{AppWindow, ObservationRow, slint_model};
use infiltrator_contract::surface_snapshot::{ConnectionSnapshot, SurfaceSnapshot};
use infiltrator_domain::connection_view::ConnectionSortKey;
use infiltrator_presentation::connection_page_model::ConnectionsPageModel;
use infiltrator_shared::byte_units::format_bytes;
use std::cmp::Reverse;

/// Projects the connections page from the shared page model.
pub(super) fn project_connections(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    if window.get_connection_show_closed() {
        window.set_connections(slint_model(closed_connection_rows(window, snapshot)));
        return;
    }
    let mut model = ConnectionsPageModel::<ConnectionSnapshot>::default();
    model.edit_query(window.get_connection_search().as_str());
    model.select_sort(widget_vocabulary::connection_sort_key(
        window.get_connection_sort().as_str(),
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
    window.set_connections(slint_model(
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

/// The closed connection history, ordered by the surface-local projection.
///
/// The shared connections page model reduces the *active* connection list.
/// The closed history is a different observation: `ClosedConnectionSnapshot`
/// has no `ConnectionView` implementation, so it cannot join the shared
/// model and no shared reduction can order it. The order below is therefore
/// the surface's own projection of the *shared* sort key the widget named,
/// and the shared query fold cannot filter it either.
///
/// The seam that closes both gaps is one `ConnectionView` implementation for
/// `ClosedConnectionSnapshot` in `infiltrator-domain`, which owns the
/// domain-to-contract seam:
///
/// ```text
/// impl ConnectionView for ClosedConnectionSnapshot {
///     fn view_id(&self) -> &str { &self.id }
///     fn view_host(&self) -> &str { &self.destination }
///     fn view_process_path(&self) -> &str { &self.outbound }
///     fn view_upload_total(&self) -> u64 { self.upload_bytes }
///     fn view_download_total(&self) -> u64 { self.download_bytes }
///     fn view_start(&self) -> &str { &self.closed_at }
/// }
/// ```
///
/// With that implementation the closed list joins the shared model and this
/// projection disappears.
fn closed_connection_rows(window: &AppWindow, snapshot: &SurfaceSnapshot) -> Vec<ObservationRow> {
    let query = window
        .get_connection_search()
        .to_string()
        .trim()
        .to_lowercase();
    let mut connections = snapshot
        .closed_connections
        .records
        .iter()
        .collect::<Vec<_>>();
    match widget_vocabulary::connection_sort_key(window.get_connection_sort().as_str()) {
        ConnectionSortKey::UploadDesc => {
            connections.sort_by_key(|connection| Reverse(connection.upload_bytes))
        }
        ConnectionSortKey::DownloadDesc => {
            connections.sort_by_key(|connection| Reverse(connection.download_bytes))
        }
        ConnectionSortKey::HostAsc => {
            connections.sort_by(|left, right| left.destination.cmp(&right.destination))
        }
        ConnectionSortKey::LatestDesc => {
            connections.sort_by(|left, right| right.closed_at.cmp(&left.closed_at))
        }
        // The rate keys rank on instantaneous rates the closed history does
        // not carry, so they keep the observation order.
        ConnectionSortKey::DownloadRateDesc | ConnectionSortKey::UploadRateDesc => {}
    }
    connections
        .into_iter()
        .map(|connection| ObservationRow {
            id: connection.id.clone().into(),
            title: connection.destination.clone().into(),
            accessible_title: connection.destination.clone().into(),
            flag_code: "".into(),
            flag_index: -1,
            delay_ms: -1,
            subtitle: format!("{} · {}", connection.outbound, connection.rule).into(),
            trailing: format!(
                "↑ {} · ↓ {}",
                format_bytes(connection.upload_bytes),
                format_bytes(connection.download_bytes)
            )
            .into(),
            active: false,
        })
        .filter(|row| {
            query.is_empty()
                || row.title.to_string().to_lowercase().contains(&query)
                || row.subtitle.to_string().to_lowercase().contains(&query)
                || row.id.to_string().to_lowercase().contains(&query)
        })
        .collect()
}
