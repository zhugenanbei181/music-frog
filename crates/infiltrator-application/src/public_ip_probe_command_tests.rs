use crate::command_application::CommandApplication;
use crate::public_ip_probe_application::PublicIpProbeApplication;
use async_trait::async_trait;
use infiltrator_contract::capability::{Availability, CapabilitySnapshot, CapabilityStatus};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::public_ip::PublicIpProbeStatus;
use infiltrator_contract::public_ip_probe::{
    PublicIpObservation, PublicIpProbeAnswer, PublicIpProbeRequest, PublicIpProbeRouteStamp,
};
use infiltrator_contract::snapshot::{CoreSessionObservation, CoreSnapshot};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::core_snapshot_source::CoreSnapshotSource;
use infiltrator_ports::public_ip_probe::PublicIpProbePort;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};
use std::sync::{Arc, Mutex};

struct StaticCore(CoreSnapshot);

struct SupportedCapabilities;

impl CapabilityProvider for SupportedCapabilities {
    fn host_kind(&self) -> HostKind {
        HostKind::Desktop
    }

    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::new(
            HostKind::Desktop,
            1,
            vec![CapabilityStatus {
                capability: Capability::Speedtest,
                availability: Availability::Supported,
            }],
        )
    }
}

impl CoreSnapshotSource for StaticCore {
    fn core_snapshot(&self) -> CoreSnapshot {
        self.0.clone()
    }
}

struct EchoPort(Mutex<Vec<PublicIpProbeRequest>>);

impl EchoPort {
    fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(Vec::new())))
    }
}

#[async_trait]
impl PublicIpProbePort for EchoPort {
    async fn observe(&self, request: PublicIpProbeRequest) -> PublicIpProbeAnswer {
        self.0.lock().expect("probe request lock").push(request);
        PublicIpProbeAnswer::Succeeded {
            stamp: request.stamp,
            observation: PublicIpObservation {
                ip: "203.0.113.7".to_owned(),
                country: None,
                region: None,
                city: None,
                provider: "api.ipify.org".to_owned(),
                checked_at_epoch_ms: 1_741_300_000_000,
            },
        }
    }
}

fn live_core() -> CoreSnapshot {
    let mut core = CoreSnapshot {
        session: CoreSessionObservation::Unknown,
        identity: None,
        lifecycle: CoreLifecycle::Running,
        generation: 2,
        revision: 3,
        session_token: None,
        routing_intent: Default::default(),
        core_version: None,
        sampled_at_epoch_ms: None,
        failure: None,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        watchdog: Default::default(),
    };
    core.routing_intent = RoutingIntentSnapshot {
        current: Some(RoutingIntent::RouteAllThroughProxy),
        status: RoutingIntentStatus::Ready,
        failure: None,
    };
    core
}

#[tokio::test]
async fn refresh_public_ip_probe_reaches_a_terminal_typed_readback() {
    let port = EchoPort::new();
    let owner = PublicIpProbeApplication::new(Some(port.clone()))
        .with_core_snapshot_source(Arc::new(StaticCore(live_core())));
    let commands = CommandApplication::new()
        .with_host_capabilities(Arc::new(SupportedCapabilities))
        .with_public_ip_probe(owner.clone());

    commands
        .execute(CommandIntent::RefreshPublicIpProbe)
        .await
        .expect("public IP refresh must not be unsupported");

    let snapshot = owner.snapshot();
    assert_eq!(snapshot.status, PublicIpProbeStatus::Ready);
    assert_eq!(snapshot.ip.as_deref(), Some("203.0.113.7"));
    let requests = port.0.lock().expect("probe request lock");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].stamp.route,
        PublicIpProbeRouteStamp::ThroughRuntimeHttpProxy
    );
}
