//! Typed, CoreScope-fenced sing-box routing intent observation.

use crate::api::clash_api::{SingBoxClashApiClient, SingBoxCoreVersionProvider};
use crate::api::scope_fence::{engine_id, rejected, verify_live_scope, verify_owner_scope};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::routing_intent::{
    EngineDefinedRoutingIntent, RoutingIntent, RoutingIntentExtensionId,
};
use lurker_core_ports::core_scope::CoreScopeSource;
use lurker_core_ports::error::PortError;
use lurker_core_ports::routing_intent::RoutingIntentPort;
use serde::Deserialize;
use std::sync::Arc;

/// The observed sing-box `/configs.mode` value remains engine-defined. sing-box
/// uses it to select rule `clash_mode` branches; it does not mean one of the
/// neutral routing policies. Writes remain disabled until their behavior is
/// verified against a real enabled `clash_mode` configuration.
pub struct SingBoxRoutingIntentAdapter {
    api: SingBoxClashApiClient,
    scope_source: Arc<dyn CoreScopeSource>,
    version_provider: SingBoxCoreVersionProvider,
}

impl SingBoxRoutingIntentAdapter {
    /// Creates the routing-intent adapter over one API client and scope source.
    pub fn new(api: SingBoxClashApiClient, scope_source: Arc<dyn CoreScopeSource>) -> Self {
        Self {
            version_provider: SingBoxCoreVersionProvider::new(api.clone()),
            api,
            scope_source,
        }
    }
}

#[async_trait::async_trait]
impl RoutingIntentPort for SingBoxRoutingIntentAdapter {
    async fn observe(&self, scope: &CoreScope) -> Result<RoutingIntent, PortError> {
        verify_live_scope(&self.version_provider, self.scope_source.as_ref(), scope).await?;
        let config: ClashConfigResponse = self.api.get_json(&["configs"]).await?;
        verify_live_scope(&self.version_provider, self.scope_source.as_ref(), scope).await?;
        routing_intent_from_mode(&config)
    }

    async fn apply(
        &self,
        scope: &CoreScope,
        intent: RoutingIntent,
    ) -> Result<RoutingIntent, PortError> {
        verify_owner_scope(self.scope_source.as_ref(), scope)?;
        match intent {
            RoutingIntent::EngineDefined(extension) if extension.engine() != &engine_id() => {
                Err(rejected(
                    ErrorCode::InvalidState,
                    "engine-defined routing intent belongs to another engine",
                ))
            }
            RoutingIntent::FollowRules
            | RoutingIntent::RouteAllThroughProxy
            | RoutingIntent::BypassProxy
            | RoutingIntent::EngineDefined(_) => Err(PortError::unsupported(
                Capability::RoutingIntent,
                "sing-box /configs.mode selects clash_mode rule state and has no verified write mapping to the requested routing intent",
            )),
        }
    }
}

#[derive(Deserialize)]
struct ClashConfigResponse {
    mode: String,
    #[serde(rename = "mode-list", default)]
    mode_list: Vec<String>,
}

fn routing_intent_from_mode(config: &ClashConfigResponse) -> Result<RoutingIntent, PortError> {
    if config.mode.trim().is_empty() || !config.mode_list.iter().any(|mode| mode == &config.mode) {
        return Err(rejected(
            ErrorCode::InvalidState,
            "sing-box /configs returned a mode outside its reported mode-list",
        ));
    }
    let canonical = config.mode.to_ascii_lowercase();
    if !canonical
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte))
    {
        return Err(rejected(
            ErrorCode::InvalidState,
            "sing-box /configs.mode cannot be represented by the engine-defined routing ID",
        ));
    }
    let extension_id = RoutingIntentExtensionId::new(format!("mode.{canonical}"))
        .map_err(|error| rejected(ErrorCode::InvalidState, error.to_string()))?;
    Ok(RoutingIntent::EngineDefined(
        EngineDefinedRoutingIntent::new(engine_id(), extension_id),
    ))
}

#[cfg(test)]
#[path = "routing_intent_tests.rs"]
mod tests;
