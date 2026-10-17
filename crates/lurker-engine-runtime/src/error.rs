//! Typed registry mutation failures.

use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// A registry mutation that cannot be applied to the requested engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineBackendRegistryError {
    /// A provider is already registered for this engine.
    DuplicateProvider { engine_id: EngineId },
    /// No provider is registered for this engine.
    UnknownProvider { engine_id: EngineId },
}

impl Display for EngineBackendRegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateProvider { engine_id } => write!(
                formatter,
                "an engine backend provider is already registered for {}",
                engine_id.as_str()
            ),
            Self::UnknownProvider { engine_id } => write!(
                formatter,
                "no engine backend provider is registered for {}",
                engine_id.as_str()
            ),
        }
    }
}

impl Error for EngineBackendRegistryError {}

impl From<EngineBackendRegistryError> for PortError {
    fn from(error: EngineBackendRegistryError) -> Self {
        Self::Failed(error.to_string())
    }
}
