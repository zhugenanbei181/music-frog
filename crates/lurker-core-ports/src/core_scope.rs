//! Typed observation of the active Core owner's scope.

use lurker_core_contract::identity::CoreScope;

/// The owner either has a usable scope or has no scope to publish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreScopeObservation {
    /// No scope is available; the owner is unbound or not yet started.
    Unavailable,
    /// The scope of the currently active owner.
    Observed(CoreScope),
}

/// Reads the active scope from the single Core owner.
pub trait CoreScopeSource: Send + Sync {
    /// Returns the active scope, or `Unavailable` when there is none.
    ///
    /// Implementations must never fabricate a scope: an owner that is not
    /// bound has no scope to publish.
    fn scope_observation(&self) -> CoreScopeObservation;
}
