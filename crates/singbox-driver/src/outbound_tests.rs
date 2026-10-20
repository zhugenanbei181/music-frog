//! test-intent: behavior

use crate::api::http::SingBoxClashMethod;
use crate::outbound::SingBoxOutboundAdapter;
use crate::test_support::{
    FixedScope, ScriptedHttpPort, client, enqueue_inventory_observation, group_proxy_response,
    outbound_id, proxy_response, scope, version_response,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreGeneration, EngineId};
use lurker_core_contract::outbound::details::OutboundDetailsState;
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_contract::outbound::inventory::{
    OutboundInventoryState, OutboundProbeState, OutboundSelectionState, SelectionAvailability,
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::{OutboundPort, OutboundProbeRequest, ProbeTarget};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn selector_inventory_and_put_require_scope_revision_and_exact_readback() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(8);
    let owner = FixedScope::new(scope.clone());
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(client(http.clone()), Arc::new(owner));

    let inventory = adapter.inventory(scope.clone()).await.unwrap();
    assert_eq!(inventory.revision(), SnapshotRevision::new(1));
    let OutboundInventoryState::Ready { collections, .. } = inventory.inventory() else {
        panic!("sing-box should report the real selector fixture");
    };
    assert_eq!(collections.len(), 1, "synthetic GLOBAL is excluded");
    assert_eq!(
        collections[0].availability,
        SelectionAvailability::Selectable
    );
    assert_eq!(
        collections[0].selection,
        OutboundSelectionState::Ready {
            selected: outbound_id("proxy-a")
        }
    );

    http.push_json(200, version_response());
    http.push_json(200, proxy_response("proxy-a"));
    http.push_json(200, version_response());
    http.push_empty(204);
    http.push_json(200, proxy_response("proxy-b"));
    http.push_json(200, version_response());
    let selected = adapter
        .select(
            scope,
            inventory.revision(),
            outbound_id("selector #1"),
            outbound_id("proxy-b"),
        )
        .await
        .unwrap();
    assert_eq!(selected.revision(), SnapshotRevision::new(2));
    let OutboundInventoryState::Ready { collections, .. } = selected.inventory() else {
        panic!("readback should retain selector inventory");
    };
    assert_eq!(
        collections[0].selection,
        OutboundSelectionState::Ready {
            selected: outbound_id("proxy-b")
        }
    );
    let requests = http.requests();
    let put = requests
        .iter()
        .find(|request| request.method == SingBoxClashMethod::Put)
        .unwrap();
    assert_eq!(put.url, "http://127.0.0.1:9090/proxies/selector%20%231");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(put.body.as_ref().unwrap()).unwrap(),
        serde_json::json!({"name":"proxy-b"})
    );
    assert_eq!(http.remaining_responses(), 0);
}

#[tokio::test]
async fn urltest_is_automatic_and_unknown_group_never_becomes_selectable() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(5);
    http.push_json(200, version_response());
    http.push_json(200, group_proxy_response());
    http.push_json(200, version_response());
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();
    let OutboundInventoryState::Ready { collections, .. } = inventory.inventory() else {
        panic!("known and unknown groups should be observed");
    };
    let automatic = collections
        .iter()
        .find(|collection| collection.id.value() == "automatic")
        .unwrap();
    assert_eq!(automatic.availability, SelectionAvailability::Automatic);
    let unknown = collections
        .iter()
        .find(|collection| collection.id.value() == "future-group")
        .unwrap();
    assert_eq!(unknown.availability, SelectionAvailability::Unknown);

    http.push_json(200, version_response());
    http.push_json(200, group_proxy_response());
    http.push_json(200, version_response());
    let error = adapter
        .select(
            scope,
            inventory.revision(),
            outbound_id("automatic"),
            outbound_id("proxy-a"),
        )
        .await
        .expect_err("URLTest is an automatic group");
    assert!(matches!(
        error,
        PortError::Rejected(Failure {
            code: ErrorCode::Unsupported,
            ..
        })
    ));
    assert!(
        http.requests()
            .iter()
            .all(|request| request.method != SingBoxClashMethod::Put)
    );
}

