//! Isolated port-conflict port fixture.
//!
//! The fixture publishes a real conflict list: a binding that another process
//! owns, and bindings that are free. The repair path only releases a conflict
//! the fixture says can be released; an unknown owner is refused with a typed
//! permission failure instead of killing a process it does not own.

use async_trait::async_trait;
use infiltrator_contract::port_conflict::{PortBinding, PortConflict, PortConflictSnapshot};
use infiltrator_ports::port_conflict::PortConflictPort;
use lurker_core_ports::error::PortError;
use std::sync::Mutex;

/// The mixed proxy port the fixture reports as taken.
pub const CAPTURE_TAKEN_MIXED_PORT: u16 = 7890;
/// The controller port the fixture reports as free.
pub const CAPTURE_FREE_CONTROLLER_PORT: u16 = 9090;
/// The DNS listener port the fixture reports as free.
pub const CAPTURE_FREE_DNS_PORT: u16 = 1053;

/// An isolated port-conflict port that answers with a real conflict list.
pub struct PortConflictCapturePort {
    snapshot: Mutex<PortConflictSnapshot>,
}

impl Default for PortConflictCapturePort {
    fn default() -> Self {
        Self::new()
    }
}

impl PortConflictCapturePort {
    /// A capture whose mixed proxy port is held by a foreign process.
    pub fn new() -> Self {
        Self::with_conflicts(vec![
            PortConflict {
                binding: PortBinding::MixedProxy,
                port: CAPTURE_TAKEN_MIXED_PORT,
                available: false,
                owner_pid: Some(31337),
                owner_name: Some("foreign-proxy".to_owned()),
                can_release: false,
            },
            PortConflict {
                binding: PortBinding::Controller,
                port: CAPTURE_FREE_CONTROLLER_PORT,
                available: true,
                owner_pid: None,
                owner_name: None,
                can_release: false,
            },
            PortConflict {
                binding: PortBinding::DnsListen,
                port: CAPTURE_FREE_DNS_PORT,
                available: true,
                owner_pid: None,
                owner_name: None,
                can_release: false,
            },
        ])
    }

    /// A capture that publishes exactly the supplied conflict list.
    pub fn with_conflicts(conflicts: Vec<PortConflict>) -> Self {
        Self {
            snapshot: Mutex::new(PortConflictSnapshot {
                revision: 1,
                conflicts,
            }),
        }
    }

    /// The conflict snapshot the port currently publishes.
    pub fn current_snapshot(&self) -> PortConflictSnapshot {
        self.snapshot
            .lock()
            .expect("port conflict capture snapshot")
            .clone()
    }
}

#[async_trait]
impl PortConflictPort for PortConflictCapturePort {
    async fn snapshot(&self) -> Result<PortConflictSnapshot, PortError> {
        Ok(self.current_snapshot())
    }

    async fn repair(&self) -> Result<PortConflictSnapshot, PortError> {
        let mut snapshot = self
            .snapshot
            .lock()
            .expect("port conflict capture snapshot");
        let blocked = snapshot
            .conflicts
            .iter()
            .find(|conflict| !conflict.available && !conflict.can_release)
            .cloned();
        if let Some(blocked) = blocked {
            return Err(PortError::PermissionDenied(format!(
                "port {} is owned by {} and cannot be released",
                blocked.port,
                blocked
                    .owner_name
                    .as_deref()
                    .unwrap_or("an unknown process")
            )));
        }
        snapshot.revision += 1;
        for conflict in snapshot.conflicts.iter_mut() {
            conflict.available = true;
            conflict.owner_pid = None;
            conflict.owner_name = None;
            conflict.can_release = false;
        }
        Ok(snapshot.clone())
    }
}
