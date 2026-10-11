//! Engine-scoped selection of the executable used by one Core owner.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::core_identity::{CoreVersion, EngineId};
use infiltrator_contract::version::CoreArtifactVerification;
use std::path::PathBuf;

/// Search roots supplied by the host package composition for a bundled engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundledBinarySource {
    pub candidates: Vec<PathBuf>,
    pub data_directory: PathBuf,
}

/// Policy for selecting the engine executable. Every case has one clear
/// source preference and cannot be assembled from independent flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreBinarySelectionPolicy {
    /// Use an installed, engine-managed version only.
    ManagedOnly,
    /// Prefer an installed version, but use a bundled engine if none exists.
    ManagedWithBundledFallback(BundledBinarySource),
    /// Prefer a bundled engine, then fall back to an installed version.
    PreferBundled(BundledBinarySource),
    /// Require the bundled engine; never silently fall back to managed state.
    RequireBundled(BundledBinarySource),
}

/// The selected executable's origin, including content identity for a bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreBinarySelectionSource {
    ManagedVersion,
    BundledPackage { content_sha256: String },
}

/// One resolved executable, bound to the engine and version that reported it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreBinarySelection {
    pub engine_id: EngineId,
    pub version: CoreVersion,
    pub binary_path: PathBuf,
    /// Additional package directories that may contain matching sidecar
    /// assets such as the Mihomo GeoIP database.
    pub related_asset_candidates: Vec<PathBuf>,
    pub source: CoreBinarySelectionSource,
    /// Installation verification is separate from bundle content addressing.
    /// Bundled assets stay `Unknown` until an official evidence lock verifies
    /// them for the active target.
    pub verification: CoreArtifactVerification,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreBinarySelectionRequest {
    pub engine_id: EngineId,
    pub policy: CoreBinarySelectionPolicy,
}

#[async_trait]
pub trait CoreBinarySelectionPort: Send + Sync {
    fn engine_id(&self) -> EngineId;

    async fn select(
        &self,
        request: CoreBinarySelectionRequest,
    ) -> Result<CoreBinarySelection, PortError>;
}
