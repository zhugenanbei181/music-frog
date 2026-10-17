use super::mapping::{map_details, map_inventory};
use super::{MihomoOutboundAdapter, OutboundObservationState, map_probe_result};
use crate::api::client::MihomoClient;
use crate::api::proxy::{ProxiesResponse, ProxyObservation};
use crate::error::MihomoError;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::outbound::{
    details::{
        OutboundCollectionKind, OutboundDetailsState, OutboundHealthObservation,
        OutboundObservation, OutboundProbeAvailability, OutboundProtocolObservation,
        OutboundRttStatistics, OutboundUdpSupport,
    },
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventoryState, OutboundProbeState, OutboundSelectionState, SelectionAvailability,
    },
};
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::{OutboundPort, OutboundProbeRequest, ProbeTarget};
use mockito::{Matcher, Server};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;

struct FixedScope(CoreScopeObservation);

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        self.0.clone()
    }
}

fn scope(engine: &str, generation: u64) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new(engine).expect("engine ID"),
            CoreInstanceId::new("primary").expect("instance ID"),
            None,
        ),
        generation: CoreGeneration::new(generation),
        session_token: None,
    }
}

fn outbound_id(value: &str) -> OutboundId {
    OutboundId::new(EngineId::new("mihomo").expect("engine ID"), value).expect("outbound ID")
}

fn adapter(server_url: &str, scope: CoreScope) -> MihomoOutboundAdapter {
    let client = Arc::new(MihomoClient::new(server_url, None).expect("Mihomo client"));
    MihomoOutboundAdapter::new(
        client,
        Arc::new(FixedScope(CoreScopeObservation::Observed(scope))),
    )
}

fn adapter_without_scope(server_url: &str) -> MihomoOutboundAdapter {
    let client = Arc::new(MihomoClient::new(server_url, None).expect("Mihomo client"));
    MihomoOutboundAdapter::new(
        client,
        Arc::new(FixedScope(CoreScopeObservation::Unavailable)),
    )
}

fn probe_request(timeout_ms: u64) -> OutboundProbeRequest {
    OutboundProbeRequest::new(
        ProbeTarget::http_url("https://health.example.test/check").expect("probe target"),
        timeout_ms,
    )
    .expect("probe request")
}

#[test]
fn mihomo_probe_adapter_accepts_timeouts_only_within_its_signed_limit() {
    let accepted = probe_request(12_345);
    assert_eq!(
        super::mihomo_probe_values(&accepted).unwrap(),
        ("https://health.example.test/check".to_owned(), 12_345)
    );

    let beyond_adapter_limit = probe_request(32_768);
    assert!(matches!(
        super::mihomo_probe_values(&beyond_adapter_limit),
        Err(PortError::Rejected(failure)) if failure.code == ErrorCode::InvalidInput
    ));
}

fn proxy_payload(selected: &str) -> String {
    serde_json::json!({
        "proxies": {
            "selector": {
                "type": "Selector",
                "name": "selector",
                "now": selected,
                "all": ["proxy-a", "proxy-b"],
                "history": []
            },
            "proxy-a": {"type": "Direct", "name": "proxy-a"},
            "proxy-b": {"type": "Direct", "name": "proxy-b"}
        }
    })
    .to_string()
}

fn proxies_from_payload(payload: &str) -> HashMap<String, ProxyObservation> {
    serde_json::from_str::<ProxiesResponse>(payload)
        .expect("valid controller proxy payload")
        .proxies
}

