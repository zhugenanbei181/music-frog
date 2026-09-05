//! Runtime-neutral read port for the complete UI surface model.

use async_trait::async_trait;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;

use crate::error::PortError;

/// Reads the canonical 11-page surface snapshot.
///
/// A host adapter may assemble the snapshot from Mihomo, profile storage,
/// platform capabilities, and other ports. The application/UI boundary sees
/// only this owned contract value.
#[async_trait]
pub trait SurfaceReader: Send + Sync {
    async fn read(&self) -> Result<SurfaceSnapshot, PortError>;
}
