//! Shared localized labels for routing intent state and change failures.

use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_domain::redact::redact_line;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};
use lurker_ui_presentation::copy::TextCatalog;
use lurker_ui_presentation::copy::interpolate;
use lurker_ui_presentation::routing_intent_actions::RoutingIntentActions;

pub fn from_surface(snapshot: &SurfaceSnapshot) -> RoutingIntentSnapshot {
    match snapshot.core.lifecycle {
        CoreLifecycle::Starting | CoreLifecycle::Stopping => RoutingIntentSnapshot {
            current: snapshot.core.routing_intent.current.clone(),
            status: RoutingIntentStatus::Pending,
            failure: snapshot.core.routing_intent.failure.clone(),
        },
        CoreLifecycle::Stopped => {
            let mut routing_intent = snapshot.core.routing_intent.clone();
            if routing_intent.status == RoutingIntentStatus::Ready {
                routing_intent.status = RoutingIntentStatus::Unobserved;
            }
            routing_intent
        }
        CoreLifecycle::Failed => RoutingIntentSnapshot {
            current: snapshot.core.routing_intent.current.clone(),
            status: RoutingIntentStatus::Failed,
            failure: snapshot
                .core
                .routing_intent
                .failure
                .clone()
                .or_else(|| snapshot.core.failure.clone()),
        },
        CoreLifecycle::Ready | CoreLifecycle::Running => snapshot.core.routing_intent.clone(),
    }
}

pub fn with_verified_intent(snapshot: &SurfaceSnapshot, actual: RoutingIntent) -> SurfaceSnapshot {
    let mut snapshot = snapshot.clone();
    snapshot.revision = snapshot
        .revision
        .checked_add(1)
        .expect("surface revision exhausted");
    snapshot.core.routing_intent = RoutingIntentSnapshot {
        current: Some(actual),
        status: RoutingIntentStatus::Ready,
        failure: None,
    };
    if let Some(page) = &mut snapshot.pages.overview.data {
        page.routing_intent = snapshot.core.routing_intent.clone();
    }
    snapshot
}

pub fn routing_intent_copy_key(intent: &RoutingIntent) -> &'static str {
    match intent {
        RoutingIntent::FollowRules => "routing_intent_follow_rules",
        RoutingIntent::RouteAllThroughProxy => "routing_intent_route_all_through_proxy",
        RoutingIntent::BypassProxy => "routing_intent_bypass_proxy",
        RoutingIntent::EngineDefined(_) => "routing_intent_engine_defined",
    }
}

pub fn routing_intent_copy(intent: &RoutingIntent, catalog: &dyn TextCatalog) -> String {
    match intent {
        RoutingIntent::EngineDefined(extension) => interpolate(
            catalog.text(routing_intent_copy_key(intent)).as_ref(),
            &[("extension", extension.extension_id().as_str())],
        ),
        _ => catalog.text(routing_intent_copy_key(intent)).into_owned(),
    }
}

pub fn routing_intent_failure_copy(
    actions: &RoutingIntentActions,
    catalog: &dyn TextCatalog,
) -> String {
    let Some(failure) = actions.failure() else {
        return String::new();
    };
    let target = actions
        .requested_target()
        .map(|intent| routing_intent_copy(intent, catalog))
        .unwrap_or_else(|| catalog.text("shell_readout_unknown").into_owned());
    let reason = redact_line(&failure.message, &[]);
    interpolate(
        catalog.text("routing_intent_change_failed").as_ref(),
        &[("target", &target), ("reason", &reason)],
    )
}

pub fn routing_intent_status_copy(
    snapshot: &RoutingIntentSnapshot,
    catalog: &dyn TextCatalog,
) -> String {
    let value = snapshot
        .current
        .as_ref()
        .map(|intent| routing_intent_copy(intent, catalog));
    let key = match snapshot.status {
        RoutingIntentStatus::Ready => {
            return value.unwrap_or_else(|| catalog.text("shell_readout_unknown").into_owned());
        }
        RoutingIntentStatus::Pending => "routing_intent_pending",
        RoutingIntentStatus::Unobserved
        | RoutingIntentStatus::Failed
        | RoutingIntentStatus::Unsupported => {
            if value.is_some() {
                "shell_readout_stale"
            } else {
                "shell_readout_unknown"
            }
        }
    };
    let value = value.unwrap_or_else(|| catalog.text("shell_readout_unknown").into_owned());
    interpolate(catalog.text(key).as_ref(), &[("value", &value)])
}

#[cfg(test)]
#[path = "routing_intent_projection_tests.rs"]
mod tests;
