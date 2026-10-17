//! Shared conversion from a typed Core owner observation to a use-case scope.

use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;

pub(crate) fn active_scope(source: &dyn CoreScopeSource) -> Result<CoreScope, Failure> {
    match source.scope_observation() {
        CoreScopeObservation::Observed(scope) => Ok(scope),
        CoreScopeObservation::Unavailable => Err(Failure::new(
            ErrorCode::NotReady,
            "Core owner has no active scope",
            true,
        )),
    }
}
