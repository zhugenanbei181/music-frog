//! Owner-scoped resource-port behavior against the real controller client.

use crate::resource_port::MihomoRuntimeResourcePort;
use crate::runtime_configuration::RuntimeScopeResolver;
use infiltrator_contract::resources::ResourceAvailability;
use infiltrator_ports::resources::{CoreResourcePort, GarbageCollectionOutcome};
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use mihomo_driver::api::client::MihomoClient;
use mockito::Server;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn gc_endpoint_missing_without_startup_debug_is_typed_unsupported() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("PUT", "/debug/gc")
        .with_status(404)
        .create_async()
        .await;
    let scope = resource_scope(1);
    let resolver_scope = scope.clone();
    let resolver: RuntimeScopeResolver = Arc::new(move || Ok(resolver_scope.clone()));
    let client = MihomoClient::new(&server.url(), Some("generated-by-host".to_owned())).unwrap();
    let port = MihomoRuntimeResourcePort::new(client, Some(resolver));

    let receipt = port
        .collect_garbage(scope.clone(), 0)
        .await
        .expect("missing GC endpoint is a typed unsupported outcome");
    assert_eq!(receipt.scope, scope);
    assert_eq!(receipt.outcome, GarbageCollectionOutcome::Unsupported);
    mock.assert_async().await;
}

#[tokio::test]
async fn typed_resource_gc_requires_and_returns_same_scope_memory_readback() {
    let mut server = Server::new_async().await;
    let gc = server
        .mock("PUT", "/debug/gc")
        .match_header("authorization", "Bearer generated-by-host")
        .with_status(204)
        .create_async()
        .await;
    let readback = server
        .mock("GET", "/memory")
        .match_header("authorization", "Bearer generated-by-host")
        .with_status(200)
        .with_body(json!({ "inuse": 400, "oslimit": 1000 }).to_string())
        .create_async()
        .await;
    let scope = resource_scope(7);
    let resolver_scope = scope.clone();
    let resolver: RuntimeScopeResolver = Arc::new(move || Ok(resolver_scope.clone()));
    let client = MihomoClient::new(&server.url(), Some("generated-by-host".to_owned())).unwrap();
    let port = MihomoRuntimeResourcePort::new(client, Some(resolver));

    let receipt = port
        .collect_garbage(scope.clone(), 900)
        .await
        .expect("same-scope GC and memory readback");
    assert_eq!(receipt.scope, scope);
    assert_eq!(
        receipt.outcome,
        GarbageCollectionOutcome::Collected {
            before_bytes: 900,
            after_bytes: 400,
        }
    );
    gc.assert_async().await;
    readback.assert_async().await;
}

#[tokio::test]
async fn typed_resource_observation_distinguishes_memory_from_unsupported_cpu() {
    let mut server = Server::new_async().await;
    let memory = server
        .mock("GET", "/memory")
        .with_status(200)
        .with_body(format!(
            "{}\n{}\n",
            json!({ "inuse": 0, "oslimit": 1000 }),
            json!({ "inuse": 12, "oslimit": 1000 })
        ))
        .create_async()
        .await;
    let scope = resource_scope(9);
    let resolver_scope = scope.clone();
    let resolver: RuntimeScopeResolver = Arc::new(move || Ok(resolver_scope.clone()));
    let client = MihomoClient::new(&server.url(), None).unwrap();
    let port = MihomoRuntimeResourcePort::new(client, Some(resolver));

    let observation = port
        .observe(scope.clone())
        .await
        .expect("scoped memory observation");
    assert_eq!(observation.scope, scope);
    assert_eq!(
        observation.memory_bytes,
        ResourceAvailability::Available(12)
    );
    assert_eq!(observation.cpu_percent, ResourceAvailability::Unsupported);
    memory.assert_async().await;
}

#[tokio::test]
async fn typed_resource_observation_rejects_retired_scope_before_http_access() {
    let mut server = Server::new_async().await;
    let unexpected = server.mock("GET", "/memory").expect(0).create_async().await;
    let active_scope = resource_scope(8);
    let retired_scope = resource_scope(7);
    let resolver_scope = active_scope;
    let resolver: RuntimeScopeResolver = Arc::new(move || Ok(resolver_scope.clone()));
    let client = MihomoClient::new(&server.url(), None).unwrap();
    let port = MihomoRuntimeResourcePort::new(client, Some(resolver));

    let error = port
        .observe(retired_scope)
        .await
        .expect_err("retired scope must not be read");
    assert_eq!(error.error_code(), ErrorCode::InvalidState);
    unexpected.assert_async().await;
}

fn resource_scope(generation: u64) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("mihomo-resource-tests").expect("instance"),
            Some(CoreVersion::new("test").expect("version")),
        ),
        generation: CoreGeneration::new(generation),
        session_token: None,
    }
}
