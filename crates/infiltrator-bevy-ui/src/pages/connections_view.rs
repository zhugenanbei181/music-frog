//! Reactive view state and shared reductions for the Connections page
//! (DUAL-13-02 / 13-07 / 13-12 / 13-13).
//!
//! The page scene lives in [`super::connections`]. This module owns the parts
//! that react to input: the aggregation segmented control, the instantaneous
//! rate sort pills, the keyword search field, range teardown, and the
//! two-step close-all confirmation. Every reduction delegates to
//! `infiltrator_domain::connection_view` so the Bevy page and the Iced page
//! compute the same buckets, order and matches.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Changed, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, JustifyContent, Node, UiRect, Val,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_domain::connection_view::{
    self, ConnectionGroupingMode, ConnectionSortKey, ConnectionView,
};
use std::collections::HashMap;

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
    /// DUAL-13-12: shared order of the flat rows; defaults to the same
    /// cumulative-download order the Iced surface starts with.
    pub sort: ConnectionSortKey,
}

/// DUAL-13-12: one sort pill on the connections table header. The payload is
/// the shared sort key, so both surfaces order through the same reduction.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnSortPill(pub ConnectionSortKey);

/// Bare-Chinese caption of a shared sort key.
pub fn sort_key_label(key: ConnectionSortKey) -> &'static str {
    match key {
        ConnectionSortKey::DownloadDesc => "累计下载",
        ConnectionSortKey::UploadDesc => "累计上传",
        ConnectionSortKey::DownloadRateDesc => "瞬时下载",
        ConnectionSortKey::UploadRateDesc => "瞬时上传",
        ConnectionSortKey::LatestDesc => "最新优先",
        ConnectionSortKey::HostAsc => "主机名升序",
    }
}

/// DUAL-13-12: the clickable header pills that reorder the flat rows by the
/// shared key (instantaneous download / upload included).
pub fn sort_pills_scene(palette: &UiPalette) -> impl Scene + use<> {
    let pills: Vec<Box<dyn Scene>> = [
        ConnectionSortKey::DownloadDesc,
        ConnectionSortKey::DownloadRateDesc,
        ConnectionSortKey::UploadRateDesc,
        ConnectionSortKey::HostAsc,
    ]
    .into_iter()
    .map(|key| Box::new(sort_pill(key, palette)) as Box<dyn Scene>)
    .collect();

    bsn! {
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(space::S4),
        }
        Children [ { pills } ]
    }
}

fn sort_pill(key: ConnectionSortKey, palette: &UiPalette) -> impl Scene + use<> {
    let active = key == ConnectionSortKey::default();
    let (bg, text_color) = if active {
        (palette.accent_container, palette.accent)
    } else {
        (palette.surface, palette.ink_dim)
    };
    let label = sort_key_label(key);

    bsn! {
        Node {
            padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            align_items: AlignItems::Center,
        }
        BackgroundColor({ bg })
        ConnSortPill(key)
        Button
        Children [
            ( Text(label) TextRole(Role::Caption) TextColor({ text_color }) ),
        ]
    }
}

/// DUAL-13-12: switch the shared sort key, restamp the pills and reorder the
/// flat rows. The row entities keep their projection-index markers, so every
/// text restamp, the search visibility and the drawer keep pointing at the
/// same connection; only the render order changes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn on_connections_sort_activated(
    activate: On<Activate>,
    mut pills: Query<(&mut BackgroundColor, &ConnSortPill)>,
    mut containers: Query<&mut Children, With<ConnRowsContainer>>,
    rows: Query<&ConnectionRow>,
    row_subtrees: Query<&Children, Without<ConnRowsContainer>>,
    palette: Res<UiPalette>,
    mut view_state: Option<ResMut<ConnectionsViewState>>,
    last: Option<Res<LastConnectionsProjection>>,
) {
    let Ok((_, pill)) = pills.get(activate.entity) else {
        return;
    };
    let key = pill.0;
    if let Some(state) = view_state.as_deref_mut() {
        state.sort = key;
    }
    restamp_sort_pills(&palette, &mut pills, key);
    if let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) {
        apply_connection_row_order(projection, key, &mut containers, &rows, &row_subtrees);
    }
}

