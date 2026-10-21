//! Reactive view state, the OFFICIAL segmented strips and the shared
//! reductions for the Connections page (DUAL-13-02 / 13-07 / 13-12 / 13-13).
//!
//! The page scene lives in [`super::connections`]. This module owns the parts
//! that react to input: the three segmented strips (the view filter, the row
//! order and the idle timeout), the keyword search field, the range teardown
//! and the two-step close-all confirmation. Every reduction delegates to
//! `infiltrator_domain::connection_view` so the Bevy page and the Iced page
//! compute the same buckets, order and matches.
//!
//! The strips are the OFFICIAL headless tab strips
//! (`bevy_ui_widgets::TabList` / `Tab` / `SelectedTab`): the selection, the
//! roving tabindex, the arrow-key navigation and the a11y roles belong to
//! Bevy. This module mirrors the official selection into the shared page
//! model and paints the chips from the official selection state, so no
//! page-local selection state machine exists.

use crate::pages::connections::{ConnectionsProjection, LastConnectionsProjection};
use crate::pages::connections_idle::ConnectionsIdleState;
use crate::pages::connections_search::ConnectionSearchNode;
use crate::pages::connections_virtual::ConnectionsVirtualNodes;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderColor, BorderRadius, Display, FlexDirection, FlexWrap, JustifyContent, Node,
    UiRect, Val, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{ControlOrientation, SelectedTab, Tab, TabActivation, TabList, ValueChange};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::surface_snapshot::ConnectionSnapshot;
use infiltrator_domain::connection_activity::{IDLE_TIMEOUT_CHOICES, idle_timeout_minutes_label};
use infiltrator_domain::connection_view;
use infiltrator_domain::connection_view::{ConnectionGroupingMode, ConnectionSortKey};
use infiltrator_presentation::connection_grouping::{grouping_label_key, sort_label_key};
use infiltrator_presentation::connection_page_model::{
    CONNECTION_SORT_OPTIONS, ConnectionsPageModel,
};
use std::collections::HashMap;
use zhuge_ui_core::text::SearchTextRun;

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

/// The one page model of the connections page.
///
/// The grouped rows, the query, the match counts and the sort selection are
/// owned by the shared model, so this surface only renders the model and
/// dispatches typed intents back to it.
#[derive(Resource, Clone, Debug, Default)]
pub struct ConnectionsViewState {
    pub page: ConnectionsPageModel<ConnectionSnapshot>,
}

/// Which segmented strip a tab belongs to.
///
/// The three strips are the page's three exclusive segmented choices: the
/// view filter, the row order and the idle timeout. The kind is explicit, so
/// one observer can route a selection to the right reduction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnStripKind {
    /// The view filter: flat / by process / by host.
    #[default]
    ViewFilter,
    /// The row order of the flat rows.
    Sort,
    /// The idle-connection timeout.
    IdleTimeout,
}

/// One segmented choice of the Connections page.
///
/// The payload is the shared choice itself (the grouping mode, the sort key
/// or the timeout in seconds), so a control position means the same thing on
/// every surface and the strip never carries a second copy of the choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnChoice {
    /// The view filter choice.
    ViewFilter(ConnectionGroupingMode),
    /// The row-order choice.
    Sort(ConnectionSortKey),
    /// The idle-timeout choice.
    IdleTimeout(u64),
}

impl Default for ConnChoice {
    fn default() -> Self {
        Self::ViewFilter(ConnectionGroupingMode::Flat)
    }
}

/// Marker on a segmented strip root (the official `TabList` entity).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnStrip(pub ConnStripKind);

/// Marker on one tab of a segmented strip (the official `Tab` entity).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnStripTab(pub ConnChoice);

/// Marker on a strip tab's label text; the label ink follows the strip's
/// official selection.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnStripTabLabel;

