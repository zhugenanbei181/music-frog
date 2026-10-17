//! Shared conversion from a typed Core owner observation to a use-case scope.

use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};

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