#[test]
fn known_group_types_map_to_neutral_availability_and_unknown_types_fail_closed() {
    let payload = serde_json::json!({
        "proxies": {
            "selector": {"type": "Selector", "name": "selector", "now": "proxy-a", "all": ["proxy-a"]},
            "url-test": {"type": "URLTest", "name": "url-test", "now": "proxy-a", "all": ["proxy-a"]},
            "fallback": {"type": "Fallback", "name": "fallback", "now": "proxy-a", "all": ["proxy-a"]},
            "load-balance": {"type": "LoadBalance", "name": "load-balance", "now": "proxy-a", "all": ["proxy-a"]},
            "relay": {"type": "Relay", "name": "relay", "now": "proxy-a", "all": ["proxy-a"]},
            "mystery": {"type": "MysteryGroup", "name": "mystery", "now": "proxy-a", "all": ["proxy-a"]},
            "proxy-a": {"type": "Direct", "name": "proxy-a"}
        }
    });
    let proxies = proxies_from_payload(&payload.to_string());
    let inventory = map_inventory(&proxies).expect("map inventory");
    let OutboundInventoryState::Ready { collections, .. } = inventory else {
        panic!("Mihomo reported outbounds");
    };
    let availability = |name: &str| {
        collections
            .iter()
            .find(|collection| collection.id.value() == name)
            .expect("group collection")
            .availability
    };
    assert_eq!(availability("selector"), SelectionAvailability::Selectable);
    assert_eq!(availability("url-test"), SelectionAvailability::Automatic);
    assert_eq!(availability("fallback"), SelectionAvailability::Automatic);
    assert_eq!(
        availability("load-balance"),
        SelectionAvailability::Automatic
    );
    assert_eq!(availability("relay"), SelectionAvailability::ReadOnly);
    assert_eq!(availability("mystery"), SelectionAvailability::Unknown);
    let unknown = collections
        .iter()
        .find(|collection| collection.id.value() == "mystery")
        .expect("unknown collection");
    assert_eq!(unknown.selection, OutboundSelectionState::Unknown);
}

#[test]
fn mihomo_details_map_only_runtime_reported_facts_and_keep_unknowns_explicit() {
    let payload = serde_json::json!({
        "proxies": {
            "leaf": {
                "type": "Shadowsocks", "name": "leaf", "server": "node.example.test",
                "port": 443, "cipher": "aes-128-gcm", "udp": true, "alive": false,
                "delay": 0,
                "history": [
                    {"time": "2026-10-09T10:00:00Z", "delay": 5},
                    {"time": "2026-10-09T10:01:00Z", "delay": 0},
                    {"time": "2026-10-09T10:02:00Z", "delay": 15}
                ]
            },
            "sparse": {"type": "Vmess", "name": "sparse"},
            "selector": {
                "type": "Selector", "name": "selector", "now": "leaf", "all": ["leaf"]
            },
            "mystery": {"type": "Mystery", "name": "mystery"}
        }
    });
    let proxies = proxies_from_payload(&payload.to_string());
    let details = map_details(&proxies).expect("same controller map provides details");
    let OutboundDetailsState::Ready { details } = details else {
        panic!("non-empty /proxies map provides details");
    };
    let leaf = details
        .iter()
        .find(|detail| detail.id.value() == "leaf")
        .expect("leaf details");
    assert!(matches!(
        &leaf.protocol,
        OutboundProtocolObservation::Reported(id) if id.engine_id().as_str() == "mihomo" && id.value() == "Shadowsocks"
    ));
    assert_eq!(leaf.protocol_display_name.as_deref(), Some("Shadowsocks"));
    assert_eq!(leaf.cipher_display_name.as_deref(), Some("aes-128-gcm"));
    assert_eq!(leaf.udp, OutboundUdpSupport::Supported);
    assert_eq!(leaf.health, OutboundHealthObservation::Unhealthy);
    assert_eq!(leaf.current_latency_ms, OutboundObservation::Observed(0));
    assert!(matches!(
        &leaf.latency_history,
        OutboundObservation::Observed(samples) if samples.len() == 3
    ));
    assert!(matches!(
        &leaf.cipher,
        OutboundObservation::Observed(cipher)
            if cipher.engine_id().as_str() == "mihomo" && cipher.value() == "aes-128-gcm"
    ));
    assert_eq!(
        leaf.rtt,
        OutboundObservation::Observed(OutboundRttStatistics {
            min_ms: Some(5),
            max_ms: Some(15),
            avg_ms: Some(10),
            valid_count: 2,
        })
    );
    assert_eq!(
        leaf.probe_availability,
        OutboundProbeAvailability::Supported
    );

    let sparse = details
        .iter()
        .find(|detail| detail.id.value() == "sparse")
        .expect("sparse details");
    assert_eq!(sparse.udp, OutboundUdpSupport::Unknown);
    assert_eq!(sparse.health, OutboundHealthObservation::Unknown);
    assert_eq!(sparse.endpoint, OutboundObservation::Unknown);
    assert_eq!(sparse.latency_history, OutboundObservation::Unknown);
    assert_eq!(sparse.cipher, OutboundObservation::Unknown);

    let selector = details
        .iter()
        .find(|detail| detail.id.value() == "selector")
        .expect("group details");
    assert_eq!(
        selector.collection_kind,
        OutboundObservation::Observed(OutboundCollectionKind::ManualSelection)
    );
    assert_eq!(
        selector.probe_availability,
        OutboundProbeAvailability::Unsupported
    );
    assert_eq!(selector.cipher, OutboundObservation::Unsupported);

    let mystery = details
        .iter()
        .find(|detail| detail.id.value() == "mystery")
        .expect("unknown details");
    assert_eq!(
        mystery.probe_availability,
        OutboundProbeAvailability::Unknown
    );
    assert_eq!(mystery.collection_kind, OutboundObservation::Unsupported);
}

