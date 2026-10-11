//! Composition seam for the engine-scoped version application.

use crate::core_binary_selection::CoreBinarySelectionPort;
use crate::error::PortError;
use crate::version::VersionPort;
use infiltrator_contract::core_identity::EngineId;
use std::sync::Arc;

/// Resolves an engine to a concrete version adapter that the host actually
/// implements. Unsupported engine identifiers must return typed Unsupported.
pub trait EngineVersionBackendFactory: Send + Sync {
    fn version_port(&self, engine_id: &EngineId) -> Result<Arc<dyn VersionPort>, PortError>;

    /// Resolves the engine-owned executable selection adapter. Implementors
    /// must reject unregistered engines before creating storage or touching
    /// binary candidates.
    fn binary_selection_port(
        &self,
        engine_id: &EngineId,
    ) -> Result<Arc<dyn CoreBinarySelectionPort>, PortError>;
}
