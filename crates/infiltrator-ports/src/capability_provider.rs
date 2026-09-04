use infiltrator_contract::capability::CapabilitySnapshot;
use infiltrator_contract::surface::HostKind;

/// Reports host capabilities without forcing the application to inspect
/// target-specific APIs or `cfg(target_os)` branches.
pub trait CapabilityProvider: Send + Sync {
    fn host_kind(&self) -> HostKind;
    fn capabilities(&self) -> CapabilitySnapshot;
}