/// Restamp every sort pill fill for the active key.
pub(crate) fn restamp_sort_pills<F: bevy::ecs::query::QueryFilter>(
    palette: &UiPalette,
    pills: &mut Query<(&mut BackgroundColor, &ConnSortPill), F>,
    active: ConnectionSortKey,
) {
    for (mut fill, pill) in pills.iter_mut() {
        fill.0 = if pill.0 == active {
            palette.accent_container
        } else {
            palette.surface
        };
    }
}

/// DUAL-13-12: reorder the mounted flat rows through the one shared sort
/// reduction. Each row mounts as a card wrapper around its marked node, so the
/// wrapper's subtree is walked for the projection index. Unknown rows keep the
/// last rank, so a row the projection no longer carries cannot jump to the
/// front.
pub(crate) fn apply_connection_row_order(
    projection: &ConnectionsProjection,
    key: ConnectionSortKey,
    containers: &mut Query<&mut Children, With<ConnRowsContainer>>,
    rows: &Query<&ConnectionRow>,
    row_subtrees: &Query<&Children, Without<ConnRowsContainer>>,
) {
    let mut sorted = projection.connections.clone();
    connection_view::sort_connections(&mut sorted, key);
    let rank_of_id: HashMap<&str, usize> = sorted
        .iter()
        .enumerate()
        .map(|(rank, item)| (item.id.as_str(), rank))
        .collect();

    let rank = |entity: &Entity| {
        row_index_of(row_subtrees, rows, *entity)
            .and_then(|index| projection.connections.get(index))
            .and_then(|item| rank_of_id.get(item.id.as_str()).copied())
            .unwrap_or(usize::MAX)
    };
    for mut children in containers.iter_mut() {
        children.sort_by(|left, right| rank(left).cmp(&rank(right)));
    }
}

/// The projection row index of a mounted row, found by walking the row's own
/// subtree (the marked node sits inside the row's card wrapper).
fn row_index_of(
    row_subtrees: &Query<&Children, Without<ConnRowsContainer>>,
    rows: &Query<&ConnectionRow>,
    entity: Entity,
) -> Option<usize> {
    if let Ok(row) = rows.get(entity) {
        return Some(row.0);
    }
    let subtrees = row_subtrees.get(entity).ok()?;
    subtrees
        .iter()
        .find_map(|child| row_index_of(row_subtrees, rows, *child))
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

    fn view_upload_rate_bps(&self) -> f64 {
        self.upload_bps
    }

    fn view_download_rate_bps(&self) -> f64 {
        self.download_bps
    }

    fn view_chain(&self) -> &[String] {
        &self.chains
    }

    fn view_joined_chain(&self) -> &str {
        &self.chain
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
pub(crate) fn restamp_aggregation_pills<F: bevy::ecs::query::QueryFilter>(
    palette: &infiltrator_bevy_widgets::palette::UiPalette,
    pills: &mut Query<
        (
            &mut bevy::ui::prelude::BackgroundColor,
            &crate::pages::connections::ConnAggregationPill,
        ),
        F,
    >,
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
            rule_payload: String::new(),
            chain: "DIRECT".to_owned(),
            chains: vec!["DIRECT".to_owned()],
            network: "tcp".to_owned(),
            source_ip: "192.168.1.5".to_owned(),
            source_port: "50000".to_owned(),
            destination_ip: "1.1.1.1".to_owned(),
            destination_port: "443".to_owned(),
            upload_bps: 0.0,
            download_bps: 0.0,
            upload_total: up,
            download_total: down,
            destination_geo_ip: None,
            destination_ip_asn: String::new(),
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
            stream_phase: infiltrator_contract::connection::ConnectionStreamPhase::Live,
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
