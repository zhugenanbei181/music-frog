//! Legacy desktop Host bootstrap boundary.
//!
//! The former retry path started a temporary CoreApplication and dropped it
//! while the process remained live, then created another owner during
//! materialization. It cannot transfer the product-wide active-profile owner
//! lease. Product startup uses `DesktopProductSession`; admin/rebuild callers
//! remain fail-closed until they can reuse that same Core session.

use infiltrator_contract::capability::Capability;
use infiltrator_ports::core_binary_selection::CoreBinarySelectionPolicy;
use infiltrator_ports::error::PortError;
use infiltrator_ports::host_runtime::HostRuntime;
use std::path::PathBuf;
use std::sync::Arc;

/// Binary-source preference retained for the Iced admin/rebuild boundary.
/// The Host bootstrap currently returns typed Unsupported before using it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopBinaryPreference {
    ManagedWithBundledFallback,
    PreferBundled,
    RequireBundled,
}

/// Legacy Host startup cannot safely transfer its selected-profile owner
/// binding into a long-lived runtime. Use the product Core session instead.
pub async fn bootstrap_host_runtime(
    _policy: CoreBinarySelectionPolicy,
) -> anyhow::Result<(Arc<dyn HostRuntime>, bool)> {
    Err(unsupported_legacy_host_boot())
}

/// Iced admin/rebuild entry point. Its caller must migrate to the same
/// already-bound product Core session before this API can start a process.
pub async fn bootstrap_host_runtime_from_current_home(
    _preference: DesktopBinaryPreference,
    _bundled_candidates: &[PathBuf],
) -> anyhow::Result<(Arc<dyn HostRuntime>, bool)> {
    Err(unsupported_legacy_host_boot())
}

fn unsupported_legacy_host_boot() -> anyhow::Error {
    anyhow::Error::new(PortError::unsupported(
        Capability::CoreLifecycle,
        "desktop Host retry has no transferable active-profile owner lease; use the product Core session",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn legacy_host_bootstrap_is_typed_unsupported() {
        let result = bootstrap_host_runtime(CoreBinarySelectionPolicy::ManagedOnly).await;
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("legacy Host bootstrap unexpectedly produced a runtime"),
        };
        assert!(matches!(
            error.downcast_ref::<PortError>(),
            Some(PortError::Unsupported {
                capability: Capability::CoreLifecycle,
                ..
            })
        ));
    }
}