#[tokio::test]
async fn stale_selector_revision_and_foreign_scope_refuse_write_before_put() {
    let http = Arc::new(ScriptedHttpPort::default());
    let active = scope(2);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(active.clone())),
    );
    let inventory = adapter.inventory(active.clone()).await.unwrap();

    let mut stale_scope = active.clone();
    stale_scope.generation = CoreGeneration::new(3);
    let stale_scope_error = adapter
        .inventory(stale_scope)
        .await
        .expect_err("generation mismatch must fail before controller HTTP");
    assert!(matches!(stale_scope_error, PortError::Rejected(_)));
    assert_eq!(http.requests().len(), 3);

    http.push_json(200, version_response());
    http.push_json(200, proxy_response("proxy-b"));
    http.push_json(200, version_response());
    let stale = adapter
        .select(
            active.clone(),
            inventory.revision(),
            outbound_id("selector #1"),
            outbound_id("proxy-a"),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        stale,
        PortError::Rejected(Failure {
            code: ErrorCode::InvalidState,
            ..
        })
    ));
    assert!(
        http.requests()
            .iter()
            .all(|request| request.method != SingBoxClashMethod::Put)
    );

    let mut foreign = scope(2);
    foreign.identity.engine = EngineId::new("mihomo").unwrap();
    let error = adapter
        .inventory(foreign)
        .await
        .expect_err("foreign engine must fail before HTTP");
    assert!(matches!(error, PortError::Rejected(_)));
    assert_eq!(http.remaining_responses(), 0);
}

#[tokio::test]
async fn sing_box_details_stay_explicitly_unsupported_after_the_revision_fence() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(6);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();

    for _ in 0..2 {
        http.push_json(200, version_response());
        http.push_json(200, proxy_response("proxy-a"));
        http.push_json(200, version_response());
    }
    let details = adapter
        .details(scope.clone(), inventory.revision())
        .await
        .unwrap();
    assert_eq!(details.state(), &OutboundDetailsState::Unsupported);
    assert_eq!(details.detail_revision(), SnapshotRevision::new(1));
    assert!(http.requests().iter().all(|request| {
        request.method == SingBoxClashMethod::Get && !request.url.contains("/delay")
    }));
}

#[tokio::test]
async fn the_probe_asks_the_engine_for_the_delay_and_reports_the_measured_latency() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(7);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();

    enqueue_probe_fence(&http);
    http.push_json(200, serde_json::json!({"delay": 42}));
    enqueue_probe_readback(&http);
    let probe = adapter
        .probe(
            scope.clone(),
            inventory.revision(),
            outbound_id("proxy-a"),
            probe_request(6_000),
        )
        .await
        .unwrap();

    assert_eq!(
        probe.state(),
        &OutboundProbeState::Measured { delay_ms: 42 }
    );
    assert_eq!(probe.scope(), &scope);
    assert_eq!(probe.outbound_id(), &outbound_id("proxy-a"));
    assert_eq!(probe.observed_inventory_revision(), inventory.revision());
    assert_eq!(probe.detail_revision(), SnapshotRevision::new(1));

    let delay = http
        .requests()
        .into_iter()
        .find(|request| request.url.contains("/delay"))
        .expect("the driver asks the engine for the delay");
    assert_eq!(delay.method, SingBoxClashMethod::Get);
    assert_eq!(
        delay.url,
        "http://127.0.0.1:9090/proxies/proxy-a/delay?url=https%3A%2F%2Fhealth.example.test%2Fcheck&timeout=6000"
    );
    // The transport keeps a slack behind the driver's own deadline, so the
    // driver's deadline stays the single authority over a late answer.
    assert_eq!(delay.timeout, Duration::from_secs(9));
    assert_eq!(http.remaining_responses(), 0);
}

