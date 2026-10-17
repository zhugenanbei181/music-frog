//! Engine-scoped selection of the executable used by one Core owner.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_capabilities::EngineBinaryTarget;
use infiltrator_contract::version::CoreArtifactVerification;
use lurker_core_contract::identity::{CoreVersion, EngineId};
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
    /// Exact Cargo target of the selected executable. This is independent of
    /// engine/version verification and must match the current host build.
    pub binary_target: EngineBinaryTarget,
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

impl CoreBinarySelection {
    /// Reject cross-engine, cross-target, and unregistered selections before
    /// a product owner constructs a process or capability-scoped adapter.
    pub fn validate_for(
        &self,
        engine_id: &EngineId,
        expected_target: EngineBinaryTarget,
    ) -> Result<(), PortError> {
        if self.engine_id != *engine_id {
            return Err(PortError::Failed(format!(
                "Core binary selection for {} was supplied to {}",
                self.engine_id.as_str(),
                engine_id.as_str()
            )));
        }
        if expected_target == EngineBinaryTarget::Unregistered
            || self.binary_target != expected_target
        {
            return Err(PortError::unsupported(
                Capability::CoreLifecycle,
                format!(
                    "Core binary target {:?} does not match registered host target {:?}",
                    self.binary_target, expected_target
                ),
            ));
        }
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(binary_target: EngineBinaryTarget) -> CoreBinarySelection {
        CoreBinarySelection {
            engine_id: EngineId::new("sing-box").expect("engine ID"),
            binary_target,
            version: CoreVersion::new("1.14.3").expect("version"),
            binary_path: PathBuf::from("/managed/sing-box"),
            related_asset_candidates: Vec::new(),
            source: CoreBinarySelectionSource::ManagedVersion,
            verification: CoreArtifactVerification::Unknown,
        }
    }

    #[test]
    fn binary_selection_requires_exact_engine_and_registered_target() {
        let engine_id = EngineId::new("sing-box").expect("engine ID");
        let host_target = EngineBinaryTarget::current();
        assert_ne!(host_target, EngineBinaryTarget::Unregistered);
        selection(host_target)
            .validate_for(&engine_id, host_target)
            .expect("matching engine and target are accepted");

        assert!(
            selection(EngineBinaryTarget::Unregistered)
                .validate_for(&engine_id, host_target)
                .is_err()
        );
        assert!(
            selection(host_target)
                .validate_for(&EngineId::new("mihomo").expect("engine ID"), host_target,)
                .is_err()
        );
        assert!(
            selection(EngineBinaryTarget::Unregistered)
                .validate_for(&engine_id, EngineBinaryTarget::Unregistered)
                .is_err()
        );
    }
}
