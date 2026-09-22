//! Reactive view state and shared reductions for the Connections page
//! (DUAL-13-02 / 13-07 / 13-13).
//!
//! The page scene lives in [`super::connections`]. This module owns the parts
//! that react to input: the aggregation segmented control, the keyword search
//! field, range teardown, and the two-step close-all confirmation. Every
//! reduction delegates to `infiltrator_domain::connection_view` so the Bevy
//! page and the Iced page compute the same buckets and matches.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Changed, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res};
use bevy::ui::prelude::{Display, Node};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_domain::connection_view::{self, ConnectionGroupingMode, ConnectionView};

use crate::pages::connections::{ConnectionItem, ConnectionsProjection, LastConnectionsProjection};

/// Marker on a single flat connection row root; the payload is the row index
/// into the last projection so search can hide non-matching rows in place.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionRow(pub usize);

/// Marker on the container wrapping every flat connection row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnRowsContainer;

/// Marker on the aggregation summary line (grouped modes only).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnAggregationSummary;

/// Marker on the aggregation summary container, toggled with the mode.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnAggregationSummaryContainer;

/// Marker on the wrapper of the keyword search text field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnSearchField;

/// Marker on the "close every filtered connection" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloseFilteredConnectionsButton;

/// Marker on the close-all button's caption, restamped when the button arms.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloseAllConnectionsLabel;

/// Shared grouping selection, set by the aggregation pills.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionsViewState {
    pub grouping: ConnectionGroupingMode,
}

/// Two-step confirmation latch for close-all.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionsCloseAllState {
    pub armed: bool,
}

/// Bevy's local projection row satisfies the shared reduction seam, so the
/// aggregation and search functions accept it directly.
impl ConnectionView for ConnectionItem {
    fn view_id(&self) -> &str {
        &self.id
    }

    fn view_host(&self) -> &str {
        &self.host
    }

    fn view_process_path(&self) -> &str {
        &self.process
    }

    fn view_upload_total(&self) -> u64 {
        self.upload_total
    }

    fn view_download_total(&self) -> u64 {
        self.download_total
    }

    fn view_search_terms(&self) -> Vec<&str> {
        vec![
            self.id.as_str(),
            self.host.as_str(),
            self.process.as_str(),
            self.rule.as_str(),
            self.chain.as_str(),
        ]
    }
}

/// One-line aggregation summary rendered for the grouped modes.
pub(crate) fn aggregation_summary(
    projection: &ConnectionsProjection,
    mode: ConnectionGroupingMode,
) -> String {
    let aggregates = connection_view::aggregate_connections(&projection.connections, mode);
    if aggregates.is_empty() {
        return "暂无聚合数据".to_owned();
    }
    let mode_label = match mode {
        ConnectionGroupingMode::ByProcess => "按应用进程聚合",
        ConnectionGroupingMode::ByHost => "按目标域名聚合",
        ConnectionGroupingMode::Flat => "实时流",
    };
    let mut parts = vec![format!("{mode_label} · 共 {} 组", aggregates.len())];
    for aggregate in aggregates.iter().take(6) {
        parts.push(format!("{} ({})", aggregate.key, aggregate.count));
    }
    parts.join(" · ")
}

/// Read the live keyword from the mounted search field, if any.
pub(crate) fn search_field_text<F: bevy::ecs::query::QueryFilter>(
    search_fields: &Query<&Children, With<ConnSearchField>>,
    text_fields: &Query<&TextField, F>,
) -> Option<String> {
    search_fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text().to_owned())
}

/// DUAL-13-13: hide flat rows that do not match the keyword. Runs only when a
/// search field changed, and only in flat mode (grouped modes hide the rows
/// wholesale through the pill observer).
pub(crate) fn sync_connections_search(
    search_fields: Query<&Children, With<ConnSearchField>>,
    text_fields: Query<&TextField, Changed<TextField>>,
    last: Option<Res<LastConnectionsProjection>>,
    view: Option<Res<ConnectionsViewState>>,
    mut rows: Query<(&mut Node, &ConnectionRow)>,
) {
    if view.map(|view| !view.grouping.is_flat()).unwrap_or(false) {
        return;
    }
    let Some(projection) = last.and_then(|last| last.0.clone()) else {
        return;
    };
    let Some(query) = search_field_text(&search_fields, &text_fields) else {
        return;
    };
    for (mut node, row) in &mut rows {
        let visible = projection
            .connections
            .get(row.0)
            .map(|item| connection_view::matches_search(item, &query))
            .unwrap_or(false);
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
}

/// Restamp every aggregation pill fill for the active mode.
pub(crate) fn restamp_aggregation_pills(
    palette: &infiltrator_bevy_widgets::palette::UiPalette,
    pills: &mut Query<(
        &mut bevy::ui::prelude::BackgroundColor,
        &crate::pages::connections::ConnAggregationPill,
    )>,
    active: ConnectionGroupingMode,
) {
    for (mut fill, pill) in pills.iter_mut() {
        fill.0 = if pill.0 == active {
            palette.accent_container
        } else {
            palette.surface
        };
    }
}

/// Restamp the aggregation summary text from the last projection.
pub(crate) fn restamp_aggregation_summary<F: bevy::ecs::query::QueryFilter>(
    summary: &mut Query<(&mut Text, &ConnAggregationSummary), F>,
    projection: &ConnectionsProjection,
    mode: ConnectionGroupingMode,
) {
    let text = aggregation_summary(projection, mode);
    for (mut node_text, _) in summary.iter_mut() {
        node_text.0 = text.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, host: &str, process: &str, up: u64, down: u64) -> ConnectionItem {
        ConnectionItem {
            id: id.to_owned(),
            host: host.to_owned(),
            process: process.to_owned(),
            rule: "DIRECT".to_owned(),
            chain: "DIRECT".to_owned(),
            upload_bps: 0.0,
            download_bps: 0.0,
            upload_total: up,
            download_total: down,
        }
    }

    #[test]
    fn projection_row_reuses_shared_search() {
        let row = item("c1", "api.github.com:443", "/usr/bin/git", 10, 20);
        assert!(connection_view::matches_search(&row, "github"));
        assert!(connection_view::matches_search(&row, "GIT"));
        assert!(!connection_view::matches_search(&row, "cloudflare"));
    }

    #[test]
    fn aggregation_summary_reports_buckets() {
        let projection = ConnectionsProjection {
            total_connections: 2,
            total_upload_bytes: 30,
            total_download_bytes: 40,
            connections: vec![
                item("c1", "a.com:443", "/usr/bin/git", 10, 20),
                item("c2", "b.com:443", "/usr/bin/git", 20, 20),
            ],
        };
        let summary = aggregation_summary(&projection, ConnectionGroupingMode::ByProcess);
        assert!(summary.contains("按应用进程聚合"));
        assert!(summary.contains("git (2)"));
    }
}
