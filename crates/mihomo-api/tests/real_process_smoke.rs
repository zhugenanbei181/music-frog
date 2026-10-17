use futures_util::StreamExt;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::connection::ConnectionId;
use infiltrator_contract::dns_cache::DnsCacheOperationId;
use infiltrator_contract::dns_query::{DnsQueryRequest, DnsRecordType};
use infiltrator_contract::resources::ResourceAvailability;
use infiltrator_contract::rule_provider_runtime::{
    RuleProviderId, RuleProviderPayload, RuleProviderPayloadSnapshot,
};
use infiltrator_ports::connection::{
    ConnectionControlRequest, ConnectionControlTarget, ConnectionPort,
};
use infiltrator_ports::controller_fake_ip_cache::{
    ControllerFakeIpCacheFlushRequest, ControllerFakeIpCacheFlushState, ControllerFakeIpCachePort,
};
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use infiltrator_ports::dns_query::DnsQueryPort;
use infiltrator_ports::resources::{CoreResourcePort, GarbageCollectionOutcome};
use infiltrator_ports::rule_provider_runtime::RuleProviderPort;
use infiltrator_ports::runtime_stream::{RuntimeStreamEvent, RuntimeStreamPort};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::outbound::inventory::{
    OutboundInventoryState, OutboundProbeState, OutboundSelectionState,
};
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_core_ports::outbound::{OutboundPort, OutboundProbeRequest, ProbeTarget};
use lurker_core_ports::routing_intent::RoutingIntentPort;
use mihomo_api::client::MihomoClient;
use mihomo_api::connection_port::MihomoConnectionPort;
use mihomo_api::controller_fake_ip_cache::MihomoControllerFakeIpCacheAdapter;
use mihomo_api::outbound::MihomoOutboundAdapter;
use mihomo_api::resource_port::MihomoRuntimeResourcePort;
use mihomo_api::routing_intent::MihomoRoutingIntentAdapter;
use mihomo_api::rule_provider::MihomoRuleProviderAdapter;
use mihomo_api::runtime_configuration::RuntimeScopeResolver;
use mihomo_api::runtime_stream::MihomoRuntimeStreamPort;
use std::env;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};

struct FixedScope(CoreScope);

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.0.clone())
    }
}

