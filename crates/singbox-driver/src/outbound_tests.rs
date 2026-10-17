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
async fn sing_box_details_and_delay_probe_are_explicitly_unsupported_after_revision_fence() {
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
    let request = OutboundProbeRequest::new(
        ProbeTarget::http_url("https://health.example.test/check").unwrap(),
        1000,
    )
    .unwrap();
    let probe = adapter
        .probe(
            scope.clone(),
            inventory.revision(),
            outbound_id("proxy-a"),
            request,
        )
        .await
        .unwrap();
    assert_eq!(probe.state(), &OutboundProbeState::Unsupported);
    assert!(http.requests().iter().all(|request| {
        request.method == SingBoxClashMethod::Get
            && !request.url.contains("/delay")
            && !request.url.contains("/proxies/proxy-a")
    }));
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
