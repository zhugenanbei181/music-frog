//! The shared application-routing projection over the contract observation.
//!
//! The shared routing copy is authored over the domain vocabulary, while the
//! owner publishes the contract vocabulary. This module owns the one
//! conversion between the two so no surface re-implements the copy.

use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_presentation::routing_projection;
use lurker_ui_presentation::copy::TextCatalog;

/// The shared routing summary of an observed policy.
pub(super) fn summary<M>(mode: M, count: usize, catalog: &dyn TextCatalog) -> String
where
    M: Into<AppRoutingMode>,
{
    routing_projection::routing_summary(mode.into(), count, catalog)
}

/// The shared localized label of an observed application rule.
pub(super) fn rule_label<R>(rule: R) -> &'static str
where
    R: Into<AppRoutingRule>,
{
    routing_projection::rule_key(rule.into())
}
