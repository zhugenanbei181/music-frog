//! Engine-scoped capability resolution and command admission.

use infiltrator_contract::core_capabilities::{
    CapabilityAccess, CoreCapabilityKey, CoreCapabilityMatrix, CoreCapabilityResolution,
    CoreCapabilityScope, CoreCapabilityStatus, CoreOperation, EngineBinaryTarget,
};
use infiltrator_contract::core_identity::{CoreScope, CoreVersion, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct CoreCapabilityApplication {
    provider: Option<Arc<dyn CoreCapabilityProvider>>,
}

impl Default for CoreCapabilityApplication {
    fn default() -> Self {
        Self::without_provider()
    }
}

impl CoreCapabilityApplication {
    pub fn new(provider: Arc<dyn CoreCapabilityProvider>) -> Self {
        Self {
            provider: Some(provider),
        }
    }

    /// A missing provider is represented explicitly and always denies a
    /// backend command; it never means that the capability gate is disabled.
    pub fn without_provider() -> Self {
        Self { provider: None }
    }

    /// Read the full owner-authored capability matrix for a selected engine
    /// and version. A provider returning a different identity or a matrix for
    /// another compiled binary target is rejected.
    pub fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, Failure> {
        let Some(provider) = self.provider.as_ref() else {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "core capability provider is unavailable",
                true,
            ));
        };
        let matrix = provider
            .matrix_for(engine_id, core_version)
            .map_err(capability_matrix_failure)?;
        if matrix.scope.engine_id != *engine_id
            || matrix.scope.core_version.as_ref() != core_version
            || matrix.scope.binary_target != EngineBinaryTarget::current()
            || matrix.scope.binary_target.host_kind() != Some(matrix.scope.host)
            || matrix.scope.adapter_revision.value() == 0
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "core capability provider returned a matrix for another engine/version, a different binary target, or a mismatched host/binary target",
                false,
            ));
        }
        Ok(matrix)
    }

    pub fn resolve(
        &self,
        requested_scope: Option<&CoreScope>,
        operation: CoreOperation,
    ) -> CoreCapabilityLookup {
        let Some(requested_scope) = requested_scope else {
            return CoreCapabilityLookup::Unavailable {
                operation,
                reason: CapabilityUnavailableReason::CoreScopeUnavailable,
            };
        };
        let matrix = match self.matrix_for(
            &requested_scope.identity.engine,
            requested_scope.identity.version.as_ref(),
        ) {
            Ok(matrix) => matrix,
            Err(error) => {
                if error.code == ErrorCode::InvalidState {
                    return CoreCapabilityLookup::InvalidProviderMatrix {
                        operation,
                        failure: error,
                    };
                }
                return CoreCapabilityLookup::Unavailable {
                    operation,
                    reason: CapabilityUnavailableReason::ProviderUnavailable(error.message),
                };
            }
        };
        CoreCapabilityLookup::Resolved(resolve_matrix(&matrix, requested_scope, operation))
    }

    pub fn authorize(
        &self,
        requested_scope: Option<&CoreScope>,
        requirement: CoreCapabilityRequirement,
    ) -> Result<CoreCapabilityResolution, CoreCapabilityGateFailure> {
        let resolution = match self.resolve(requested_scope, requirement.operation) {
            CoreCapabilityLookup::Resolved(resolution) => resolution,
            CoreCapabilityLookup::Unavailable { operation, reason } => {
                return Err(CoreCapabilityGateFailure::Unavailable { operation, reason });
            }
            CoreCapabilityLookup::InvalidProviderMatrix { operation, failure } => {
                return Err(CoreCapabilityGateFailure::InvalidProviderMatrix {
                    operation,
                    failure,
                });
            }
        };
        match &resolution.status {
            CoreCapabilityStatus::Available {
                access: CapabilityAccess::Supported,
                ..
            } => Ok(resolution),
            CoreCapabilityStatus::Available {
                access: CapabilityAccess::ReadOnly,
                ..
            } if requirement.effect == CoreCommandEffect::Read => Ok(resolution),
            CoreCapabilityStatus::Available {
                access: CapabilityAccess::ReadOnly,
                ..
            } => Err(CoreCapabilityGateFailure::ReadOnly(Box::new(resolution))),
            CoreCapabilityStatus::Unsupported { .. }
            | CoreCapabilityStatus::VersionUnavailable { .. }
            | CoreCapabilityStatus::HostUnavailable { .. }
            | CoreCapabilityStatus::AdapterMissing { .. }
            | CoreCapabilityStatus::Unregistered
            | CoreCapabilityStatus::ScopeMismatch => {
                Err(CoreCapabilityGateFailure::Rejected(Box::new(resolution)))
            }
        }
    }
}

