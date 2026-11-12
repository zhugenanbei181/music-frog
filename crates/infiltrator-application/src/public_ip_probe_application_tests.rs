use super::*;
use async_trait::async_trait;
use infiltrator_contract::public_ip::PublicIpProbeStatus;
use infiltrator_contract::public_ip_probe::{PublicIpObservation, PublicIpProbeRequest};
use infiltrator_contract::snapshot::{CoreSessionObservation, CoreSnapshot};
use infiltrator_ports::core_snapshot_source::CoreSnapshotSource;
use lurker_core_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use tokio::sync::Notify;
use tokio::task::yield_now;

struct StaticCore(CoreSnapshot);

impl CoreSnapshotSource for StaticCore {
    fn core_snapshot(&self) -> CoreSnapshot {
        self.0.clone()
    }
}

fn owner(port: Option<Arc<dyn PublicIpProbePort>>, core: CoreSnapshot) -> PublicIpProbeApplication {
    PublicIpProbeApplication::new(port).with_core_snapshot_source(Arc::new(StaticCore(core)))
}

struct ScriptedPort {
    calls: Mutex<Vec<PublicIpProbeRequest>>,
    answers: Mutex<Vec<PublicIpProbeAnswer>>,
}

impl ScriptedPort {
    fn new(answers: Vec<PublicIpProbeAnswer>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            answers: Mutex::new(answers),
        })
    }
}

#[async_trait]
impl PublicIpProbePort for ScriptedPort {
    async fn observe(&self, request: PublicIpProbeRequest) -> PublicIpProbeAnswer {
        self.calls.lock().expect("probe request lock").push(request);
        self.answers.lock().expect("probe answer lock").remove(0)
    }
}

struct GatedPort {
    entered: Arc<AtomicUsize>,
    release: Arc<Notify>,
}

#[async_trait]
impl PublicIpProbePort for GatedPort {
    async fn observe(&self, request: PublicIpProbeRequest) -> PublicIpProbeAnswer {
        self.entered.fetch_add(1, Ordering::SeqCst);
        self.release.notified().await;
        PublicIpProbeAnswer::Succeeded {
            stamp: request.stamp,
            observation: observation(),
        }
    }
}

fn core(lifecycle: CoreLifecycle) -> CoreSnapshot {
    CoreSnapshot {
        session: CoreSessionObservation::Unknown,
        identity: None,
        lifecycle,
        generation: 4,
        revision: 9,
        session_token: None,
        routing_intent: Default::default(),
        core_version: None,
        sampled_at_epoch_ms: None,
        failure: None,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        watchdog: Default::default(),
    }
}

fn proxy_core() -> CoreSnapshot {
    let mut snapshot = core(CoreLifecycle::Running);
    snapshot.routing_intent = RoutingIntentSnapshot {
        current: Some(RoutingIntent::RouteAllThroughProxy),
        status: RoutingIntentStatus::Ready,
        failure: None,
    };
    snapshot
}

fn observation() -> PublicIpObservation {
    PublicIpObservation {
        ip: "203.0.113.7".to_owned(),
        country: Some("HK".to_owned()),
        region: None,
        city: Some("Hong Kong".to_owned()),
        provider: "api.ipify.org".to_owned(),
        checked_at_epoch_ms: 1_741_300_000_000,
    }
}

fn succeeded(stamp: PublicIpProbeStamp) -> PublicIpProbeAnswer {
    PublicIpProbeAnswer::Succeeded {
        stamp,
        observation: observation(),
    }
}

#[tokio::test]
async fn refresh_publishes_probing_then_terminal_readback() {
    let stamp = PublicIpProbeStamp {
        request_id: 1,
        core_generation: 4,
        revision: 9,
        route: PublicIpProbeRouteStamp::Direct,
    };
    let port = ScriptedPort::new(vec![succeeded(stamp)]);
    let application = owner(Some(port.clone()), core(CoreLifecycle::Running));
    assert_eq!(application.snapshot().status, PublicIpProbeStatus::Unknown);

    application.refresh(PublicIpRefreshRoute::Direct).await;

    let snapshot = application.snapshot();
    assert_eq!(snapshot.status, PublicIpProbeStatus::Ready);
    assert_eq!(snapshot.generation, 4);
    assert_eq!(snapshot.revision, 9);
    assert_eq!(snapshot.ip.as_deref(), Some("203.0.113.7"));
    assert_eq!(snapshot.checked_at_epoch_ms, Some(1_741_300_000_000));
    let calls = port.calls.lock().expect("probe request lock");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].stamp, stamp);
}

