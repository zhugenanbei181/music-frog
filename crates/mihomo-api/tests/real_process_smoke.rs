use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::outbound::inventory::{
    OutboundInventoryState, OutboundProbeState, OutboundSelectionState,
};
use infiltrator_contract::routing_intent::RoutingIntent;
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use infiltrator_ports::outbound::{
    OutboundPort, OutboundProbeRequest, OutboundScopeSource, ProbeTarget,
};
use infiltrator_ports::routing_intent::RoutingIntentPort;
use mihomo_api::client::{MihomoClient, StreamEvent};
use mihomo_api::outbound::MihomoOutboundAdapter;
use mihomo_api::routing_intent::MihomoRoutingIntentAdapter;
use std::env;
use std::sync::Arc;
use std::time::Duration;

struct FixedScope(CoreScope);

impl OutboundScopeSource for FixedScope {
    fn current_scope(&self) -> Option<CoreScope> {
        Some(self.0.clone())
    }
}

fn required_environment(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set by the explicit Mihomo smoke"))
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked Mihomo real-process smoke"]
async fn typed_core_log_level_outbound_and_routing_ports_drive_the_live_mihomo_controller() {
    let controller_url = required_environment("MIHOMO_REAL_CONTROLLER_URL");
    let controller_secret = required_environment("MIHOMO_REAL_CONTROLLER_SECRET");
    let expected_version = required_environment("MIHOMO_REAL_VERSION");
    let client = MihomoClient::new(&controller_url, Some(controller_secret))
        .expect("construct production Mihomo controller client");
    let observed_version = client
        .get_version()
        .await
        .expect("read live controller version");
    assert_eq!(observed_version.version, expected_version);

    let mut log_events = client
        .stream_logs_events(None)
        .await
        .expect("open typed Mihomo log stream");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match log_events.recv().await.expect("log stream remains open") {
                StreamEvent::Connected => break,
                StreamEvent::Failed(failure) => panic!("log stream failed: {failure}"),
                StreamEvent::Connecting | StreamEvent::Item(_) | StreamEvent::Reconnecting(_) => {}
            }
        }
    })
    .await
    .expect("live Mihomo accepts the documented /logs WebSocket operation");
    drop(log_events);

    let prior_log_level = client
        .get_config()
        .await
        .expect("read current Mihomo log level");
    let prior_log_level = CoreLogLevel::from_key(&prior_log_level.log_level)
        .or_else(|| {
            prior_log_level
                .log_level
                .eq_ignore_ascii_case("warning")
                .then_some(CoreLogLevel::Warn)
        })
        .expect("live Mihomo log level is in the typed shared catalog");
    let requested_log_level = if prior_log_level == CoreLogLevel::Debug {
        CoreLogLevel::Error
    } else {
        CoreLogLevel::Debug
    };
    let changed_log_level =
        CoreLogLevelPort::set_core_log_level(&client, requested_log_level).await;
    let restored_log_level = CoreLogLevelPort::set_core_log_level(&client, prior_log_level).await;
    assert_eq!(
        changed_log_level.expect("typed core log-level write and readback"),
        requested_log_level
    );
    assert_eq!(
        restored_log_level.expect("restore core log level through the typed port"),
        prior_log_level
    );

    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("Mihomo engine ID"),
            CoreInstanceId::new("core-008-real-process").expect("core instance ID"),
            Some(CoreVersion::new(expected_version).expect("live core version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let outbound = MihomoOutboundAdapter::new(
        Arc::new(client.clone()),
        Arc::new(FixedScope(scope.clone())),
    );

    let inventory = OutboundPort::inventory(&outbound, scope.clone())
        .await
        .expect("typed outbound inventory read");
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        panic!("live Mihomo inventory must be ready");
    };
    let proxy_id = entries
        .iter()
        .find(|entry| entry.id.value() == "CORE-SMOKE-HTTP")
        .expect("profile HTTP outbound is present")
        .id
        .clone();
    let collection = collections
        .iter()
        .find(|collection| collection.id.value() == "CORE-SMOKE-SELECTOR")
        .expect("profile selector collection is present");
    let collection_id = collection.id.clone();
    let reject_id = collection
        .member_ids
        .iter()
        .find(|member| member.value() == "REJECT")
        .expect("selector contains REJECT")
        .clone();
    let direct_id = collection
        .member_ids
        .iter()
        .find(|member| member.value() == "DIRECT")
        .expect("selector contains DIRECT")
        .clone();
    assert!(matches!(
        &collection.selection,
        OutboundSelectionState::Ready { selected } if selected.value() == "DIRECT"
    ));

    let observation = OutboundPort::observation(&outbound, scope.clone())
        .await
        .expect("typed inventory and details observation");
    let OutboundInventoryState::Ready { entries, .. } = observation.inventory().inventory() else {
        panic!("typed observation inventory must be ready");
    };
    assert!(entries.iter().any(|entry| entry.id == proxy_id));
    assert!(observation.details().detail(&proxy_id).is_some());

    let selected = OutboundPort::select(
        &outbound,
        scope.clone(),
        inventory.revision(),
        collection_id.clone(),
        reject_id.clone(),
    )
    .await
    .expect("typed selection write and controller readback");
    let OutboundInventoryState::Ready { collections, .. } = selected.inventory() else {
        panic!("post-selection inventory must be ready");
    };
    let readback = collections
        .iter()
        .find(|collection| collection.id.value() == "CORE-SMOKE-SELECTOR")
        .expect("selector readback is present");
    assert!(matches!(
        &readback.selection,
        OutboundSelectionState::Ready { selected } if selected == &reject_id
    ));

    let probe_request = OutboundProbeRequest::new(
        ProbeTarget::http_url("http://probe.test/health").expect("valid local probe URL"),
        5_000,
    )
    .expect("valid typed probe request");
    let probe = OutboundPort::probe(
        &outbound,
        scope.clone(),
        selected.revision(),
        proxy_id,
        probe_request,
    )
    .await
    .expect("typed outbound probe");
    assert!(matches!(probe.state(), OutboundProbeState::Measured { .. }));

    let before_restore = OutboundPort::inventory(&outbound, scope.clone())
        .await
        .expect("read outbound inventory before restoring the fixture");
    let restored = OutboundPort::select(
        &outbound,
        scope.clone(),
        before_restore.revision(),
        collection_id,
        direct_id,
    )
    .await
    .expect("restore selector through the typed port");
    let OutboundInventoryState::Ready { collections, .. } = restored.inventory() else {
        panic!("restored inventory must be ready");
    };
    assert!(collections.iter().any(|collection| {
        collection.id.value() == "CORE-SMOKE-SELECTOR"
            && matches!(
                &collection.selection,
                OutboundSelectionState::Ready { selected } if selected.value() == "DIRECT"
            )
    }));

    let routing = MihomoRoutingIntentAdapter::new(client);
    assert_eq!(
        routing.observe(&scope).await.expect("typed routing GET"),
        RoutingIntent::FollowRules
    );
    assert_eq!(
        routing
            .apply(&scope, RoutingIntent::RouteAllThroughProxy)
            .await
            .expect("typed routing SET and readback"),
        RoutingIntent::RouteAllThroughProxy
    );
    assert_eq!(
        routing
            .observe(&scope)
            .await
            .expect("typed routing readback"),
        RoutingIntent::RouteAllThroughProxy
    );
    assert_eq!(
        routing
            .apply(&scope, RoutingIntent::FollowRules)
            .await
            .expect("restore fixture routing mode through typed port"),
        RoutingIntent::FollowRules
    );
}
