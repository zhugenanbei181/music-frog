//! Typed observation of the active Core owner's scope.

use lurker_core_contract::identity::CoreScope;

/// The owner either has a usable scope or has no scope to publish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreScopeObservation {
    Unavailable,
    Observed(CoreScope),
}

/// Reads the active scope from the single Core owner.
pub trait CoreScopeSource: Send + Sync {
    fn scope_observation(&self) -> CoreScopeObservation;
}
