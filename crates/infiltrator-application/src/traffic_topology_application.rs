//! Application seam for the shared live traffic topology.

use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_contract::traffic_topology::TrafficTopologySnapshot;
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::runtime::{ConfigSnapshot, ConnectionSnapshot};
use infiltrator_ports::error::PortError;
use std::collections::HashMap;

/// Converts composed controller observations into one immutable topology
/// snapshot. UI surfaces never call a gateway or infer the routing chain.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrafficTopologyApplication;

impl TrafficTopologyApplication {
    pub fn project(
        &self,
        core: &CoreSnapshot,
        config: Option<&Result<ConfigSnapshot, PortError>>,
        proxies: Option<&Result<HashMap<String, Proxy>, PortError>>,
        connections: Option<&Result<ConnectionSnapshot, PortError>>,
    ) -> TrafficTopologySnapshot {
        let revision = core.revision.max(1);
        if !matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ) {
            return TrafficTopologySnapshot::unavailable(
                core.generation,
                revision,
                "core is not running; topology is not sampled",
            );
        }

        let Some(config) = config else {
            return TrafficTopologySnapshot::unsupported(
                core.generation,
                revision,
                "runtime config gateway is not composed",
            );
        };
        let Some(proxies) = proxies else {
            return TrafficTopologySnapshot::unsupported(
                core.generation,
                revision,
                "proxy gateway is not composed",
            );
        };
        let Some(connections) = connections else {
            return TrafficTopologySnapshot::unsupported(
                core.generation,
                revision,
                "connection gateway is not composed",
            );
        };

        let config = match config {
            Ok(config) => config,
            Err(error) => return failed(core, revision, "runtime config", error),
        };
        let proxies = match proxies {
            Ok(proxies) => proxies,
            Err(error) => return failed(core, revision, "proxy list", error),
        };
        let connections = match connections {
            Ok(connections) => connections,
            Err(error) => return failed(core, revision, "connection list", error),
        };

        infiltrator_domain::traffic_topology::derive(
            infiltrator_domain::traffic_topology::TrafficTopologyInput {
                generation: core.generation,
                revision,
                lifecycle: core.lifecycle.clone(),
                upload_bps: core.upload_bps,
                download_bps: core.download_bps,
                config,
                connections: &connections.connections,
                proxies,
            },
        )
    }
}

fn failed(
    core: &CoreSnapshot,
    revision: u64,
    source: &str,
    error: &PortError,
) -> TrafficTopologySnapshot {
    TrafficTopologySnapshot::failed(
        core.generation,
        revision,
        format!("{source} topology input failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::snapshot::CoreLifecycle;
    use infiltrator_contract::traffic_topology::TrafficTopologyStatus;

    fn core(lifecycle: CoreLifecycle) -> CoreSnapshot {
        CoreSnapshot {
            lifecycle,
            generation: 7,
            session_token: None,
            revision: 9,
            proxy_mode: None,
            core_version: None,
            sampled_at_epoch_ms: None,
            failure: None,
            upload_bps: 100.0,
            download_bps: 200.0,
            active_connections: 0,
            memory_bytes: None,
            watchdog: Default::default(),
        }
    }

    #[test]
    fn missing_gateway_is_typed_unsupported() {
        let snapshot =
            TrafficTopologyApplication.project(&core(CoreLifecycle::Running), None, None, None);
        assert_eq!(snapshot.status, TrafficTopologyStatus::Unsupported);
        assert!(!snapshot.is_flowing());
    }

    #[test]
    fn stopped_core_does_not_reuse_traffic_as_a_live_topology() {
        let snapshot =
            TrafficTopologyApplication.project(&core(CoreLifecycle::Stopped), None, None, None);
        assert_eq!(snapshot.status, TrafficTopologyStatus::Unknown);
        assert!(!snapshot.is_drawable());
    }
}