#[test]
fn detail_only_runtime_changes_advance_detail_revision_without_staling_inventory() {
    let scope = scope("mihomo", 3);
    let first = serde_json::json!({
        "proxies": {
            "leaf": {
                "type": "Shadowsocks", "name": "leaf", "server": "node.example.test",
                "port": 443, "cipher": "aes-128-gcm", "udp": true, "alive": true,
                "history": [{"time": "2026-10-09T10:00:00Z", "delay": 5}]
            }
        }
    });
    let second = serde_json::json!({
        "proxies": {
            "leaf": {
                "type": "Shadowsocks", "name": "leaf", "server": "node.example.test",
                "port": 443, "cipher": "aes-128-gcm", "udp": true, "alive": true,
                "history": [
                    {"time": "2026-10-09T10:00:00Z", "delay": 5},
                    {"time": "2026-10-09T10:01:00Z", "delay": 8}
                ]
            }
        }
    });
    let mut observed = OutboundObservationState::default();
    observed
        .observe(
            &scope,
            map_inventory(&proxies_from_payload(&first.to_string())).expect("first inventory"),
            map_details(&proxies_from_payload(&first.to_string())).expect("first details"),
        )
        .expect("first observation");

    observed
        .observe(
            &scope,
            map_inventory(&proxies_from_payload(&second.to_string())).expect("second inventory"),
            map_details(&proxies_from_payload(&second.to_string())).expect("second details"),
        )
        .expect("updated details observation");

    assert_eq!(observed.inventory_revision, SnapshotRevision::new(1));
    assert_eq!(observed.detail_revision, SnapshotRevision::new(2));
}

#[tokio::test]
async fn one_observation_cycle_reads_inventory_and_details_from_one_proxies_sample() {
    let mut server = Server::new_async().await;
    let proxies = server
        .mock("GET", "/proxies")
        .expect(1)
        .with_status(200)
        .with_body(proxy_payload("proxy-a"))
        .create_async()
        .await;
    let active_scope = scope("mihomo", 12);
    let adapter = adapter(&server.url(), active_scope.clone());

    let observation = OutboundPort::observation(&adapter, active_scope.clone())
        .await
        .expect("atomic outbound observation succeeds");

    assert_eq!(observation.inventory().scope(), &active_scope);
    assert_eq!(observation.details().scope(), &active_scope);
    assert_eq!(observation.inventory().revision(), SnapshotRevision::new(1));
    assert_eq!(
        observation.details().detail_revision(),
        SnapshotRevision::new(1)
    );
    let OutboundInventoryState::Ready { entries, .. } = observation.inventory().inventory() else {
        panic!("inventory is observed");
    };
    let OutboundDetailsState::Ready { details } = observation.details().state() else {
        panic!("details are observed");
    };
    assert_eq!(entries.len(), details.len());
    proxies.assert_async().await;
}

