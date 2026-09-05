//! Iced host access to the shared per-app routing application.

use infiltrator_application::routing_application::RoutingApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};

pub async fn application() -> Result<RoutingApplication, InfiltratorError> {
    let store = infiltrator_desktop::storage::app_routing_store()
        .map_err(|error| InfiltratorError::Config(error.to_string()))?;
    Ok(RoutingApplication::new(std::sync::Arc::new(store)))
}

pub fn mode_to_domain(mode: crate::types::app_routing::AppRoutingMode) -> AppRoutingMode {
    match mode {
        crate::types::app_routing::AppRoutingMode::Global => AppRoutingMode::ProxyAll,
        crate::types::app_routing::AppRoutingMode::Whitelist => AppRoutingMode::ProxySelected,
        crate::types::app_routing::AppRoutingMode::Blacklist => AppRoutingMode::BypassSelected,
    }
}

pub fn mode_from_domain(mode: AppRoutingMode) -> crate::types::app_routing::AppRoutingMode {
    match mode {
        AppRoutingMode::ProxyAll => crate::types::app_routing::AppRoutingMode::Global,
        AppRoutingMode::ProxySelected => crate::types::app_routing::AppRoutingMode::Whitelist,
        AppRoutingMode::BypassSelected => crate::types::app_routing::AppRoutingMode::Blacklist,
    }
}

pub fn rule_to_domain(rule: crate::types::app_routing::AppRouteRule) -> AppRoutingRule {
    match rule {
        crate::types::app_routing::AppRouteRule::Proxy => AppRoutingRule::Proxy,
        crate::types::app_routing::AppRouteRule::Direct => AppRoutingRule::Direct,
        crate::types::app_routing::AppRouteRule::Block => AppRoutingRule::Block,
    }
}

pub fn rule_from_domain(rule: AppRoutingRule) -> crate::types::app_routing::AppRouteRule {
    match rule {
        AppRoutingRule::Proxy => crate::types::app_routing::AppRouteRule::Proxy,
        AppRoutingRule::Direct => crate::types::app_routing::AppRouteRule::Direct,
        AppRoutingRule::Block => crate::types::app_routing::AppRouteRule::Block,
    }
}
