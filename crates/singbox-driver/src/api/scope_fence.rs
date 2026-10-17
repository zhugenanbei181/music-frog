//! CoreScope and live-version fence shared by every sing-box Clash API adapter.

use crate::api::clash_api::SingBoxCoreVersionProvider;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreScope, EngineId};
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::error::PortError;

pub(crate) fn engine_id() -> EngineId {
    EngineId::sing_box()
}

pub(crate) fn rejected(code: ErrorCode, message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(code, message, false))
}

pub(crate) fn verify_owner_scope(
    scope_source: &dyn CoreScopeSource,
    requested: &CoreScope,
) -> Result<(), PortError> {
    if requested.identity.engine != engine_id() {
        return Err(rejected(
            ErrorCode::InvalidState,
            "sing-box Clash API adapter received a scope for another engine",
        ));
    }
    if requested.identity.version.is_none() {
        return Err(rejected(
            ErrorCode::NotReady,
            "sing-box Clash API operations require an observed core version in CoreScope",
        ));
    }
    match scope_source.scope_observation() {
        CoreScopeObservation::Observed(current) if &current == requested => Ok(()),
        CoreScopeObservation::Observed(_) => Err(rejected(
            ErrorCode::InvalidState,
            "active CoreScope changed during sing-box Clash API operation",
        )),
        CoreScopeObservation::Unavailable => Err(rejected(
            ErrorCode::NotReady,
            "no active CoreScope is available for sing-box Clash API operation",
        )),
    }
}

pub(crate) async fn verify_live_scope(
    version_provider: &SingBoxCoreVersionProvider,
    scope_source: &dyn CoreScopeSource,
    requested: &CoreScope,
) -> Result<(), PortError> {
    verify_owner_scope(scope_source, requested)?;
    let actual = version_provider.observe().await?;
    verify_owner_scope(scope_source, requested)?;
    if requested.identity.version.as_ref() != Some(&actual) {
        return Err(rejected(
            ErrorCode::InvalidState,
            "live sing-box version differs from the observed CoreScope version",
        ));
    }
    Ok(())
}