#[test]
fn absent_group_members_are_not_synthesized_as_observed_entries() {
    let payload = r#"{"proxies":{"selector":{"type":"Selector","name":"selector","now":"not-present","all":["not-present"]}}}"#;
    let proxies = proxies_from_payload(payload);
    let failure = map_inventory(&proxies)
        .expect_err("missing controller members are invalid wire observations");
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert!(failure.message.contains("absent from /proxies"));
}

#[tokio::test]
async fn reqwest_timeout_maps_to_timed_out() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("local listener");
    let address = listener.local_addr().expect("listener address");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("request connection");
        sleep(Duration::from_millis(100)).await;
        drop(stream);
    });
    let error = reqwest::Client::new()
        .get(format!("http://{address}/delay"))
        .timeout(Duration::from_millis(20))
        .send()
        .await
        .expect_err("the local endpoint withholds its response past the deadline");
    assert!(error.is_timeout());
    assert_eq!(
        map_probe_result(Err(MihomoError::Http(error))),
        OutboundProbeState::TimedOut
    );
    server.await.expect("mock server task");
}

#[tokio::test]
async fn inventory_rejects_a_scope_mismatch_before_controller_io() {
    let mut server = Server::new_async().await;
    let unused = server
        .mock("GET", "/proxies")
        .expect(0)
        .with_status(200)
        .with_body(proxy_payload("proxy-a"))
        .create_async()
        .await;
    let adapter = adapter(&server.url(), scope("mihomo", 5));

    let error = OutboundPort::inventory(&adapter, scope("mihomo", 4))
        .await
        .expect_err("wrong generation is refused");
    assert!(
        matches!(error, PortError::Rejected(failure) if failure.code == ErrorCode::InvalidState)
    );
    unused.assert_async().await;
}

#[tokio::test]
async fn unavailable_owner_scope_refuses_inventory_and_selection_before_controller_io() {
    let mut server = Server::new_async().await;
    let unused_get = server
        .mock("GET", "/proxies")
        .expect(0)
        .with_status(200)
        .with_body(proxy_payload("proxy-a"))
        .create_async()
        .await;
    let unused_put = server
        .mock("PUT", "/proxies/selector")
        .expect(0)
        .with_status(204)
        .create_async()
        .await;
    let adapter = adapter_without_scope(&server.url());
    let requested_scope = scope("mihomo", 9);

    let inventory_error = OutboundPort::inventory(&adapter, requested_scope.clone())
        .await
        .expect_err("unavailable owner scope refuses inventory");
    assert!(matches!(
        inventory_error,
        PortError::Rejected(failure) if failure.code == ErrorCode::NotReady
    ));
    let selection_error = OutboundPort::select(
        &adapter,
        requested_scope,
        SnapshotRevision::new(1),
        outbound_id("selector"),
        outbound_id("proxy-b"),
    )
    .await
    .expect_err("unavailable owner scope refuses selection");
    assert!(matches!(
        selection_error,
        PortError::Rejected(failure) if failure.code == ErrorCode::NotReady
    ));
    unused_get.assert_async().await;
    unused_put.assert_async().await;
}

