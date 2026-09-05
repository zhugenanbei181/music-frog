//! Physical-link to TUN MTU negotiation use-case.

use infiltrator_contract::error::Failure;
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, MtuProbeState};
use infiltrator_ports::error::PortError;
use infiltrator_ports::mtu_probe::MtuProbePort;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use crate::runtime_query_application::RuntimeQueryApplication;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MTU_PROBE_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct MtuApplication {
    port: Arc<dyn MtuProbePort>,
    next_revision: Arc<AtomicU64>,
    last_snapshot: Arc<Mutex<Option<(Instant, MtuNegotiationSnapshot)>>>,
}

impl MtuApplication {
    pub fn new(port: Arc<dyn MtuProbePort>) -> Self {
        Self {
            port,
            next_revision: Arc::new(AtomicU64::new(1)),
            last_snapshot: Arc::new(Mutex::new(None)),
        }
    }

    /// Force a fresh host observation. User-triggered probes use this path so
    /// a network handover cannot be hidden behind the surface cache.
    pub async fn probe(&self) -> MtuNegotiationSnapshot {
        let snapshot = self.probe_fresh().await;
        self.cache(snapshot.clone());
        snapshot
    }

    /// Read a recent observation for telemetry surfaces. Interface probing
    /// invokes OS commands, so a frame-driven surface must not repeat it on
    /// every tick.
    pub async fn probe_cached(&self) -> MtuNegotiationSnapshot {
        if let Some((probed_at, snapshot)) = self
            .last_snapshot
            .lock()
            .expect("MTU snapshot cache lock")
            .as_ref()
            .cloned()
            && probed_at.elapsed() < MTU_PROBE_CACHE_TTL
        {
            return snapshot;
        }
        self.probe().await
    }

    /// Probe, calculate and atomically verify the live controller update.
    /// The returned contract snapshot records whether Mihomo confirmed the
    /// calculated value; a failed apply never masquerades as a ready result.
    pub async fn probe_and_apply(
        &self,
        gateway: Arc<dyn RuntimeGateway>,
    ) -> MtuNegotiationSnapshot {
        let snapshot = self.probe().await;
        let tun_mtu = snapshot.tun_mtu;
        let applied = match (&snapshot.state, tun_mtu) {
            (MtuProbeState::Ready, Some(tun_mtu)) => {
                Some(
                    RuntimeQueryApplication::new(gateway)
                        .set_tun_mtu(tun_mtu)
                        .await,
                )
            }
            (MtuProbeState::Ready, None) => Some(Err(Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidState,
                "ready MTU probe did not contain a calculated TUN MTU",
                false,
            ))),
            _ => None,
        };
        let snapshot = match applied {
            Some(Ok(())) => match tun_mtu {
                Some(tun_mtu) => snapshot.with_applied_tun_mtu(tun_mtu),
                None => snapshot.with_failure(Failure::new(
                    infiltrator_contract::error::ErrorCode::InvalidState,
                    "ready MTU probe did not contain a calculated TUN MTU",
                    false,
                )),
            },
            Some(Err(failure)) => snapshot.with_failure(failure),
            None => snapshot,
        };
        self.cache(snapshot.clone());
        snapshot
    }

    async fn probe_fresh(&self) -> MtuNegotiationSnapshot {
        let revision = self.next_revision.fetch_add(1, Ordering::Relaxed);
        match self.port.probe_physical_mtu().await {
            Ok(physical)
                if !physical.interface.trim().is_empty()
                    && (infiltrator_contract::mtu::MIN_TUN_MTU_BYTES
                        ..=infiltrator_contract::mtu::MAX_TUN_MTU_BYTES)
                        .contains(&physical.mtu) =>
            {
                let (tun_mtu, tcp_mss) = infiltrator_domain::mtu_optimizer::MtuOptimizer::
                    negotiate_tun_mtu(physical.mtu);
                MtuNegotiationSnapshot::ready(revision, physical, tun_mtu, tcp_mss)
            }
            Ok(_) => MtuNegotiationSnapshot::failed(
                revision,
                Failure::new(
                    infiltrator_contract::error::ErrorCode::InvalidInput,
                    "physical MTU probe returned an invalid interface or MTU",
                    false,
                ),
            ),
            Err(PortError::Unsupported { .. }) => {
                MtuNegotiationSnapshot::unsupported(revision)
            }
            Err(error) => MtuNegotiationSnapshot::failed(revision, Failure::from(error)),
        }
    }

    fn cache(&self, snapshot: MtuNegotiationSnapshot) {
        *self
            .last_snapshot
            .lock()
            .expect("MTU snapshot cache lock") = Some((Instant::now(), snapshot));
    }

    pub fn probing_snapshot(&self) -> MtuNegotiationSnapshot {
        MtuNegotiationSnapshot::probing(self.next_revision.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::mtu::{MtuProbeState, PhysicalMtuSnapshot};

    struct ReadyProbe;

    #[async_trait]
    impl MtuProbePort for ReadyProbe {
        async fn probe_physical_mtu(&self) -> Result<PhysicalMtuSnapshot, PortError> {
            Ok(PhysicalMtuSnapshot {
                interface: "wlan0".to_owned(),
                mtu: 1500,
            })
        }
    }

    struct UnsupportedProbe;

    #[async_trait]
    impl MtuProbePort for UnsupportedProbe {
        async fn probe_physical_mtu(&self) -> Result<PhysicalMtuSnapshot, PortError> {
            Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Tun,
                "native interface MTU is not exposed",
            ))
        }
    }

    struct InvalidProbe;

    #[async_trait]
    impl MtuProbePort for InvalidProbe {
        async fn probe_physical_mtu(&self) -> Result<PhysicalMtuSnapshot, PortError> {
            Ok(PhysicalMtuSnapshot {
                interface: "eth0".to_owned(),
                mtu: 1200,
            })
        }
    }

    #[tokio::test]
    async fn application_negotiates_from_host_physical_mtu() {
        let application = MtuApplication::new(Arc::new(ReadyProbe));
        let snapshot = application.probe().await;
        assert_eq!(snapshot.state, MtuProbeState::Ready);
        assert_eq!(snapshot.tun_mtu, Some(1420));
        assert_eq!(snapshot.tcp_mss, Some(1380));
        assert_eq!(snapshot.revision, 1);
    }

    #[tokio::test]
    async fn application_preserves_typed_unsupported_state() {
        let application = MtuApplication::new(Arc::new(UnsupportedProbe));
        assert_eq!(application.probe().await.state, MtuProbeState::Unsupported);
    }

    #[tokio::test]
    async fn application_rejects_a_physical_mtu_below_the_safe_tun_floor() {
        let application = MtuApplication::new(Arc::new(InvalidProbe));
        let snapshot = application.probe().await;
        assert!(matches!(snapshot.state, MtuProbeState::Failed { ref failure }
            if failure.code == infiltrator_contract::error::ErrorCode::InvalidInput));
    }
}
