//! Shared conversion from a typed Core owner observation to a use-case scope.

use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};

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
