//! Port for observing and applying a shared traffic-routing intent.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::routing_intent::RoutingIntent;

/// Observes and applies a shared routing intent through one engine adapter.
///
/// Implementations must reject unsupported or foreign-engine intents before
/// issuing a write and must confirm successful writes by observing the active
/// intent again.
#[async_trait]
pub trait RoutingIntentPort: Send + Sync {
    /// Observe the active intent for the supplied owner-authored scope.
    async fn observe(&self, scope: &CoreScope) -> Result<RoutingIntent, PortError>;

    /// Apply a value and return the value read back from the core.
    async fn apply(
        &self,
        scope: &CoreScope,
        intent: RoutingIntent,
    ) -> Result<RoutingIntent, PortError>;
}
