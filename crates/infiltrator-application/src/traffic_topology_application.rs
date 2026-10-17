//! Application seam for the shared live traffic topology.

use crate::runtime_configuration_application::RuntimeConfigurationReadout;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_contract::traffic_topology::TrafficTopologySnapshot;
use infiltrator_domain::traffic_topology::{TrafficTopologyInput, derive};
use infiltrator_ports::connection::ConnectionObservationSnapshot;
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_configuration::ObservedValue;
use lurker_core_contract::error::Failure;
use lurker_core_contract::outbound::inventory::OutboundObservationSnapshot;

/// Converts composed controller observations into one immutable topology
/// snapshot. UI surfaces never call a gateway or infer the routing chain.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrafficTopologyApplication;

impl TrafficTopologyApplication {
    pub fn project(
        &self,
        core: &CoreSnapshot,
        config: Option<&Result<RuntimeConfigurationReadout, Failure>>,
        observation: Option<&Result<OutboundObservationSnapshot, Failure>>,
        connections: Option<&Result<ConnectionObservationSnapshot, PortError>>,
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
        let Some(observation) = observation else {
            return TrafficTopologySnapshot::unsupported(
                core.generation,
                revision,
                "outbound inventory source is not composed",
            );
        };
        let Some(connections) = connections else {
            return TrafficTopologySnapshot::unsupported(
                core.generation,
                revision,
                "connection port is not composed",
            );
        };

        let config = match config {
            Ok(config) => config,
            Err(failure) => return failed_failure(core, revision, "runtime config", failure),
        };
        let observation = match observation {
            Ok(observation) => observation,
            Err(failure) => {
                return TrafficTopologySnapshot::failed(
                    core.generation,
                    revision,
                    format!(
                        "outbound inventory topology input failed: {}",
                        failure.message
                    ),
                );
            }
        };
        let connections = match connections {
            Ok(connections) => connections,
            Err(error) => return failed(core, revision, "connection list", error),
        };

        let Some(scope) = core.scope() else {
            return TrafficTopologySnapshot::unavailable(
                core.generation,
                revision,
                "active core scope is unavailable",
            );
        };
        if observation.inventory().scope() != &scope {
            return TrafficTopologySnapshot::unavailable(
                core.generation,
                revision,
                "outbound observation belongs to a different core scope",
            );
        }
        if connections.scope() != &scope {
            return TrafficTopologySnapshot::unavailable(
                core.generation,
                revision,
                "connection observation belongs to a different core scope",
            );
        }
        if config.tun.scope != scope
            || config.lan_sharing.scope != scope
            || config.sniffer.scope != scope
            || config.proxy_ports.scope != scope
        {
            return TrafficTopologySnapshot::unavailable(
                core.generation,
                revision,
                "runtime configuration observation belongs to a different Core scope",
            );
        }
        let tun_enabled = reported(&config.tun.value.enabled);
        let tun_stack = reported(&config.tun.value.stack);
        let mixed_port = reported(&config.proxy_ports.value.mixed_port);
        let http_port = reported(&config.proxy_ports.value.http_port);
        let sniffer_enabled = reported(&config.sniffer.value.enabled);

        derive(TrafficTopologyInput {
            generation: core.generation,
            revision,
            lifecycle: core.lifecycle.clone(),
            upload_bps: core.upload_bps,
            download_bps: core.download_bps,
            tun_enabled,
            tun_stack: tun_stack.as_deref(),
            mixed_port,
            http_port,
            sniffer_enabled,
            connection_scope: &scope,
            connections: &connections.snapshot().connections,
            outbounds: observation,
        })
    }
}

fn reported<T: Clone>(value: &ObservedValue<T>) -> Option<T> {
    match value {
        ObservedValue::Unreported => None,
        ObservedValue::Reported(value) => Some(value.clone()),
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

fn failed_failure(
    core: &CoreSnapshot,
    revision: u64,
    source: &str,
    failure: &Failure,
) -> TrafficTopologySnapshot {
    TrafficTopologySnapshot::failed(
        core.generation,
        revision,
        format!("{source} topology input failed: {}", failure.message),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::snapshot::CoreLifecycle;
    use infiltrator_contract::traffic_topology::TrafficTopologyStatus;

    fn core(lifecycle: CoreLifecycle) -> CoreSnapshot {
        CoreSnapshot {
            identity: None,
            lifecycle,
            generation: 7,
            session_token: None,
            revision: 9,
            routing_intent: Default::default(),
            core_version: None,
            sampled_at_epoch_ms: None,
            failure: None,
            upload_bps: 100.0,
            download_bps: 200.0,
            active_connections: 0,
            watchdog: Default::default(),
        }
    }

    #[test]
    fn missing_connection_port_is_typed_unsupported() {
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
