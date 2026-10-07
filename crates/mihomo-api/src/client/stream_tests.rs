//! test-intent: behavior
use crate::client::MihomoClient;
use futures_util::StreamExt;
use infiltrator_contract::error::ErrorCode;
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStreamEvent};
use mockito::{Matcher, Server};
use std::time::Duration;
use tokio::time::timeout;

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
        let mut stream = RuntimeGateway::stream_logs(&client, Some("debug".into()))
            .await
            .unwrap();
        assert!(matches!(
            timeout(Duration::from_secs(5), stream.next())
                .await
                .unwrap(),
            Some(RuntimeStreamEvent::Connecting)
        ));
        let Some(RuntimeStreamEvent::Failed(failure)) =
            timeout(Duration::from_secs(5), stream.next())
                .await
                .unwrap()
        else {
            panic!("typed terminal rejection");
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
    let mut stream = RuntimeGateway::stream_logs(&client, None).await.unwrap();
    assert!(matches!(
        stream.next().await,
        Some(RuntimeStreamEvent::Connecting)
    ));
    let Some(RuntimeStreamEvent::Failed(failure)) = stream.next().await else {
        panic!("header encoding must reject before network access");
    };
    assert_eq!(failure.code, ErrorCode::Configuration);
    assert!(!failure.retryable);
    assert!(!failure.message.contains(secret));
    assert!(stream.next().await.is_none());
}
