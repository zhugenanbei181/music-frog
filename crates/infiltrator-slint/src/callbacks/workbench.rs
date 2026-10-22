//! Slint workbench callbacks routed to the shared product owner.
//!
//! Every gesture of a workbench page resolves through
//! `surface::action::workbench`, so the running surface and a driven surface
//! test observe exactly the same shared command and the same typed refusal.

use super::super::*;
use crate::surface::action::workbench::{self, WorkbenchGesture, WorkbenchOutcome};
use crate::surface::workbenches::Workbenches;
use infiltrator_contract::subscription_filter_form::FilterField;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

/// Registers the workbench callbacks on the real window.
pub(super) fn register(
    window: &AppWindow,
    requests: Sender<WorkerRequest>,
    latest: Arc<Mutex<Option<SurfaceSnapshot>>>,
    workbenches: Arc<Workbenches>,
) {
    let weak = window.as_weak();

    let weak_for_action = weak.clone();
    let latest_for_action = Arc::clone(&latest);
    let workbenches_for_action = Arc::clone(&workbenches);
    let tx_for_action = requests.clone();
    window.on_workbench_action(move |action, row| {
        let action = action.to_string();
        let row = row.to_string();
        let Some(window) = weak_for_action.upgrade() else {
            return;
        };
        let draft = window.get_workbench_draft().to_string();
        let draft2 = window.get_workbench_draft_2().to_string();
        let snapshot = latest_for_action
            .lock()
            .ok()
            .and_then(|guard| guard.clone());
        let gesture = WorkbenchGesture {
            action: &action,
            draft: &draft,
            draft2: &draft2,
            row: &row,
        };
        match workbench::dispatch(&workbenches_for_action, snapshot.as_ref(), &gesture) {
            Ok(WorkbenchOutcome::Command(owner_action)) => {
                // The reducer that owns the command correlates the owner's
                // terminal, so the surface records which one waits for it.
                // A command no shared reducer owns is recorded as well, so
                // the surface still waits for its terminal.
                if let SlintAction::Command(intent) = &owner_action {
                    workbenches_for_action.record(workbench::owner_business_for(&action, intent));
                }
                queue_action(&tx_for_action, &weak_for_action, Ok(owner_action));
                // The command is on its way to the owner, so the panel that
                // owns the business shows its pending phase right away. The
                // owner still owes its terminal, so the panel waits for it
                // instead of showing a state the owner never published.
                if let Some(snapshot) = snapshot.as_ref() {
                    lumen_projection::project_workbench(&window, snapshot, &workbenches_for_action);
                }
            }
            Ok(WorkbenchOutcome::Refresh) => {
                // The owner publishes its read models on its own cadence; a
                // refresh gesture asks for the next read immediately.
                let _ = tx_for_action.send(WorkerRequest::Read);
            }
            Ok(WorkbenchOutcome::Local) => {
                if let Some(snapshot) = snapshot.as_ref() {
                    lumen_projection::project_snapshot(&window, snapshot, None);
                    lumen_projection::project_workbench(&window, snapshot, &workbenches_for_action);
                }
            }
            Err(failure) => show_failure(&weak_for_action, failure),
        }
    });

    let weak_for_draft = weak.clone();
    let workbenches_for_draft = Arc::clone(&workbenches);
    window.on_workbench_draft_edited(move |id, value| {
        let id = id.to_string();
        let value = value.to_string();
        edit_draft(&workbenches_for_draft, &id, value.clone());
        // The window draft is the projection of the reducer, so the surface
        // keeps showing the value the shared reducer accepted.
        if let Some(window) = weak_for_draft.upgrade() {
            if window.get_workbench_draft_id().as_str() == id {
                window.set_workbench_draft(value.into());
            } else if window.get_workbench_draft_2_id().as_str() == id {
                window.set_workbench_draft_2(value.into());
            }
        }
    });
}

/// Routes one edited draft field into the shared reducer that owns it.
fn edit_draft(workbenches: &Workbenches, id: &str, value: String) {
    match id {
        "dns-query-name" => {
            if let Ok(mut query) = workbenches.dns_query.lock() {
                query.set_name(value);
            }
        }
        "script-code" => {
            if let Ok(mut script) = workbenches.script.lock() {
                script.edit_script(value);
            }
        }
        "filter-include" => {
            if let Ok(mut filter) = workbenches.filter.lock() {
                filter.edit(FilterField::Include, value);
            }
        }
        "dns-hosts-row" => {
            if let Ok(mut drafts) = workbenches.drafts.lock() {
                drafts.set_hosts_row(value);
            }
        }
        "aggregator-target" => {
            if let Ok(mut drafts) = workbenches.drafts.lock() {
                drafts.set_aggregation_target(value);
            }
        }
        "app-routing-rule" => {
            if let Ok(mut drafts) = workbenches.drafts.lock() {
                drafts.set_routing_rule(value);
            }
        }
        "command-query" => {
            if let Ok(mut drafts) = workbenches.drafts.lock() {
                drafts.set_command_query(value);
            }
        }
        _ => {}
    }
}
