//! Reactive view state and shared reductions for the Connections page
//! (DUAL-13-02 / 13-07 / 13-12 / 13-13).
//!
//! The page scene lives in [`super::connections`]. This module owns the parts
//! that react to input: the aggregation segmented control, the instantaneous
//! rate sort pills, the keyword search field, range teardown, and the
//! two-step close-all confirmation. Every reduction delegates to
//! `infiltrator_domain::connection_view` so the Bevy page and the Iced page
//! compute the same buckets, order and matches.

use crate::pages::connections::{
    ConnAggregationPill, ConnectionsProjection, LastConnectionsProjection,
};
use crate::pages::connections_search::ConnectionSearchNode;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexWrap, JustifyContent, Node, UiRect, Val,
};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::connection_grouping::{ConnectionGroupingState, sort_label_key};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::surface_snapshot::ConnectionSnapshot;
use infiltrator_domain::connection_view;
use infiltrator_domain::connection_view::{ConnectionGroupingMode, ConnectionSortKey};
use std::collections::HashMap;

/// Marker on a single flat connection row root; the payload is the row index
/// into the last projection so search can hide non-matching rows in place.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ConnectionSearchNode)]
pub struct ConnectionRow(pub usize);

/// Marker on the container wrapping every flat connection row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ConnectionSearchNode)]
pub struct ConnRowsContainer;

/// Marker on the aggregation summary line (grouped modes only).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnAggregationSummary;

/// Marker on the aggregation summary container, toggled with the mode.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ConnectionSearchNode)]
pub struct ConnAggregationSummaryContainer;

/// Marker on the wrapper of the keyword search text field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnSearchField;

/// Marker on the "close every filtered connection" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ButtonDisabled)]
pub struct CloseFilteredConnectionsButton;

/// Marker on the close-all button's caption, restamped when the button arms.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloseAllConnectionsLabel;

/// Shared grouping selection, set by the aggregation pills.
#[derive(Resource, Clone, Debug, Default)]
pub struct ConnectionsViewState {
    pub groups: ConnectionGroupingState<ConnectionSnapshot>,
    /// DUAL-13-12: shared order of the flat rows; defaults to the same
    /// cumulative-download order the Iced surface starts with.
    pub sort: ConnectionSortKey,
}

/// DUAL-13-12: one sort pill on the connections table header. The payload is
/// the shared sort key, so both surfaces order through the same reduction.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnSortPill(pub ConnectionSortKey);

/// DUAL-13-12: the clickable header pills that reorder the flat rows by the
/// shared key (instantaneous download / upload included).
pub fn sort_pills_scene(palette: &UiPalette) -> impl Scene + use<> {
    let pills: Vec<Box<dyn Scene>> = [
        ConnectionSortKey::DownloadDesc,
        ConnectionSortKey::UploadDesc,
        ConnectionSortKey::DownloadRateDesc,
        ConnectionSortKey::UploadRateDesc,
        ConnectionSortKey::LatestDesc,
        ConnectionSortKey::HostAsc,
    ]
    .into_iter()
    .map(|key| Box::new(sort_pill(key, palette)) as Box<dyn Scene>)
    .collect();

    bsn! {
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                flex_wrap: FlexWrap::Wrap,
                row_gap: Val::Px(space::S4),
                column_gap: Val::Px(space::S4),
            }
            Children [
                { pills }
            ]
    }
}

fn sort_pill(key: ConnectionSortKey, palette: &UiPalette) -> impl Scene + use<> {
    let active = key == ConnectionSortKey::default();
    let (bg, text_color) = if active {
        (palette.accent_container, palette.accent)
    } else {
        (palette.surface, palette.ink_dim)
    };
    let label = LocalizedText::plain(sort_label_key(key));

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
                LocalizedText { .. { label } } TextRole(Role::Caption) TextColor({ text_color })
            ]
    }
}

/// DUAL-13-12: switch the shared sort key, restamp the pills and reorder the
/// flat rows. The row entities keep their projection-index markers, so every
/// text restamp, the search visibility and the drawer keep pointing at the
/// same connection; only the render order changes.
#[derive(SystemParam)]
pub(crate) struct ConnectionSortControls<'w, 's> {
    pills: Query<'w, 's, (&'static mut BackgroundColor, &'static ConnSortPill)>,
    containers: Query<'w, 's, &'static mut Children, With<ConnRowsContainer>>,
    rows: Query<'w, 's, &'static ConnectionRow>,
    row_subtrees: Query<'w, 's, &'static Children, Without<ConnRowsContainer>>,
    palette: Res<'w, UiPalette>,
    view_state: Option<ResMut<'w, ConnectionsViewState>>,
    last: Option<Res<'w, LastConnectionsProjection>>,
}
pub(crate) fn on_connections_sort_activated(
    activate: On<Activate>,
    controls: ConnectionSortControls,
) {
    let ConnectionSortControls {
        mut pills,
        mut containers,
        rows,
        row_subtrees,
        palette,
        mut view_state,
        last,
    } = controls;
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
pub(crate) fn restamp_sort_pills<F: QueryFilter>(
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

/// Pending navigation and visible confirmation state for close-all.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionsCloseAllState {
    pub armed: bool,
    pub requested: bool,
}

/// Read the live keyword from the mounted search field, if any.
pub(crate) fn search_field_text<F: QueryFilter>(
    search_fields: &Query<&Children, With<ConnSearchField>>,
    text_fields: &Query<&TextField, F>,
) -> Option<String> {
    search_fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text().to_owned())
}

/// Restamp every aggregation pill fill for the active mode.
pub(crate) fn restamp_aggregation_pills<F: QueryFilter>(
    palette: &UiPalette,
    pills: &mut Query<(&mut BackgroundColor, &ConnAggregationPill), F>,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use infiltrator_contract::connection::ConnectionStreamPhase;

    fn item(id: &str, host: &str, process: &str, up: u64, down: u64) -> ConnectionSnapshot {
        ConnectionSnapshot {
            start: String::new(),
            id: id.to_owned(),
            destination_host: host.to_owned(),
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
            rate_observed: true,
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
            stream_phase: ConnectionStreamPhase::Live,
            connections: vec![
                item("c1", "a.com:443", "/usr/bin/git", 10, 20),
                item("c2", "b.com:443", "/usr/bin/git", 20, 20),
            ],
        };
        let mut state = ConnectionGroupingState::default();
        state.observe(&projection.connections);
        state.select(ConnectionGroupingMode::ByProcess);
        let rows = state.rows();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].key.as_str(), rows[0].count), ("git", 2));
    }
}