#[tokio::test]
async fn the_engines_own_timeout_answer_stays_a_timeout() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(8);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();

    enqueue_probe_fence(&http);
    http.push_json(504, serde_json::json!({"message": "Timeout"}));
    enqueue_probe_readback(&http);
    let probe = adapter
        .probe(
            scope.clone(),
            inventory.revision(),
            outbound_id("proxy-a"),
            probe_request(4_000),
        )
        .await
        .unwrap();

    assert_eq!(probe.state(), &OutboundProbeState::TimedOut);
}

#[tokio::test]
async fn a_failed_delay_test_stays_a_failure_and_never_becomes_a_measurement() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(9);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();

    enqueue_probe_fence(&http);
    http.push_json(
        503,
        serde_json::json!({"message": "An error occurred in the delay test"}),
    );
    enqueue_probe_readback(&http);
    let probe = adapter
        .probe(
            scope.clone(),
            inventory.revision(),
            outbound_id("proxy-a"),
            probe_request(4_000),
        )
        .await
        .unwrap();

    assert!(
        matches!(
            probe.state(),
            OutboundProbeState::Failed { failure } if failure.code == ErrorCode::Network
        ),
        "a failed delay test must stay a typed failure"
    );
}

#[tokio::test]
async fn an_unknown_outbound_stays_a_failure() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(10);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();

    enqueue_probe_fence(&http);
    http.push_json(404, serde_json::json!({"message": "Resource not found"}));
    enqueue_probe_readback(&http);
    let probe = adapter
        .probe(
            scope.clone(),
            inventory.revision(),
            outbound_id("proxy-a"),
            probe_request(4_000),
        )
        .await
        .unwrap();

    assert!(
        matches!(
            probe.state(),
            OutboundProbeState::Failed { failure } if failure.code == ErrorCode::InvalidState
        ),
        "an unknown outbound must stay a typed failure"
    );
}

#[tokio::test]
async fn a_delay_route_that_does_not_answer_reports_the_capability_as_unsupported() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(11);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();

    enqueue_probe_fence(&http);
    http.push_text(404, "404 page not found");
    enqueue_probe_readback(&http);
    let probe = adapter
        .probe(
            scope.clone(),
            inventory.revision(),
            outbound_id("proxy-a"),
            probe_request(4_000),
        )
        .await
        .unwrap();

    assert_eq!(probe.state(), &OutboundProbeState::Unsupported);
}

#[tokio::test]
async fn a_body_that_is_not_a_measurement_stays_a_failure() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(12);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();

    enqueue_probe_fence(&http);
    http.push_text(200, "not a measurement");
    enqueue_probe_readback(&http);
    let probe = adapter
        .probe(
            scope.clone(),
            inventory.revision(),
            outbound_id("proxy-a"),
            probe_request(4_000),
        )
        .await
        .unwrap();

    assert!(
        matches!(
            probe.state(),
            OutboundProbeState::Failed { failure } if failure.code == ErrorCode::InvalidState
        ),
        "an unreadable measurement must stay a typed failure"
    );
}

#[tokio::test(start_paused = true)]
async fn a_controller_that_never_answers_is_a_timeout_and_never_a_measurement() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(13);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();

    enqueue_probe_fence(&http);
    http.push_hang();
    enqueue_probe_readback(&http);
    let probe = adapter
        .probe(
            scope.clone(),
            inventory.revision(),
            outbound_id("proxy-a"),
            probe_request(1_000),
        )
        .await
        .unwrap();

    assert_eq!(probe.state(), &OutboundProbeState::TimedOut);
}

#[tokio::test]
async fn a_transport_failure_stays_a_network_failure() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(14);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();

    enqueue_probe_fence(&http);
    http.push_failure();
    enqueue_probe_readback(&http);
    let probe = adapter
        .probe(
            scope.clone(),
            inventory.revision(),
            outbound_id("proxy-a"),
            probe_request(4_000),
        )
        .await
        .unwrap();

    assert!(
        matches!(
            probe.state(),
            OutboundProbeState::Failed { failure } if failure.code == ErrorCode::Network
        ),
        "a transport failure must stay a typed network failure"
    );
}

