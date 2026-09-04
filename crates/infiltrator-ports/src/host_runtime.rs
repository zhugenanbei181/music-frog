//! Host-owned runtime handle exposed to inbound surfaces.

use crate::core_lifecycle::CoreLifecyclePort;
use crate::runtime_gateway::ManagedRuntime;
use std::path::PathBuf;
use std::sync::Arc;

/// Cross-platform value describing TUN privilege/service state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TunServiceStatus {
    InstalledAndRunning,
    InstalledStopped,
    NotInstalled,
    MissingPrivilege,
    Unsupported,
}

/// The only runtime handle an inbound UI/FFI surface should store.
///
/// Concrete process managers, controller clients and platform service types
/// stay behind the host adapter. `lifecycle_port` is provided for composition
/// code that needs a typed restart without exposing the application object.
pub trait HostRuntime: ManagedRuntime {
    fn controller_url(&self) -> String;
    fn core_binary_path(&self) -> PathBuf;
    fn tun_service_status(&self) -> TunServiceStatus;
    fn lifecycle_port(&self) -> Arc<dyn CoreLifecyclePort>;
}
