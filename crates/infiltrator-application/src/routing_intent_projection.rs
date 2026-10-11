//! Shared localized labels for routing intent state and change failures.

use crate::routing_intent_actions::RoutingIntentActions;
use infiltrator_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};
use infiltrator_domain::redact::redact_line;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn routing_intent_copy_key(intent: &RoutingIntent) -> &'static str {
    match intent {
        RoutingIntent::FollowRules => "routing_intent_follow_rules",
        RoutingIntent::RouteAllThroughProxy => "routing_intent_route_all_through_proxy",
        RoutingIntent::BypassProxy => "routing_intent_bypass_proxy",
        RoutingIntent::EngineDefined(_) => "routing_intent_engine_defined",
    }
}

pub fn routing_intent_copy(intent: &RoutingIntent, locale: &str) -> String {
    match intent {
        RoutingIntent::EngineDefined(extension) => interpolate(
            Lang(locale).tr(routing_intent_copy_key(intent)).as_ref(),
            &[("extension", extension.extension_id().as_str())],
        ),
        _ => Lang(locale)
            .tr(routing_intent_copy_key(intent))
            .into_owned(),
    }
}

pub fn routing_intent_failure_copy(actions: &RoutingIntentActions, locale: &str) -> String {
    let Some(failure) = &actions.failure else {
        return String::new();
    };
    let target = actions
        .requested
        .as_ref()
        .map(|intent| routing_intent_copy(intent, locale))
        .unwrap_or_else(|| Lang(locale).tr("shell_readout_unknown").into_owned());
    let reason = redact_line(&failure.message, &[]);
    interpolate(
        Lang(locale).tr("routing_intent_change_failed").as_ref(),
        &[("target", &target), ("reason", &reason)],
    )
}

pub fn routing_intent_status_copy(snapshot: &RoutingIntentSnapshot, locale: &str) -> String {
    let value = snapshot
        .current
        .as_ref()
        .map(|intent| routing_intent_copy(intent, locale));
    let key = match snapshot.status {
        RoutingIntentStatus::Ready => {
            return value.unwrap_or_else(|| Lang(locale).tr("shell_readout_unknown").into_owned());
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
    let value = value.unwrap_or_else(|| Lang(locale).tr("shell_readout_unknown").into_owned());
    interpolate(Lang(locale).tr(key).as_ref(), &[("value", &value)])
}

#[cfg(test)]
#[path = "routing_intent_projection_tests.rs"]
mod tests;