#[tokio::test]
async fn stale_answer_cannot_overwrite_the_active_readback() {
    let first = PublicIpProbeStamp {
        request_id: 1,
        core_generation: 4,
        revision: 9,
        route: PublicIpProbeRouteStamp::Direct,
    };
    let stale = PublicIpProbeStamp {
        request_id: 0,
        ..first
    };
    let port = ScriptedPort::new(vec![succeeded(stale)]);
    let application = owner(Some(port.clone()), core(CoreLifecycle::Running));

    application.refresh(PublicIpRefreshRoute::Direct).await;
    let after_stale = application.snapshot();
    assert_eq!(after_stale.status, PublicIpProbeStatus::Unknown);
    assert_eq!(after_stale.ip, None);
    assert_eq!(after_stale.failure, None);

    let second = PublicIpProbeStamp {
        request_id: 2,
        ..first
    };
    port.answers
        .lock()
        .expect("probe answer lock")
        .push(succeeded(second));
    application.refresh(PublicIpRefreshRoute::Direct).await;
    let settled = application.snapshot();
    assert_eq!(settled.status, PublicIpProbeStatus::Ready);
    assert_eq!(settled.ip.as_deref(), Some("203.0.113.7"));
}

#[tokio::test]
async fn duplicate_refresh_does_not_call_the_port_twice() {
    let entered = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let port = Arc::new(GatedPort {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    });
    let application = owner(Some(port), core(CoreLifecycle::Running));
    let duplicate = application.clone();
    let first =
        tokio::spawn(async move { application.refresh(PublicIpRefreshRoute::Direct).await });
    while entered.load(Ordering::SeqCst) == 0 {
        yield_now().await;
    }
    assert_eq!(duplicate.snapshot().status, PublicIpProbeStatus::Probing);

    let duplicate_refresh =
        tokio::spawn(async move { duplicate.refresh(PublicIpRefreshRoute::Direct).await });
    duplicate_refresh.await.expect("duplicate refresh task");
    assert_eq!(entered.load(Ordering::SeqCst), 1);
    release.notify_one();
    first.await.expect("first refresh task");
}

#[tokio::test]
async fn port_failure_is_a_typed_terminal_readback() {
    let application = owner(None, core(CoreLifecycle::Running));
    application.refresh(PublicIpRefreshRoute::Direct).await;
    let snapshot = application.snapshot();
    assert_eq!(snapshot.status, PublicIpProbeStatus::Failed);
    assert!(
        snapshot
            .failure
            .as_deref()
            .is_some_and(|reason| reason.contains(NO_PROBE_PORT_REASON))
    );
}

#[tokio::test]
async fn stopped_core_fails_without_calling_the_host_port() {
    let stamp = PublicIpProbeStamp {
        request_id: 1,
        core_generation: 4,
        revision: 9,
        route: PublicIpProbeRouteStamp::Direct,
    };
    let port = ScriptedPort::new(vec![succeeded(stamp)]);
    let application = owner(Some(port.clone()), core(CoreLifecycle::Stopped));
    application.refresh(PublicIpRefreshRoute::Direct).await;
    assert_eq!(application.snapshot().status, PublicIpProbeStatus::Failed);
    assert!(port.calls.lock().expect("probe request lock").is_empty());
}

#[tokio::test]
async fn route_is_stamped_from_the_core_routing_intent() {
    let application = PublicIpProbeApplication::unconfigured()
        .with_core_snapshot_source(Arc::new(StaticCore(proxy_core())));
    assert_eq!(
        application.refresh_route(),
        PublicIpRefreshRoute::RuntimeHttpProxy
    );
    application.refresh(application.refresh_route()).await;
    assert_eq!(application.snapshot().status, PublicIpProbeStatus::Failed);
    assert!(
        application
            .snapshot()
            .failure
            .as_deref()
            .is_some_and(|reason| reason.contains(NO_PROBE_PORT_REASON))
    );
}
