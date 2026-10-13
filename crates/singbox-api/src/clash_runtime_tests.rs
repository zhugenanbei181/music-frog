//! test-intent: behavior

use crate::clash_api::SingBoxClashApiClient;
use crate::clash_runtime::{SingBoxOutboundAdapter, SingBoxRoutingIntentAdapter};
use crate::controller_configuration::SingBoxControllerConfiguration;
use crate::http::{
    SingBoxClashHttpPort, SingBoxClashMethod, SingBoxClashResponse, SingBoxControllerAuthorization,
};
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::details::OutboundDetailsState;
use infiltrator_contract::outbound::identity::{OutboundId, SnapshotRevision};
use infiltrator_contract::outbound::inventory::{
    OutboundInventoryState, OutboundProbeState, OutboundSelectionState, SelectionAvailability,
};
use infiltrator_contract::routing_intent::{
    EngineDefinedRoutingIntent, RoutingIntent, RoutingIntentExtensionId,
};
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::{OutboundPort, OutboundProbeRequest, ProbeTarget};
use infiltrator_ports::routing_intent::RoutingIntentPort;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

#[derive(Clone, Debug)]
struct ObservedRequest {
    url: String,
    method: SingBoxClashMethod,
    authorization: SingBoxControllerAuthorization,
    body: Option<Vec<u8>>,
}

#[derive(Default)]
struct ScriptedHttpPort {
    responses: Mutex<VecDeque<SingBoxClashResponse>>,
    requests: Mutex<Vec<ObservedRequest>>,
}

impl ScriptedHttpPort {
    fn push_json(&self, status: u16, body: serde_json::Value) {
        self.responses
            .lock()
            .unwrap()
            .push_back(SingBoxClashResponse {
                status,
                body: serde_json::to_vec(&body).unwrap(),
            });
    }

    fn push_empty(&self, status: u16) {
        self.responses
            .lock()
            .unwrap()
            .push_back(SingBoxClashResponse {
                status,
                body: Vec::new(),
            });
    }

    fn requests(&self) -> Vec<ObservedRequest> {
        self.requests.lock().unwrap().clone()
    }

    fn remaining_responses(&self) -> usize {
        self.responses.lock().unwrap().len()
    }
}

#[async_trait]
impl SingBoxClashHttpPort for ScriptedHttpPort {
    async fn request(
        &self,
        url: &Url,
        method: SingBoxClashMethod,
        authorization: &SingBoxControllerAuthorization,
        body: Option<Vec<u8>>,
        _timeout: Duration,
    ) -> Result<SingBoxClashResponse, PortError> {
        self.requests.lock().unwrap().push(ObservedRequest {
            url: url.to_string(),
            method,
            authorization: authorization.clone(),
            body,
        });
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| PortError::Failed("scripted HTTP response is missing".to_owned()))
    }
}

#[derive(Clone)]
struct FixedScope(Arc<Mutex<CoreScopeObservation>>);

impl FixedScope {
    fn new(scope: CoreScope) -> Self {
        Self(Arc::new(Mutex::new(CoreScopeObservation::Observed(scope))))
    }
}

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        self.0.lock().unwrap().clone()
    }
}

fn scope(generation: u64) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("sing-box").unwrap(),
            CoreInstanceId::new("primary").unwrap(),
            Some(CoreVersion::new("v1.14.3").unwrap()),
        ),
        generation: CoreGeneration::new(generation),
        session_token: None,
    }
}

fn client(http: Arc<ScriptedHttpPort>) -> SingBoxClashApiClient {
    SingBoxClashApiClient::new(
        SingBoxControllerConfiguration::from_profile_json(
            br#"{"experimental":{"clash_api":{"external_controller":"127.0.0.1:9090","secret":"fixture-secret"}}}"#,
        )
        .unwrap(),
        http,
    )
    .unwrap()
}

fn version_response() -> serde_json::Value {
    serde_json::json!({"version":"sing-box 1.14.3","meta":true})
}

fn proxy_response(selected: &str) -> serde_json::Value {
    serde_json::json!({
        "proxies": {
            "selector #1": {
                "type":"Selector", "name":"selector #1", "now":selected,
                "all":["proxy-a", "proxy-b"], "history":[], "udp":true
            },
            "proxy-a":{"type":"Direct", "name":"proxy-a", "udp":true, "history":[]},
            "proxy-b":{"type":"Direct", "name":"proxy-b", "udp":true, "history":[]},
            "GLOBAL":{"type":"Fallback", "name":"GLOBAL", "now":selected,
                "all":["selector #1", "proxy-a", "proxy-b"], "history":[]}
        }
    })
}

fn group_proxy_response() -> serde_json::Value {
    serde_json::json!({
        "proxies": {
            "automatic": {
                "type":"URLTest", "name":"automatic", "now":"proxy-a",
                "all":["proxy-a"], "history":[]
            },
            "future-group": {
                "type":"FutureGroup", "name":"future-group", "now":"proxy-a",
                "all":["proxy-a"], "history":[]
            },
            "proxy-a":{"type":"Direct", "name":"proxy-a", "history":[]}
        }
    })
}

fn enqueue_inventory_observation(http: &ScriptedHttpPort, selected: &str) {
    http.push_json(200, version_response());
    http.push_json(200, proxy_response(selected));
    http.push_json(200, version_response());
}

fn outbound_id(value: &str) -> OutboundId {
    OutboundId::new(EngineId::new("sing-box").unwrap(), value).unwrap()
}

#[tokio::test]
async fn live_version_and_engine_defined_mode_are_observed_without_neutral_semantic_aliases() {
    let http = Arc::new(ScriptedHttpPort::default());
    http.push_json(200, version_response());
    http.push_json(
        200,
        serde_json::json!({"mode":"Rule", "mode-list":["Rule"]}),
    );
    http.push_json(200, version_response());
    let scope = scope(4);
    let api = client(http.clone());
    let router = SingBoxRoutingIntentAdapter::new(api, Arc::new(FixedScope::new(scope.clone())));

    let observed = router.observe(&scope).await.unwrap();
    assert_eq!(
        observed,
        RoutingIntent::EngineDefined(EngineDefinedRoutingIntent::new(
            EngineId::new("sing-box").unwrap(),
            RoutingIntentExtensionId::new("mode.rule").unwrap(),
        ))
    );
    let requests = http.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].url, "http://127.0.0.1:9090/version");
    assert_eq!(requests[1].url, "http://127.0.0.1:9090/configs");
    assert!(requests.iter().all(|request| {
        request.authorization
            == SingBoxControllerAuthorization::BearerToken("fixture-secret".to_owned())
    }));
    assert!(
        requests
            .iter()
            .all(|request| request.method == SingBoxClashMethod::Get)
    );

    let unsupported = router
        .apply(&scope, RoutingIntent::FollowRules)
        .await
        .unwrap_err();
    assert!(matches!(
        unsupported,
        PortError::Unsupported {
            capability: Capability::RoutingIntent,
            ..
        }
    ));
    assert_eq!(
        http.requests().len(),
        3,
        "unsupported writes have no HTTP side effect"
    );
}

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