fn capability_matrix_failure(error: CoreCapabilityProviderError) -> Failure {
    match &error {
        CoreCapabilityProviderError::EngineUnavailable { .. } => {
            Failure::unsupported(error.to_string())
        }
        CoreCapabilityProviderError::MatrixUnavailable { .. } => {
            Failure::new(ErrorCode::NotReady, error.to_string(), true)
        }
    }
}

fn resolve_matrix(
    matrix: &CoreCapabilityMatrix,
    requested_scope: &CoreScope,
    operation: CoreOperation,
) -> CoreCapabilityResolution {
    let key = CoreCapabilityKey::new(
        CoreCapabilityScope::new_for_target(
            requested_scope.identity.engine.clone(),
            requested_scope.identity.version.clone(),
            matrix.scope.host,
            matrix.scope.binary_target,
            matrix.scope.adapter_revision,
        ),
        operation,
    );
    matrix.resolve(key)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreCommandEffect {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoreCapabilityRequirement {
    pub operation: CoreOperation,
    pub effect: CoreCommandEffect,
}

impl CoreCapabilityRequirement {
    pub const fn new(operation: CoreOperation, effect: CoreCommandEffect) -> Self {
        Self { operation, effect }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreCapabilityLookup {
    Resolved(CoreCapabilityResolution),
    Unavailable {
        operation: CoreOperation,
        reason: CapabilityUnavailableReason,
    },
    InvalidProviderMatrix {
        operation: CoreOperation,
        failure: Failure,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapabilityUnavailableReason {
    CoreScopeUnavailable,
    ProviderUnavailable(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreCapabilityGateFailure {
    Unavailable {
        operation: CoreOperation,
        reason: CapabilityUnavailableReason,
    },
    InvalidProviderMatrix {
        operation: CoreOperation,
        failure: Failure,
    },
    Rejected(Box<CoreCapabilityResolution>),
    ReadOnly(Box<CoreCapabilityResolution>),
}

impl CoreCapabilityGateFailure {
    pub fn into_failure(self) -> Failure {
        match self {
            Self::InvalidProviderMatrix { failure, .. } => failure,
            Self::Unavailable { operation, reason } => Failure::new(
                ErrorCode::NotReady,
                format!("capability gate unavailable for {operation:?}: {reason:?}"),
                true,
            ),
            Self::Rejected(resolution) => {
                let resolution = *resolution;
                let code = match &resolution.status {
                    CoreCapabilityStatus::ScopeMismatch => ErrorCode::InvalidState,
                    CoreCapabilityStatus::VersionUnavailable { .. }
                    | CoreCapabilityStatus::HostUnavailable { .. }
                    | CoreCapabilityStatus::AdapterMissing { .. }
                    | CoreCapabilityStatus::Unregistered
                    | CoreCapabilityStatus::Unsupported { .. } => ErrorCode::Unsupported,
                    CoreCapabilityStatus::Available { .. } => ErrorCode::Internal,
                };
                Failure::new(
                    code,
                    format!("core capability denied: {:?}", resolution.status),
                    false,
                )
            }
            Self::ReadOnly(resolution) => {
                let resolution = *resolution;
                Failure::new(
                    ErrorCode::Unsupported,
                    format!(
                        "core capability is read-only: {:?}",
                        resolution.key.operation
                    ),
                    false,
                )
            }
        }
    }
}

#[cfg(test)]
#[path = "core_capability_application_tests.rs"]
mod tests;