/// One strip tab's label: a localized catalog key or a plain literal.
pub enum ConnStripLabel {
    /// A localized catalog key.
    Key(&'static str),
    /// A plain literal label (already formatted).
    Plain(String),
}

/// One segmented strip over the OFFICIAL headless tab list.
///
/// The strip root IS the `bevy_ui_widgets::TabList`, and every tab is one of
/// its direct children — the official click and keyboard handling requires
/// exactly that shape. The builder supplies the token-styled geometry and the
/// labels; the selection itself is the official `SelectedTab`.
pub fn strip_scene(
    kind: ConnStripKind,
    tabs: Vec<(ConnStripLabel, ConnChoice)>,
    active: ConnChoice,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let scenes: Vec<Box<dyn Scene>> = tabs
        .into_iter()
        .map(|(label, choice)| strip_tab_scene(label, choice, choice == active, palette))
        .collect();

    bsn! {
            Node {
                width: Val::Auto,
                max_width: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: rem(space::S4),
                row_gap: rem(space::S4),
                padding: UiRect::all(rem(space::S4)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            BorderColor { top: { palette.border }, right: { palette.border }, bottom: { palette.border }, left: { palette.border } }
            TabList {
                orientation: ControlOrientation::Horizontal,
                activation: TabActivation::Automatic,
            }
            ConnStrip({ kind })
            Children [
                { scenes }
            ]
    }
}

/// One tab of a segmented strip: an official `Tab` carrying its typed choice.
fn strip_tab_scene(
    label: ConnStripLabel,
    choice: ConnChoice,
    selected: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let fill = if selected {
        Token::AccentContainer
    } else {
        Token::SurfaceElevated
    };
    let ink = if selected {
        Token::Accent
    } else {
        Token::InkDim
    };
    let label_scene: Box<dyn Scene> = match label {
        ConnStripLabel::Key(key) => Box::new(bsn! {
            LocalizedText::plain(key) TextRole(Role::Caption) ThemeTextColor({ ink }) ConnStripTabLabel
        }),
        ConnStripLabel::Plain(text) => Box::new(bsn! {
            Text({ text }) TextRole(Role::Caption) ThemeTextColor({ ink }) ConnStripTabLabel
        }),
    };

    Box::new(bsn! {
            Node {
                min_height: rem(palette.control_height_px * 0.8),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(rem(space::S12)),
                border_radius: BorderRadius::MAX,
            }
            ThemeBackgroundColor({ fill })
            Tab
            ConnStripTab({ choice })
            Children [
                @{ label_scene }
            ]
    })
}

/// The view filter strip: the shared grouping modes as official tabs.
pub fn view_filter_strip_scene(
    active: ConnectionGroupingMode,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let tabs = ConnectionGroupingMode::ALL
        .into_iter()
        .map(|mode| {
            (
                ConnStripLabel::Key(grouping_label_key(mode)),
                ConnChoice::ViewFilter(mode),
            )
        })
        .collect();
    strip_scene(
        ConnStripKind::ViewFilter,
        tabs,
        ConnChoice::ViewFilter(active),
        palette,
    )
}

/// The row-order strip: the shared ordered sort choices as official tabs.
pub fn sort_strip_scene(active: ConnectionSortKey, palette: &UiPalette) -> impl Scene + use<> {
    let tabs = CONNECTION_SORT_OPTIONS
        .into_iter()
        .map(|key| {
            (
                ConnStripLabel::Key(sort_label_key(key)),
                ConnChoice::Sort(key),
            )
        })
        .collect();
    strip_scene(ConnStripKind::Sort, tabs, ConnChoice::Sort(active), palette)
}

/// The idle-timeout strip: the shared timeout choices as official tabs.
pub fn idle_timeout_strip_scene(active: u64, palette: &UiPalette) -> impl Scene + use<> {
    let tabs = IDLE_TIMEOUT_CHOICES
        .into_iter()
        .map(|secs| {
            (
                ConnStripLabel::Plain(idle_timeout_minutes_label(secs)),
                ConnChoice::IdleTimeout(secs),
            )
        })
        .collect();
    strip_scene(
        ConnStripKind::IdleTimeout,
        tabs,
        ConnChoice::IdleTimeout(active),
        palette,
    )
}

/// The choice a strip currently holds, read from the shared owners.
fn active_choice(
    kind: ConnStripKind,
    view: Option<&ConnectionsViewState>,
    idle: Option<&ConnectionsIdleState>,
) -> Option<ConnChoice> {
    match kind {
        ConnStripKind::ViewFilter => Some(ConnChoice::ViewFilter(view?.page.mode())),
        ConnStripKind::Sort => Some(ConnChoice::Sort(view?.page.sort_key())),
        ConnStripKind::IdleTimeout => Some(ConnChoice::IdleTimeout(idle?.timeout_secs)),
    }
}

/// Seed each strip's OFFICIAL selection from the shared owner.
///
/// `SelectedTab` is the strip's controlled selection; the seed makes it agree
/// with the shared model from the first frame, so the official derived state
/// and the painted chip never disagree.
pub(crate) fn seed_connections_strips(
    mut commands: Commands,
    strips: Query<(Entity, &ConnStrip, &Children, &SelectedTab), With<ConnStrip>>,
    tabs: Query<&ConnStripTab, With<Tab>>,
    view: Option<Res<ConnectionsViewState>>,
    idle: Option<Res<ConnectionsIdleState>>,
) {
    for (entity, strip, children, selected) in &strips {
        if selected.0.is_some() {
            continue;
        }
        let Some(choice) = active_choice(strip.0, view.as_deref(), idle.as_deref()) else {
            continue;
        };
        let Some(tab) = children
            .iter()
            .copied()
            .find(|child| tabs.get(*child).is_ok_and(|tab| tab.0 == choice))
        else {
            continue;
        };
        commands.entity(entity).insert(SelectedTab(Some(tab)));
    }
}

/// Paint the strips from the OFFICIAL selection state.
///
/// The chip fill and the label ink follow the strip's `SelectedTab`, so the
/// official selection is the only source of the segmented control's look.
pub(crate) fn sync_connections_strips(
    mut commands: Commands,
    strips: Query<(&Children, &SelectedTab), With<ConnStrip>>,
    tabs: Query<(&ConnStripTab, &Children), With<Tab>>,
    labels: Query<Entity, With<ConnStripTabLabel>>,
) {
    for (children, selected) in &strips {
        let Some(selected_entity) = selected.0 else {
            continue;
        };
        for child in children.iter() {
            let Ok((_, tab_children)) = tabs.get(*child) else {
                continue;
            };
            let is_selected = *child == selected_entity;
            commands
                .entity(*child)
                .insert(ThemeBackgroundColor(if is_selected {
                    Token::AccentContainer.theme_token()
                } else {
                    Token::SurfaceElevated.theme_token()
                }));
            for label in tab_children.iter() {
                if labels.contains(*label) {
                    commands
                        .entity(*label)
                        .insert(ThemeTextColor(if is_selected {
                            Token::Accent.theme_token()
                        } else {
                            Token::InkDim.theme_token()
                        }));
                }
            }
        }
    }
}

/// The OFFICIAL strip selection changed: apply the choice to the shared
/// owner and mirror it back into the official controlled selection.
///
/// The shared model stays the only place a choice lives: the official
/// `SelectedTab` mirrors it, never the other way round.
pub(crate) fn on_connections_strip_changed(
    change: On<ValueChange<Option<Entity>>>,
    mut commands: Commands,
    mut view: Option<ResMut<ConnectionsViewState>>,
    mut idle: Option<ResMut<ConnectionsIdleState>>,
    strips: Query<&ConnStrip>,
    tabs: Query<&ConnStripTab>,
    last: Option<Res<LastConnectionsProjection>>,
    mut containers: Query<&mut Children, With<ConnRowsContainer>>,
    rows: Query<&ConnectionRow>,
    row_subtrees: Query<&Children, Without<ConnRowsContainer>>,
    mut summary_containers: Query<
        &mut Node,
        (
            With<ConnAggregationSummaryContainer>,
            Without<ConnRowsContainer>,
        ),
    >,
    mut rows_containers: Query<
        &mut Node,
        (
            With<ConnRowsContainer>,
            Without<ConnAggregationSummaryContainer>,
        ),
    >,
) {
    let Ok(_strip) = strips.get(change.source) else {
        return;
    };
    let Some(selected) = change.value else {
        return;
    };
    let Ok(tab) = tabs.get(selected) else {
        return;
    };
    // The official controlled selection mirrors the shared choice.
    commands
        .entity(change.source)
        .insert(SelectedTab(Some(selected)));
    match tab.0 {
        ConnChoice::ViewFilter(mode) => {
            if let Some(state) = view.as_deref_mut() {
                state.page.select(mode);
            }
            // The grouped summary and the flat rows are exclusive views.
            let flat = mode.is_flat();
            for mut node in &mut summary_containers {
                node.display = if flat { Display::None } else { Display::Flex };
            }
            for mut node in &mut rows_containers {
                node.display = if flat { Display::Flex } else { Display::None };
            }
        }
        ConnChoice::Sort(key) => {
            if let Some(state) = view.as_deref_mut() {
                state.page.select_sort(key);
            }
            // BANDROID-010: the recycler re-derives the sorted window from the
            // shared sort key, so the full-mount child reorder must not touch
            // its slots.
            if let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) {
                apply_connection_row_order(projection, key, &mut containers, &rows, &row_subtrees);
            }
        }
        ConnChoice::IdleTimeout(secs) => {
            if let Some(state) = idle.as_deref_mut() {
                state.timeout_secs = secs;
            }
        }
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

/// The close-all confirmation is one typed stage table.
///
/// The destructive action always passes through a visible modal: a request
/// waits for the connections route, an armed modal waits for the user's
/// decision, and the idle stage has nothing pending. The stages are
/// exclusive, so a confirmation that is already open can never also be
/// waiting for a route.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnectionsCloseAllState {
    /// Nothing is pending; the confirmation modal is closed.
    #[default]
    Idle,
    /// A confirmation was requested and waits for the connections route.
    Requested,
    /// The confirmation modal is open and waits for the user's decision.
    Armed,
}

/// Why one close-all stage transition was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseAllRefusal {
    /// The confirmation modal is already open.
    AlreadyArmed,
    /// A confirmation request is already waiting.
    AlreadyRequested,
    /// No confirmation is pending.
    NotPending,
}

/// The outcome of one close-all stage transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseAllTransition {
    /// The stage changed; the payload is the stage that is now running.
    Applied(ConnectionsCloseAllState),
    /// The transition is not legal in the running stage; the payload is that
    /// stage together with the reason it was refused.
    Refused(ConnectionsCloseAllState, CloseAllRefusal),
}

impl ConnectionsCloseAllState {
    /// Whether the confirmation modal is open.
    pub const fn is_armed(self) -> bool {
        matches!(self, Self::Armed)
    }

