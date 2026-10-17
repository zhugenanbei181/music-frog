//! Supplies capability matrices for an engine and version selected by the
//! current Core owner.

use infiltrator_contract::core_capabilities::CoreCapabilityMatrix;
use lurker_core_contract::identity::{CoreVersion, EngineId};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// A typed failure to obtain an engine-scoped capability matrix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreCapabilityProviderError {
    EngineUnavailable { engine_id: EngineId, reason: String },
    MatrixUnavailable { reason: String },
}

impl Display for CoreCapabilityProviderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EngineUnavailable { engine_id, reason } => {
                write!(
                    formatter,
                    "engine `{}` is unavailable: {reason}",
                    engine_id.as_str()
                )
            }
            Self::MatrixUnavailable { reason } => {
                write!(formatter, "core capability matrix is unavailable: {reason}")
            }
        }
    }
}

impl Error for CoreCapabilityProviderError {}

/// Produces a matrix whose host and adapter revision are authored by the
/// adapter; callers provide only the selected engine/version identity.
pub trait CoreCapabilityProvider: Send + Sync {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError>;
}