#[tokio::test]
async fn selection_checks_revision_before_put_and_returns_post_write_readback() {
    let mut server = Server::new_async().await;
    let before = server
        .mock("GET", "/proxies")
        .expect(2)
        .with_status(200)
        .with_body(proxy_payload("proxy-a"))
        .create_async()
        .await;
    let after = server
        .mock("GET", "/proxies")
        .expect(1)
        .with_status(200)
        .with_body(proxy_payload("proxy-b"))
        .create_async()
        .await;
    let put = server
        .mock("PUT", "/proxies/selector")
        .expect(1)
        .match_body(Matcher::JsonString(r#"{"name":"proxy-b"}"#.to_owned()))
        .with_status(204)
        .create_async()
        .await;
    let active_scope = scope("mihomo", 9);
    let adapter = adapter(&server.url(), active_scope.clone());
    let observed = OutboundPort::inventory(&adapter, active_scope.clone())
        .await
        .expect("inventory");
    assert_eq!(observed.revision(), SnapshotRevision::new(1));

    let selected = OutboundPort::select(
        &adapter,
        active_scope.clone(),
        observed.revision(),
        outbound_id("selector"),
        outbound_id("proxy-b"),
    )
    .await
    .expect("controller readback confirms selection");

    assert_eq!(selected.scope(), &active_scope);
    assert!(selected.revision() > observed.revision());
    let OutboundInventoryState::Ready { collections, .. } = selected.inventory() else {
        panic!("readback is ready");
    };
    assert_eq!(
        collections[0].selection,
        OutboundSelectionState::Ready {
            selected: outbound_id("proxy-b")
        }
    );
    before.assert_async().await;
    after.assert_async().await;
    put.assert_async().await;
}

#[tokio::test]
async fn changed_inventory_advances_revision_and_refuses_stale_selection_without_put() {
    let mut server = Server::new_async().await;
    let initial = server
        .mock("GET", "/proxies")
        .expect(1)
        .with_status(200)
        .with_body(proxy_payload("proxy-a"))
        .create_async()
        .await;
    let changed = server
        .mock("GET", "/proxies")
        .expect(1)
        .with_status(200)
        .with_body(proxy_payload("proxy-b"))
        .create_async()
        .await;
    let put = server
        .mock("PUT", "/proxies/selector")
        .expect(0)
        .with_status(204)
        .create_async()
        .await;
    let active_scope = scope("mihomo", 2);
    let adapter = adapter(&server.url(), active_scope.clone());
    let observed = OutboundPort::inventory(&adapter, active_scope.clone())
        .await
        .expect("initial inventory");

    let error = OutboundPort::select(
        &adapter,
        active_scope,
        observed.revision(),
        outbound_id("selector"),
        outbound_id("proxy-b"),
    )
    .await
    .expect_err("controller drift invalidates the old snapshot");

    assert!(
        matches!(error, PortError::Rejected(failure) if failure.code == ErrorCode::InvalidState)
    );
    initial.assert_async().await;
    changed.assert_async().await;
    put.assert_async().await;
}

#[tokio::test]
async fn probe_uses_typed_target_and_returns_a_new_scoped_observation() {
    let mut server = Server::new_async().await;
    let inventory = server
        .mock("GET", "/proxies")
        .expect(3)
        .with_status(200)
        .with_body(proxy_payload("proxy-a"))
        .create_async()
        .await;
    let delay = server
        .mock("GET", "/proxies/proxy-a/delay")
        .expect(1)
        .match_query("url=https%3A%2F%2Fhealth.example.test%2Fcheck&timeout=6000")
        .with_status(200)
        .with_body(r#"{"delay":0}"#)
        .create_async()
        .await;
    let active_scope = scope("mihomo", 6);
    let adapter = adapter(&server.url(), active_scope.clone());
    let observed = OutboundPort::inventory(&adapter, active_scope.clone())
        .await
        .expect("inventory");
    let result = OutboundPort::probe(
        &adapter,
        active_scope.clone(),
        observed.revision(),
        outbound_id("proxy-a"),
        probe_request(6_000),
    )
    .await
    .expect("delay result is scoped and revisioned");

    assert_eq!(result.scope(), &active_scope);
    assert_eq!(result.observed_inventory_revision(), observed.revision());
    assert!(result.detail_revision() > SnapshotRevision::new(1));
    assert_eq!(
        result.state(),
        &OutboundProbeState::Measured { delay_ms: 0 }
    );
    inventory.assert_async().await;
    delay.assert_async().await;
}

#[tokio::test]
async fn delay_endpoint_failure_stays_failed_instead_of_becoming_a_timeout_claim() {
    let mut server = Server::new_async().await;
    let inventory = server
        .mock("GET", "/proxies")
        .expect(3)
        .with_status(200)
        .with_body(proxy_payload("proxy-a"))
        .create_async()
        .await;
    let delay = server
        .mock("GET", "/proxies/proxy-a/delay")
        .expect(1)
        .match_query("url=https%3A%2F%2Fhealth.example.test%2Fcheck&timeout=4000")
        .with_status(504)
        .with_body("controller delay endpoint failed")
        .create_async()
        .await;
    let active_scope = scope("mihomo", 7);
    let adapter = adapter(&server.url(), active_scope.clone());
    let observed = OutboundPort::inventory(&adapter, active_scope.clone())
        .await
        .expect("inventory");
    let result = OutboundPort::probe(
        &adapter,
        active_scope.clone(),
        observed.revision(),
        outbound_id("proxy-a"),
        probe_request(4_000),
    )
    .await
    .expect("probe failure is a typed observation");

    assert_eq!(result.scope(), &active_scope);
    assert_eq!(result.observed_inventory_revision(), observed.revision());
    assert!(result.detail_revision() > SnapshotRevision::new(1));
    assert!(matches!(
        result.state(),
        OutboundProbeState::Failed { failure } if failure.code == ErrorCode::Network
    ));
    inventory.assert_async().await;
    delay.assert_async().await;
}

#[tokio::test]
async fn mihomo_u32_timeout_boundary_is_checked_before_any_controller_io() {
    let mut server = Server::new_async().await;
    let unused = server
        .mock("GET", "/proxies")
        .expect(0)
        .with_status(200)
        .with_body(proxy_payload("proxy-a"))
        .create_async()
        .await;
    let adapter = adapter(&server.url(), scope("mihomo", 1));
    let error = OutboundPort::probe(
        &adapter,
        scope("mihomo", 1),
        SnapshotRevision::new(1),
        outbound_id("proxy-a"),
        probe_request(u64::from(u32::MAX) + 1),
    )
    .await
    .expect_err("Mihomo wire timeout is u32");
    assert!(
        matches!(error, PortError::Rejected(failure) if failure.code == ErrorCode::InvalidInput)
    );
    unused.assert_async().await;
}

#[tokio::test]
async fn stale_probe_revision_is_refused_before_probe_endpoint() {
    let mut server = Server::new_async().await;
    let inventory = server
        .mock("GET", "/proxies")
        .expect(1)
        .with_status(200)
        .with_body(proxy_payload("proxy-a"))
        .create_async()
        .await;
    let unused = server
        .mock("GET", "/proxies/proxy-a/delay")
        .expect(0)
        .with_status(200)
        .with_body(r#"{"delay":10}"#)
        .create_async()
        .await;
    let active_scope = scope("mihomo", 14);
    let adapter = adapter(&server.url(), active_scope.clone());
    let observed = OutboundPort::inventory(&adapter, active_scope.clone())
        .await
        .expect("inventory");
    assert_eq!(observed.revision(), SnapshotRevision::new(1));

    let error = OutboundPort::probe(
        &adapter,
        active_scope,
        SnapshotRevision::new(0),
        outbound_id("proxy-a"),
        probe_request(1_000),
    )
    .await
    .expect_err("stale revision is rejected");

    assert!(
        matches!(error, PortError::Rejected(failure) if failure.code == ErrorCode::InvalidState)
    );
    inventory.assert_async().await;
    unused.assert_async().await;
}

#[test]
fn unknown_wire_group_never_maps_as_a_manually_selectable_group() {
    let payload = r#"{"proxies":{"custom":{"type":"NovelSelector","name":"custom","now":"leaf","all":["leaf"]},"leaf":{"type":"Direct","name":"leaf"}}}"#;
    let proxies = proxies_from_payload(payload);
    let inventory = map_inventory(&proxies).expect("map unknown group");
    let OutboundInventoryState::Ready { collections, .. } = inventory else {
        panic!("inventory is ready");
    };
    assert_eq!(collections[0].availability, SelectionAvailability::Unknown);
    assert_eq!(collections[0].selection, OutboundSelectionState::Unknown);
}
