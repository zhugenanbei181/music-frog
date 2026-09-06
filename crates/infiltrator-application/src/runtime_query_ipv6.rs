//! Live Mihomo IPv6 policy operations.

use super::RuntimeQueryApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::ipv6::Ipv6RoutingSnapshot;
use std::sync::atomic::Ordering;

impl RuntimeQueryApplication {
    /// Apply Mihomo's top-level IPv6 policy and verify both the policy and TUN
    /// context through a controller readback before reporting success.
    pub async fn set_ipv6_routing(&self, enabled: bool) -> Result<Ipv6RoutingSnapshot, Failure> {
        self.gateway
            .patch_config(serde_json::json!({ "ipv6": enabled }))
            .await
            .map_err(Failure::from)?;
        let observed = self.gateway.get_config().await.map_err(Failure::from)?;
        if observed.ipv6 != enabled {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "IPv6 routing readback mismatch: requested {enabled}, observed {}",
                    observed.ipv6
                ),
                true,
            ));
        }
        Ok(Ipv6RoutingSnapshot::new(
            self.next_revision.fetch_add(1, Ordering::Relaxed),
            observed.ipv6,
            observed.tun.as_ref().is_some_and(|tun| tun.enable),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeQueryApplication;
    use crate::runtime_query_application::tests::{lan_state, TestGateway};
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn ipv6_routing_patch_reads_back_core_policy_and_tun_context() {
        let gateway = Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("system".to_owned())),
            tun_mtu: Arc::new(Mutex::new(Some(1420))),
            tun_routing: Arc::new(Mutex::new((true, true))),
            tun_enabled: Arc::new(Mutex::new(true)),
            lan: lan_state(),
            apply_patch: true,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        });
        let snapshot = RuntimeQueryApplication::new(gateway)
            .set_ipv6_routing(false)
            .await
            .expect("IPv6 routing readback should match");

        assert!(!snapshot.enabled);
        assert!(snapshot.tun_enabled);
        assert_eq!(snapshot.revision, 1);
    }

    #[tokio::test]
    async fn ipv6_routing_fails_closed_when_controller_ignores_patch() {
        let application = RuntimeQueryApplication::new(Arc::new(TestGateway {
            level: Arc::new(Mutex::new("info".to_owned())),
            tun_stack: Arc::new(Mutex::new("gvisor".to_owned())),
            tun_mtu: Arc::new(Mutex::new(None)),
            tun_routing: Arc::new(Mutex::new((true, false))),
            tun_enabled: Arc::new(Mutex::new(false)),
            lan: lan_state(),
            apply_patch: false,
            memory_bytes: Arc::new(Mutex::new(0)),
            gc_calls: Arc::new(AtomicUsize::new(0)),
        }));
        let failure = application
            .set_ipv6_routing(false)
            .await
            .expect_err("ignored IPv6 patch must not report success");

        assert_eq!(
            failure.code,
            infiltrator_contract::error::ErrorCode::InvalidState
        );
    }
}
