//! Shared physical-link to TUN MTU negotiation contract.

use crate::error::Failure;
use serde::{Deserialize, Serialize};

pub const DEFAULT_TUN_OVERHEAD_BYTES: u32 = 80;
pub const MIN_TUN_MTU_BYTES: u32 = 1280;
pub const MAX_TUN_MTU_BYTES: u32 = 9000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalMtuSnapshot {
    pub interface: String,
    pub mtu: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MtuProbeState {
    Unknown,
    Probing,
    Ready,
    Unsupported,
    Failed { failure: Failure },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MtuNegotiationSnapshot {
    pub state: MtuProbeState,
    pub physical_interface: Option<String>,
    pub physical_mtu: Option<u32>,
    pub tun_mtu: Option<u32>,
    /// The last TUN MTU confirmed by the running Mihomo controller. A probe
    /// can calculate a value without being allowed to apply it (for example
    /// on a stopped or mobile host), so this must remain distinct from
    /// `tun_mtu`.
    #[serde(default)]
    pub applied_tun_mtu: Option<u32>,
    pub tcp_mss: Option<u32>,
    pub overhead_bytes: u32,
    pub revision: u64,
}

impl Default for MtuNegotiationSnapshot {
    fn default() -> Self {
        Self {
            state: MtuProbeState::Unknown,
            physical_interface: None,
            physical_mtu: None,
            tun_mtu: None,
            applied_tun_mtu: None,
            tcp_mss: None,
            overhead_bytes: DEFAULT_TUN_OVERHEAD_BYTES,
            revision: 0,
        }
    }
}

impl MtuNegotiationSnapshot {
    pub fn probing(revision: u64) -> Self {
        Self {
            state: MtuProbeState::Probing,
            revision,
            ..Self::default()
        }
    }

    pub fn ready(revision: u64, physical: PhysicalMtuSnapshot, tun_mtu: u32, tcp_mss: u32) -> Self {
        Self {
            state: MtuProbeState::Ready,
            physical_interface: Some(physical.interface),
            physical_mtu: Some(physical.mtu),
            tun_mtu: Some(tun_mtu),
            applied_tun_mtu: None,
            tcp_mss: Some(tcp_mss),
            overhead_bytes: DEFAULT_TUN_OVERHEAD_BYTES,
            revision,
        }
    }

    pub fn unsupported(revision: u64) -> Self {
        Self {
            state: MtuProbeState::Unsupported,
            revision,
            ..Self::default()
        }
    }

    pub fn failed(revision: u64, failure: Failure) -> Self {
        Self {
            state: MtuProbeState::Failed { failure },
            revision,
            ..Self::default()
        }
    }

    pub fn is_ready(&self) -> bool {
        self.state == MtuProbeState::Ready
    }

    pub fn with_applied_tun_mtu(mut self, mtu: u32) -> Self {
        self.applied_tun_mtu = Some(mtu);
        self
    }

    pub fn with_failure(mut self, failure: Failure) -> Self {
        self.state = MtuProbeState::Failed { failure };
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_snapshot_keeps_physical_and_virtual_values() {
        let snapshot = MtuNegotiationSnapshot::ready(
            4,
            PhysicalMtuSnapshot {
                interface: "eth0".to_owned(),
                mtu: 1500,
            },
            1420,
            1380,
        );
        assert!(snapshot.is_ready());
        assert_eq!(snapshot.physical_interface.as_deref(), Some("eth0"));
        assert_eq!(snapshot.physical_mtu, Some(1500));
        assert_eq!(snapshot.tun_mtu, Some(1420));
        assert_eq!(snapshot.applied_tun_mtu, None);
        assert_eq!(snapshot.tcp_mss, Some(1380));
        assert_eq!(snapshot.clone().with_applied_tun_mtu(1420).applied_tun_mtu, Some(1420));
    }

    #[test]
    fn unsupported_and_failed_are_not_ready() {
        assert!(!MtuNegotiationSnapshot::unsupported(1).is_ready());
        assert!(!MtuNegotiationSnapshot::failed(
            2,
            Failure::unsupported("no native link probe")
        )
        .is_ready());
    }
}
