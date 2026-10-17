//! test-intent: behavior

use crate::api::http::{SingBoxClashMethod, SingBoxControllerAuthorization};
use crate::routing_intent::SingBoxRoutingIntentAdapter;
use crate::test_support::{FixedScope, ScriptedHttpPort, client, scope, version_response};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::routing_intent::{
    EngineDefinedRoutingIntent, RoutingIntent, RoutingIntentExtensionId,
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::routing_intent::RoutingIntentPort;
use std::sync::Arc;

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
