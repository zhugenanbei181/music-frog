//! The Overview page performs its own topology drill-down and owns the
//! reconnect mask the crash watchdog publishes.
use crate::demo::owner::attach_capture_owner;
use crate::demo::watchdog::attach_watchdog_capture_owner;
use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::view::overview_topology::topology_route_for_stage;
use iced::Task;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::overview_layout_application::OverviewLayoutApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::traffic_topology::TrafficTopologyStage;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use std::sync::Arc;

/// The filter query the topology drill-down carries into its target page.
///
/// The RuleSet stage of the demo observation reports this detail, so the
/// drill-down lands on the rules page already narrowed to that stage.
pub(super) const DRILLDOWN_FILTER: &str = "MRS / GeoIP";

pub(super) fn activate(state: &mut AppState, feature: FeatureId) -> Task<Message> {
    match feature {
        FeatureId::OverviewCardReorder => {
            // The user moves one Overview card one slot up: the shared layout
            // order changes and the page assembles its cards from that order.
            // The move is the owner's business, so the demo gives the
            // surface an owner that answers the shared intent.
            attach_overview_layout_owner(state);
            state.update(Message::MoveOverviewCardUp(OverviewCardKind::Traffic))
        }
        FeatureId::OverviewTopologyDrilldown => activate_topology_drilldown(state),
        FeatureId::OverviewReconnectMask => attach_watchdog_capture_owner(state),
        _ => Task::none(),
    }
}

/// Attach the owner that holds the overview card order.
///
/// The surface dispatches the shared reorder intent and renders the order the
/// owner publishes, so the demo needs a real owner - not a local copy of the
/// order.
fn attach_overview_layout_owner(state: &mut AppState) {
    let core = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().expect("isolated capture runtime"),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_overview_layout(OverviewLayoutApplication::new()),
    ));
    attach_capture_owner(state, core);
}

/// The user activates the RuleSet stage of the topology chain.
///
/// The stage control resolves its destination through the shared navigation
/// target, and the drill-down carries the stage's own detail into the target
/// page as its filter query, exactly like the peer surface does.
fn activate_topology_drilldown(state: &mut AppState) -> Task<Message> {
    let stage = TrafficTopologyStage::RuleSet;
    let route = topology_route_for_stage(stage).unwrap_or(Route::Rules);
    let detail = state
        .runtime
        .traffic_topology
        .node(stage)
        .map(|node| node.detail.clone())
        .filter(|detail| !detail.is_empty())
        .unwrap_or_else(|| DRILLDOWN_FILTER.to_string());
    let navigation = state.update(Message::Navigate(route));
    navigation.chain(state.update(Message::FilterRules(detail)))
}
