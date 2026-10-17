//! Shared admission and verification for engine-neutral routing changes.

use crate::core_capability_application::{
    CoreCapabilityApplication, CoreCapabilityGateFailure, CoreCapabilityRequirement,
    CoreCommandEffect,
};
use infiltrator_contract::core_capabilities::{CoreOperation, ProxyControlOperation};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::routing_intent::EngineDefinedRoutingIntent;
use lurker_core_contract::routing_intent::{
    RoutingIntent, RoutingIntentExtensionId, RoutingIntentSnapshot, RoutingIntentStatus,
};
use lurker_core_ports::routing_intent::RoutingIntentPort;
use std::sync::Arc;

const ROUTING_INTENT_REQUIREMENT: CoreCapabilityRequirement = CoreCapabilityRequirement::new(
    CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
    CoreCommandEffect::Write,
);

const ROUTING_INTENT_READ_REQUIREMENT: CoreCapabilityRequirement = CoreCapabilityRequirement::new(
    CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
    CoreCommandEffect::Read,
);

#[derive(Clone)]
pub struct RoutingIntentApplication {
    port: Arc<dyn RoutingIntentPort>,
    capabilities: CoreCapabilityApplication,
}

impl RoutingIntentApplication {
    pub fn new(port: Arc<dyn RoutingIntentPort>, capabilities: CoreCapabilityApplication) -> Self {
        Self { port, capabilities }
    }

    /// Read through the same engine adapter used for typed writes. Missing
    /// scope or capability is represented as a non-selectable typed state.
    pub async fn observe(&self, scope: Option<&CoreScope>) -> RoutingIntentSnapshot {
        let Some(scope) = scope else {
            return failed_snapshot(Failure::new(
                ErrorCode::InvalidState,
                "routing intent observation has no observed Core scope",
                false,
            ));
        };
        if let Err(error) = self
            .capabilities
            .authorize(Some(scope), ROUTING_INTENT_READ_REQUIREMENT)
        {
            return failed_snapshot(CoreCapabilityGateFailure::into_failure(error));
        }
        match self.port.observe(scope).await {
            Ok(current) => {
                if let RoutingIntent::EngineDefined(extension) = &current
                    && extension.engine() != &scope.identity.engine
                {
                    return failed_snapshot(Failure::new(
                        ErrorCode::InvalidState,
                        "engine-defined routing readback belongs to another Core engine",
                        false,
                    ));
                }
                RoutingIntentSnapshot {
                    current: Some(current),
                    status: RoutingIntentStatus::Ready,
                    failure: None,
                }
            }
            Err(error) => failed_snapshot(Failure::from(error)),
        }
    }

    /// Apply only when both the scope and its exact operation capability are
    /// known. Missing or unregistered capability data is always a refusal.
    pub async fn apply(
        &self,
        scope: Option<&CoreScope>,
        intent: RoutingIntent,
    ) -> Result<RoutingIntent, Failure> {
        let scope = scope.ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "routing intent has no observed Core scope",
                false,
            )
        })?;

        if let RoutingIntent::EngineDefined(extension) = &intent
            && extension.engine() != &scope.identity.engine
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "engine-defined routing intent belongs to another Core engine",
                false,
            ));
        }

        self.capabilities
            .authorize(Some(scope), ROUTING_INTENT_REQUIREMENT)
            .map_err(CoreCapabilityGateFailure::into_failure)?;

        let observed = self
            .port
            .apply(scope, intent.clone())
            .await
            .map_err(Failure::from)?;
        if observed != intent {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Core routing intent readback did not match the requested intent",
                true,
            ));
        }
        Ok(observed)
    }
}

fn failed_snapshot(failure: Failure) -> RoutingIntentSnapshot {
    RoutingIntentSnapshot {
        current: None,
        status: if failure.code == ErrorCode::Unsupported {
            RoutingIntentStatus::Unsupported
        } else {
            RoutingIntentStatus::Failed
        },
        failure: Some(failure),
    }
}

/// Build an engine-defined extension request without exposing an unchecked ID.
pub fn engine_defined_intent(
    engine: EngineId,
    extension_id: impl Into<String>,
) -> Result<RoutingIntent, Failure> {
    let extension_id = RoutingIntentExtensionId::new(extension_id)
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    Ok(RoutingIntent::EngineDefined(
        EngineDefinedRoutingIntent::new(engine, extension_id),
    ))
}

#[cfg(test)]
#[path = "routing_intent_application_tests.rs"]
mod tests;
