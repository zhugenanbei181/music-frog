//! The App Routing page edits the per-application routing policy.
//!
//! The policy edit is a local surface decision in demo mode: the demo never
//! writes the host's application-routing store, so the page shows the rule
//! the user picked while the host keeps its own copy.
use crate::demo::fixtures::demo_app_routing_processes;
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_domain::app_routing::AppRoutingRule;

/// The process the demo edits: the first row of the demo's process grid.
pub(super) const DEMO_PROCESS: &str = "chrome";

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    // The demo grid is a fixture, so the policy the user edits is the policy
    // the page renders.
    if state.app_routing.processes.is_empty() {
        state.app_routing.processes = demo_app_routing_processes();
    }
    // The user picks the direct route for one application: the page's own
    // control writes the rule and the policy grid renders it.
    state.update(Message::SetAppRouteRule {
        process: DEMO_PROCESS.to_string(),
        rule: AppRoutingRule::Direct,
    })
}