#[tokio::test]
async fn a_delay_timeout_the_endpoint_cannot_carry_is_refused_before_any_request() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(15);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();
    let requests_before = http.requests().len();

    let error = adapter
        .probe(
            scope.clone(),
            inventory.revision(),
            outbound_id("proxy-a"),
            probe_request(u64::from(i16::MAX as u32) + 1),
        )
        .await
        .expect_err("the delay endpoint carries a signed 16-bit millisecond count");
    assert!(matches!(
        error,
        PortError::Rejected(Failure {
            code: ErrorCode::InvalidInput,
            ..
        })
    ));
    assert_eq!(http.requests().len(), requests_before);
    assert!(
        http.requests()
            .iter()
            .all(|request| !request.url.contains("/delay"))
    );
}

#[tokio::test]
async fn a_stale_revision_is_refused_before_the_delay_request() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(16);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();
    assert_eq!(inventory.revision(), SnapshotRevision::new(1));

    // The fence checks the live scope before it compares the revision, so the
    // refusal happens without any delay request.
    http.push_json(200, version_response());
    let error = adapter
        .probe(
            scope.clone(),
            SnapshotRevision::new(0),
            outbound_id("proxy-a"),
            probe_request(1_000),
        )
        .await
        .expect_err("a stale revision is refused");
    assert!(matches!(
        error,
        PortError::Rejected(Failure {
            code: ErrorCode::InvalidState,
            ..
        })
    ));
    assert!(
        http.requests()
            .iter()
            .all(|request| !request.url.contains("/delay"))
    );
}

#[tokio::test]
async fn an_outbound_outside_the_observed_inventory_is_refused_before_the_delay_request() {
    let http = Arc::new(ScriptedHttpPort::default());
    let scope = scope(17);
    enqueue_inventory_observation(&http, "proxy-a");
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );
    let inventory = adapter.inventory(scope.clone()).await.unwrap();

    // The fence observes the inventory before the membership compare, so the
    // refusal happens without any delay request.
    enqueue_probe_fence(&http);
    let error = adapter
        .probe(
            scope.clone(),
            inventory.revision(),
            outbound_id("proxy-z"),
            probe_request(1_000),
        )
        .await
        .expect_err("an unobserved outbound is refused");
    assert!(matches!(
        error,
        PortError::Rejected(Failure {
            code: ErrorCode::InvalidState,
            ..
        })
    ));
    assert!(
        http.requests()
            .iter()
            .all(|request| !request.url.contains("/delay"))
    );
}

#[tokio::test]
async fn live_scope_version_mismatch_fails_before_proxy_inventory_read() {
    let http = Arc::new(ScriptedHttpPort::default());
    http.push_json(200, serde_json::json!({"version":"sing-box 1.14.4"}));
    let scope = scope(1);
    let adapter = SingBoxOutboundAdapter::new(
        client(http.clone()),
        Arc::new(FixedScope::new(scope.clone())),
    );

    let error = adapter.inventory(scope).await.unwrap_err();
    assert!(matches!(error, PortError::Rejected(_)));
    assert_eq!(http.requests().len(), 1);
    assert_eq!(http.requests()[0].url, "http://127.0.0.1:9090/version");
}

/// Scripts the requests the probe's revision fence makes.
fn enqueue_probe_fence(http: &ScriptedHttpPort) {
    http.push_json(200, version_response());
    http.push_json(200, proxy_response("proxy-a"));
    http.push_json(200, version_response());
}

/// Scripts the requests the probe's post-probe readback makes.
fn enqueue_probe_readback(http: &ScriptedHttpPort) {
    http.push_json(200, proxy_response("proxy-a"));
    http.push_json(200, version_response());
}

/// Builds one probe request for the shared health target.
fn probe_request(timeout_ms: u64) -> OutboundProbeRequest {
    OutboundProbeRequest::new(
        ProbeTarget::http_url("https://health.example.test/check").unwrap(),
        timeout_ms,
    )
    .unwrap()
}
