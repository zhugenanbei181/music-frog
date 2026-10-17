//! test-intent: behavior
use crate::runtime_stream::MihomoRuntimeStreamPort;
use futures_util::StreamExt;
use infiltrator_ports::runtime_stream::{
    RuntimeStreamEvent, RuntimeStreamPort, RuntimeStreamTerminal,
};
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use mihomo_driver::api::client::MihomoClient;
use mockito::{Matcher, Server};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

fn scope() -> CoreScope {
    scope_for("mihomo")
}

fn scope_for(engine: &str) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new(engine).unwrap(),
            CoreInstanceId::new("stream-test").unwrap(),
            None,
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

#[tokio::test]
async fn mihomo_stream_adapter_rejects_a_foreign_engine_before_opening_io() {
    let active_scope = scope();
    let client = MihomoClient::new("http://127.0.0.1:1", None).unwrap();
    let stream_port = port(client, active_scope);

    let error = match stream_port.logs(scope_for("sing-box"), None).await {
        Err(error) => error,
        Ok(_) => panic!("Mihomo must reject a foreign engine before opening a stream"),
    };

    assert_eq!(error.error_code(), ErrorCode::Unsupported);
}

fn port(client: MihomoClient, active: CoreScope) -> MihomoRuntimeStreamPort {
    let resolver_scope = active.clone();
    MihomoRuntimeStreamPort::new(client, Some(Arc::new(move || Ok(resolver_scope.clone()))))
}

#[tokio::test]
async fn controller_stream_auth_permission_and_missing_endpoint_are_typed_terminal_failures() {
    for (status, expected) in [
        (401, ErrorCode::Authentication),
        (403, ErrorCode::Permission),
        (404, ErrorCode::Unsupported),
    ] {
        let mut server = Server::new_async().await;
        let request = server
            .mock("GET", "/logs")
            .match_query(Matcher::UrlEncoded("level".into(), "debug".into()))
            .with_status(status)
            .expect(1)
            .create_async()
            .await;
        let client = MihomoClient::new(&server.url(), None).unwrap();
        let active_scope = scope();
        let stream_port = port(client, active_scope.clone());
        let mut stream = stream_port
            .logs(active_scope.clone(), Some("debug".into()))
            .await
            .unwrap();
        assert!(matches!(
            timeout(Duration::from_secs(5), stream.next())
                .await
                .unwrap(),
            Some(event) if event.scope == active_scope && event.event == RuntimeStreamEvent::Connecting
        ));
        let Some(event) = timeout(Duration::from_secs(5), stream.next())
            .await
            .unwrap()
        else {
            panic!("typed terminal rejection");
        };
        let failure = match event.event {
            RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Unsupported(failure))
                if expected == ErrorCode::Unsupported =>
            {
                failure
            }
            RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Failed(failure))
                if expected != ErrorCode::Unsupported =>
            {
                failure
            }
            other => panic!("expected typed terminal rejection, received {other:?}"),
        };
        assert_eq!(failure.code, expected);
        assert!(!failure.retryable);
        assert!(
            timeout(Duration::from_secs(5), stream.next())
                .await
                .unwrap()
                .is_none(),
            "permanent HTTP failures must close rather than schedule a reconnect"
        );
        request.assert_async().await;
    }
}
#[tokio::test]
async fn invalid_secret_header_is_a_configuration_failure_without_exposing_the_secret() {
    let secret = "private\nsecret";
    let client = MihomoClient::new("http://127.0.0.1:9", Some(secret.into())).unwrap();
    let active_scope = scope();
    let stream_port = port(client, active_scope.clone());
    let mut stream = stream_port.logs(active_scope, None).await.unwrap();
    assert!(matches!(
        stream.next().await,
        Some(event) if event.event == RuntimeStreamEvent::Connecting
    ));
    let Some(event) = stream.next().await else {
        panic!("header encoding must reject before network access");
    };
    let failure = match event.event {
        RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Failed(failure)) => failure,
        other => panic!("expected typed failure, received {other:?}"),
    };
    assert_eq!(failure.code, ErrorCode::Configuration);
    assert!(!failure.retryable);
    assert!(!failure.message.contains(secret));
    assert!(stream.next().await.is_none());
}