fn required_environment(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set by the explicit Mihomo smoke"))
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked Mihomo real-process smoke"]
async fn typed_core_dns_fakeip_rule_provider_connection_close_and_runtime_ports_drive_live_mihomo()
{
    let controller_url = required_environment("MIHOMO_REAL_CONTROLLER_URL");
    let controller_secret = required_environment("MIHOMO_REAL_CONTROLLER_SECRET");
    let expected_version = required_environment("MIHOMO_REAL_VERSION");
    let client = MihomoClient::new(&controller_url, Some(controller_secret))
        .expect("construct production Mihomo controller client");
    let connection_scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("Mihomo engine ID"),
            CoreInstanceId::new("real-process-smoke").expect("smoke Core instance ID"),
            None,
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let resolver_scope = connection_scope.clone();
    let scope_resolver: RuntimeScopeResolver = Arc::new(move || Ok(resolver_scope.clone()));
    let connection_port = MihomoConnectionPort::new(client.clone(), Some(scope_resolver.clone()));
    let observed_version = client
        .get_version()
        .await
        .expect("read live controller version");
    assert_eq!(observed_version.version, expected_version);

    let dns_request = DnsQueryRequest {
        name: "core-smoke.test".to_owned(),
        record_type: DnsRecordType::A,
    };
    let dns_response = DnsQueryPort::query(&client, &dns_request)
        .await
        .expect("typed DNS query against the running Mihomo controller");
    assert_eq!(dns_response.status, 0, "fixture DNS query is successful");
    assert!(dns_response.answers.iter().any(|answer| {
        answer.record_type == DnsRecordType::A.number() && answer.data == "192.0.2.42"
    }));

    let active_connections = connection_port
        .observe(connection_scope.clone())
        .await
        .expect("typed Mihomo connection snapshot");
    let active_connection = active_connections
        .snapshot()
        .connections
        .iter()
        .find(|connection| connection.metadata.host == "core-smoke-active.test")
        .expect("the smoke fixture holds one live connection through Mihomo");
    let active_connection_id = ConnectionId::new(
        connection_scope.identity.engine.clone(),
        active_connection.id.clone(),
    )
    .expect("engine-scoped Mihomo connection ID");
    let close_request = ConnectionControlRequest::new(
        connection_scope.clone(),
        active_connections.revision(),
        ConnectionControlTarget::One(active_connection_id.clone()),
    )
    .expect("close request uses the observed connection revision");
    connection_port
        .close(close_request)
        .await
        .expect("typed Connection.Close reaches the live Mihomo controller");
    timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = connection_port
                .observe(connection_scope.clone())
                .await
                .expect("typed connection readback remains available");
            if !snapshot.connection_ids().contains(&active_connection_id) {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("Mihomo removes the exact connection after Connection.Close");

    let stream_port = MihomoRuntimeStreamPort::new(client.clone(), Some(scope_resolver.clone())); // runtime_stream_port()
    let mut log_events = stream_port
        .logs(connection_scope.clone(), None)
        .await
        .expect("open Core-scoped Mihomo log stream");
    timeout(Duration::from_secs(5), async {
        loop {
            let event = log_events.next().await.expect("log stream remains open");
            assert_eq!(event.scope, connection_scope);
            match event.event {
                RuntimeStreamEvent::Connected => break,
                RuntimeStreamEvent::Terminal(terminal) => {
                    panic!("log stream terminated: {terminal:?}")
                }
                RuntimeStreamEvent::Connecting
                | RuntimeStreamEvent::Item(_)
                | RuntimeStreamEvent::Reconnecting(_) => {}
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

    assert_eq!(
        client
            .get_config()
            .await
            .expect("read startup-enabled Mihomo debug endpoint state")
            .log_level,
        "debug",
        "the smoke process starts in debug mode because Mihomo mounts /debug only when log-level is debug at process startup"
    );
    let resource_port =
        MihomoRuntimeResourcePort::new(client.clone(), Some(scope_resolver.clone()));
    let resource_before = resource_port
        .observe(connection_scope.clone())
        .await
        .expect("typed scoped resource observation");
    let before_bytes = match resource_before.memory_bytes {
        ResourceAvailability::Available(value) => value,
        outcome => panic!("live Mihomo memory must be available: {outcome:?}"),
    };
    let gc_receipt = resource_port
        .collect_garbage(connection_scope.clone(), before_bytes)
        .await
        .expect("typed scoped Mihomo GC and memory readback");
    assert_eq!(gc_receipt.scope, connection_scope);
    let after_bytes = match gc_receipt.outcome {
        GarbageCollectionOutcome::Collected {
            before_bytes: observed,
            after_bytes,
        } if observed == before_bytes => after_bytes,
        outcome => panic!("GC requires same-scope memory readback: {outcome:?}"),
    };
    assert!(
        after_bytes > 0,
        "live Mihomo readback must contain memory use"
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
    let fake_ip_cache = MihomoControllerFakeIpCacheAdapter::new(client.clone());
    let fake_ip_request = ControllerFakeIpCacheFlushRequest {
        scope: scope.clone(),
        operation: DnsCacheOperationId(43),
    };
    let fake_ip_receipt = ControllerFakeIpCachePort::flush(&fake_ip_cache, fake_ip_request.clone())
        .await
        .expect("typed scoped Fake-IP flush reaches the live Mihomo controller");
    assert_eq!(fake_ip_receipt.scope, fake_ip_request.scope);
    assert_eq!(fake_ip_receipt.operation, fake_ip_request.operation);
    assert_eq!(
        fake_ip_receipt.state,
        ControllerFakeIpCacheFlushState::Flushed
    );

    let rule_provider_name = "CORE-SMOKE-RULE";
    let rule_provider_id = RuleProviderId::new(EngineId::mihomo(), rule_provider_name)
        .expect("valid controller rule-provider ID");
    let rule_providers = MihomoRuleProviderAdapter::new(
        Arc::new(client.clone()),
        Arc::new(FixedScope(scope.clone())),
    );
    let update_signal_path = required_environment("MIHOMO_REAL_RULE_PROVIDER_UPDATE_SIGNAL");
    let initial_inventory = timeout(Duration::from_secs(5), async {
        loop {
            let inventory = RuleProviderPort::inventory(&rule_providers, scope.clone())
                .await
                .expect("typed rule-provider list remains available");
            if let Some(provider) = inventory.provider(&rule_provider_id)
                && provider.rule_count == 1
            {
                break inventory;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("live rule provider starts with one fixture rule");
    assert_eq!(
        initial_inventory
            .provider(&rule_provider_id)
            .expect("typed provider fact")
            .rule_count,
        1
    );
    assert_eq!(
        RuleProviderPort::payload(
            &rule_providers,
            scope.clone(),
            initial_inventory.revision(),
            rule_provider_id.clone(),
        )
        .await
        .expect("HTTP providers do not publish inline payloads"),
        RuleProviderPayloadSnapshot::new(
            scope.clone(),
            initial_inventory.revision(),
            rule_provider_id.clone(),
            RuleProviderPayload::NotPublished,
        )
        .expect("valid not-published payload snapshot")
    );
    fs::write(&update_signal_path, b"update").expect("enable the second local fixture payload");
    let receipt = RuleProviderPort::refresh(
        &rule_providers,
        scope.clone(),
        initial_inventory.revision(),
        rule_provider_id.clone(),
    )
    .await
    .expect("typed rule-provider update reaches the live Mihomo controller");
    assert_eq!(receipt.scope(), &scope);
    assert_eq!(receipt.provider_id(), &rule_provider_id);
    let updated_rule_provider = timeout(Duration::from_secs(5), async {
        loop {
            let inventory = RuleProviderPort::inventory(&rule_providers, scope.clone())
                .await
                .expect("typed rule-provider readback remains available");
            if let Some(provider) = inventory.provider(&rule_provider_id)
                && provider.rule_count == 2
            {
                break provider.rule_count;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("rule-provider update readback reports both fixture rules");
    assert_eq!(updated_rule_provider, 2);

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