    /// Whether a confirmation request waits for the connections route.
    pub const fn is_requested(self) -> bool {
        matches!(self, Self::Requested)
    }

    /// Raise a request to show the confirmation modal.
    ///
    /// A request that arrives while the modal is open is refused: the open
    /// modal is the stage that is already running, and a second pending
    /// request could never be honoured.
    pub fn request(&mut self) -> CloseAllTransition {
        match self {
            Self::Idle => {
                *self = Self::Requested;
                CloseAllTransition::Applied(*self)
            }
            Self::Requested => {
                CloseAllTransition::Refused(*self, CloseAllRefusal::AlreadyRequested)
            }
            Self::Armed => CloseAllTransition::Refused(*self, CloseAllRefusal::AlreadyArmed),
        }
    }

    /// Open the confirmation modal.
    ///
    /// The modal opens straight from a click and from a pending request; an
    /// already open modal keeps its stage.
    pub fn arm(&mut self) -> CloseAllTransition {
        match self {
            Self::Idle | Self::Requested => {
                *self = Self::Armed;
                CloseAllTransition::Applied(*self)
            }
            Self::Armed => CloseAllTransition::Refused(*self, CloseAllRefusal::AlreadyArmed),
        }
    }

    /// Close the confirmation modal.
    ///
    /// Only an armed modal can be closed; nothing else is pending.
    pub fn disarm(&mut self) -> CloseAllTransition {
        match self {
            Self::Armed => {
                *self = Self::Idle;
                CloseAllTransition::Applied(*self)
            }
            Self::Idle | Self::Requested => {
                CloseAllTransition::Refused(*self, CloseAllRefusal::NotPending)
            }
        }
    }
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

/// Split a highlighted run block at the last occurrence of `separator`.
///
/// The separator itself is dropped from the right part. The runs keep their
/// highlight flags, so a search match inside either half still lights up.
pub fn split_runs_at_last(
    runs: &[SearchTextRun],
    separator: &str,
) -> Option<(Vec<SearchTextRun>, Vec<SearchTextRun>)> {
    let text: String = runs.iter().map(|run| run.text.as_str()).collect();
    let cut = text.rfind(separator)?;
    Some(split_runs_at(runs, cut, cut + separator.len()))
}

/// Split a highlighted run block at the first occurrence of `separator`.
///
/// The separator itself is dropped from the right part, exactly like the last
/// occurrence split: the right half never repeats what the left half dropped.
pub fn split_runs_at_first(
    runs: &[SearchTextRun],
    separator: &str,
) -> Option<(Vec<SearchTextRun>, Vec<SearchTextRun>)> {
    let text: String = runs.iter().map(|run| run.text.as_str()).collect();
    let cut = text.find(separator)?;
    Some(split_runs_at(runs, cut, cut + separator.len()))
}

/// The shared splitter: the left part owns `[0, left_end)` and the right part
/// owns `[right_start, end)`. The separator between them belongs to neither
/// half, so the right half never repeats the separator the left half already
/// dropped. A run that straddles the cut is split in two and keeps its
/// highlight flag; a run that covers only the separator is dropped.
fn split_runs_at(
    runs: &[SearchTextRun],
    left_end: usize,
    right_start: usize,
) -> (Vec<SearchTextRun>, Vec<SearchTextRun>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut offset = 0usize;
    for run in runs {
        let start = offset;
        let end = offset + run.text.len();
        offset = end;
        if end <= left_end {
            left.push(run.clone());
            continue;
        }
        if start >= right_start {
            right.push(run.clone());
            continue;
        }
        // The run overlaps the cut: its head belongs to the left half and its
        // tail belongs to the right half, and the separator between the two
        // cut points belongs to neither.
        if start < left_end {
            let head = left_end - start;
            left.push(SearchTextRun {
                text: run.text[..head].to_owned(),
                highlighted: run.highlighted,
            });
        }
        if end > right_start {
            let tail = right_start - start;
            right.push(SearchTextRun {
                text: run.text[tail..].to_owned(),
                highlighted: run.highlighted,
            });
        }
    }
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::connection::ConnectionRateObservation;
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
            rate_observation: ConnectionRateObservation::Measured,
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
        let mut state = ConnectionsPageModel::default();
        state.observe(&projection.connections);
        state.select(ConnectionGroupingMode::ByProcess);
        let rows = state.rows();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].key.as_str(), rows[0].count), ("git", 2));
    }

    #[test]
    fn endpoint_runs_split_into_host_and_port() {
        let runs = vec![SearchTextRun {
            text: "api.github.com:443".to_owned(),
            highlighted: false,
        }];
        let (host, port) = split_runs_at_last(&runs, ":").expect("a port separator");
        assert_eq!(host[0].text, "api.github.com");
        assert_eq!(port[0].text, "443");
    }

    #[test]
    fn endpoint_runs_keep_the_highlight_on_the_matching_half() {
        let runs = vec![
            SearchTextRun {
                text: "api.".to_owned(),
                highlighted: false,
            },
            SearchTextRun {
                text: "github".to_owned(),
                highlighted: true,
            },
            SearchTextRun {
                text: ".com:443".to_owned(),
                highlighted: false,
            },
        ];
        let (host, port) = split_runs_at_last(&runs, ":").expect("a port separator");
        assert_eq!(host.len(), 3);
        assert!(host[1].highlighted);
        assert_eq!(port[0].text, "443");
        assert!(!port[0].highlighted);
    }

    #[test]
    fn process_runs_split_into_process_and_rule() {
        let runs = vec![SearchTextRun {
            text: "chrome · GEOSITE youtube".to_owned(),
            highlighted: false,
        }];
        let (process, rule) = split_runs_at_first(&runs, " · ").expect("a rule separator");
        assert_eq!(process[0].text, "chrome");
        assert_eq!(rule[0].text, "GEOSITE youtube");
    }

    #[test]
    fn a_split_never_repeats_the_separator_in_the_right_half() {
        let runs = vec![SearchTextRun {
            text: "chrome · GEOSITE youtube".to_owned(),
            highlighted: false,
        }];
        let (process, rule) = split_runs_at_first(&runs, " · ").expect("a rule separator");
        assert!(!process[0].text.contains("·"));
        assert!(!rule[0].text.contains("·"));
        assert_eq!(rule[0].text, "GEOSITE youtube");
    }

    #[test]
    fn an_endpoint_without_a_port_stays_one_half() {
        let runs = vec![SearchTextRun {
            text: "api-0".to_owned(),
            highlighted: false,
        }];
        assert!(split_runs_at_last(&runs, ":").is_none());
    }
}
