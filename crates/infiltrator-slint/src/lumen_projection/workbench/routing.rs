//! The shared application-routing projection over the contract observation.
//!
//! The shared routing copy is authored over the domain vocabulary, while the
//! owner publishes the contract vocabulary. This module owns the one
//! conversion between the two so no surface re-implements the copy.

use infiltrator_contract::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_domain::app_routing;
use infiltrator_presentation::routing_projection;
use lurker_ui_presentation::copy::TextCatalog;

/// The shared routing summary of the routing policy the owner published.
///
/// The mode is optional because the owner may not have published one yet:
/// the summary then says so instead of naming a mode the owner never
/// reported.
pub(super) fn summary<M>(mode: Option<M>, count: usize, catalog: &dyn TextCatalog) -> String
where
    M: Into<AppRoutingMode> + Into<app_routing::AppRoutingMode>,
{
    routing_projection::routing_summary(mode.map(|mode| mode.into()), count, catalog)
}

/// The shared localized label of an observed application rule.
pub(super) fn rule_label<R>(rule: R) -> &'static str
where
    R: Into<AppRoutingRule> + Into<app_routing::AppRoutingRule>,
{
    routing_projection::rule_key(rule.into())
}
