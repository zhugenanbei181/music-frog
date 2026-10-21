//! Native query input and search presentation consume the shared cached decisions.
use crate::pages::connections::{
    CloseConnectionButton, ConnHostText, ConnPortText, ConnProcessTagText, ConnRuleText,
    LastConnectionsProjection,
};
use crate::pages::connections_rows::ConnectionRowText;
use crate::pages::connections_view::{
    CloseFilteredConnectionsButton, ConnAggregationSummaryContainer, ConnRowsContainer,
    ConnSearchField, ConnectionRow, ConnectionsViewState, search_field_text, split_runs_at_first,
    split_runs_at_last,
};
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::Insert;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::ui::prelude::{Display, Node};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_runs::{TextRuns, TextRunsSet};

#[derive(Component, Clone, Copy, Default)]
pub struct ConnectionSearchNode;
#[derive(Component, Clone, Copy, Default)]
#[require(ConnectionRowText)]
pub struct ConnectionMatchTerm(pub usize);
#[derive(Component, Clone, Copy, Default)]
#[require(ButtonDisabled)]
pub struct ClearConnectionsSearch;
#[derive(Component, Clone, Copy, Default)]
pub struct ConnectionsSearchSummary;
#[derive(Component, Clone, Copy, Default)]
#[require(ConnectionSearchNode)]
pub struct ConnectionsSearchEmpty;

/// Which text slot of a row the search replay fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SearchSlot {
    /// The row's host.
    Host,
    /// The row's destination port.
    Port,
    /// The row's process tag.
    Tag,
    /// The row's matched rule.
    Rule,
    /// The row's matched term.
    Matched,
}

#[derive(QueryData)]
#[query_data(mutable)]
struct SearchCopy {
    runs: &'static mut TextRuns,
    host: Option<&'static ConnHostText>,
    port: Option<&'static ConnPortText>,
    tag: Option<&'static ConnProcessTagText>,
    rule: Option<&'static ConnRuleText>,
    matched: Option<&'static ConnectionMatchTerm>,
}
pub struct ConnectionsSearchPlugin;
impl Plugin for ConnectionsSearchPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(initialize_query)
            .add_observer(clear_query)
            .add_systems(Update, replay_search.before(TextRunsSet));
    }
}
fn initialize_query(
    inserted: On<Insert<NativeTextField>>,
    view: Res<ConnectionsViewState>,
    mut fields: Query<(&NativeTextField, &mut TextField)>,
) {
    if let Ok((marker, mut field)) = fields.get_mut(inserted.entity)
        && marker.0 == 10
    {
        field
            .0
            .apply(TextFieldInput::SetText(view.page.query().into()));
    }
}
fn clear_query(
    activated: On<Activate>,
    buttons: Query<(), With<ClearConnectionsSearch>>,
    mut fields: Query<(&NativeTextField, &mut TextField)>,
) {
    if !buttons.contains(activated.entity) {
        return;
    }
    for (marker, mut field) in &mut fields {
        if marker.0 == 10 {
            field.0.apply(TextFieldInput::Clear);
        }
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
struct SearchNode {
    node: &'static mut Node,
    row: Option<&'static ConnectionRow>,
    flat: Option<&'static ConnRowsContainer>,
    grouped: Option<&'static ConnAggregationSummaryContainer>,
    empty: Option<&'static ConnectionsSearchEmpty>,
}
#[derive(QueryData)]
#[query_data(mutable)]
struct ActionGate {
    disabled: &'static mut ButtonDisabled,
    row: Option<&'static CloseConnectionButton>,
    filtered: Option<&'static CloseFilteredConnectionsButton>,
    clear: Option<&'static ClearConnectionsSearch>,
}
#[derive(SystemParam)]
struct SearchSurface<'w, 's> {
    view: ResMut<'w, ConnectionsViewState>,
    last: Res<'w, LastConnectionsProjection>,
    search: Query<'w, 's, &'static Children, With<ConnSearchField>>,
    fields: Query<'w, 's, &'static TextField>,
    nodes: Query<'w, 's, SearchNode, With<ConnectionSearchNode>>,
    copies: Query<'w, 's, SearchCopy, With<ConnectionRowText>>,
    summaries: Query<'w, 's, &'static mut LocalizedText, With<ConnectionsSearchSummary>>,
    actions: Query<'w, 's, ActionGate>,
}
fn replay_search(surface: SearchSurface) {
    let SearchSurface {
        mut view,
        last,
        search,
        fields,
        mut nodes,
        mut copies,
        mut summaries,
        mut actions,
    } = surface;
    if let Some(query) = search_field_text(&search, &fields) {
        view.page.edit_query(&query);
    }
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    let flat = view.page.mode().is_flat();
    for mut item in &mut nodes {
        let visible = if let Some(row) = item.row {
            projection
                .connections
                .get(row.0)
                .and_then(|row| view.page.search_row(&row.id))
                .is_some_and(|row| row.visible)
        } else if item.flat.is_some() {
            flat
        } else if item.grouped.is_some() {
            !flat
        } else if item.empty.is_some() {
            flat && view.page.summary_empty()
        } else {
            continue;
        };
        let display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if item.node.display != display {
            item.node.display = display;
        }
    }
    for mut copy in &mut copies {
        let (index, slot) = if let Some(host) = copy.host {
            (host.0, SearchSlot::Host)
        } else if let Some(port) = copy.port {
            (port.0, SearchSlot::Port)
        } else if let Some(tag) = copy.tag {
            (tag.0, SearchSlot::Tag)
        } else if let Some(rule) = copy.rule {
            (rule.0, SearchSlot::Rule)
        } else if let Some(matched) = copy.matched {
            (matched.0, SearchSlot::Matched)
        } else {
            continue;
        };
        let Some(row) = projection
            .connections
            .get(index)
            .and_then(|row| view.page.search_row(&row.id))
        else {
            continue;
        };
        let runs = match slot {
            // The endpoint is split into its host and its port, so the port
            // can carry its own ink; the highlight rides the matching half.
            SearchSlot::Host => split_runs_at_last(&row.endpoint, ":")
                .map(|(host, _)| host)
                .unwrap_or_else(|| row.endpoint.clone()),
            SearchSlot::Port => split_runs_at_last(&row.endpoint, ":")
                .map(|(_, port)| port)
                .unwrap_or_default(),
            SearchSlot::Tag => row.process_name.clone(),
            // The rule is the second half of "process · rule".
            SearchSlot::Rule => split_runs_at_first(&row.process_rule, " · ")
                .map(|(_, rule)| rule)
                .unwrap_or_default(),
            SearchSlot::Matched => row.matched_term.clone(),
        };
        if copy.runs.0 != runs {
            copy.runs.0 = runs;
        }
    }
    for mut action in &mut actions {
        if action.clear.is_some() {
            action.disabled.0 = view.page.query().trim().is_empty();
            continue;
        }
        if action.row.is_some() || action.filtered.is_some() {
            action.disabled.0 = !view.page.source_current()
                || (action.filtered.is_some() && view.page.query().trim().is_empty());
        }
    }
    let summary = LocalizedText::new(
        view.page.search_summary_key(),
        vec![
            ("matched", view.page.matched_count().to_string()),
            ("total", view.page.source_count().to_string()),
        ],
    );
    for mut copy in &mut summaries {
        if *copy != summary {
            *copy = summary.clone();
        }
    }
}
