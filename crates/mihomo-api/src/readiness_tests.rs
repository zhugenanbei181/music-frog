use super::readiness::ControllerReadiness;
use infiltrator_ports::core_process::CoreReadiness;
use mockito::Server;

#[tokio::test]
async fn active_core_version_comes_from_the_live_controller_response() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/version")
        .with_status(200)
        .with_body(r#"{"version":"v1.20.3","premium":false}"#)
        .create_async()
        .await;
    let readiness = ControllerReadiness::new(server.url(), None);

    let version = readiness
        .active_core_version()
        .await
        .expect("controller version read succeeds")
        .expect("controller reports an active version");

    mock.assert_async().await;
    assert_eq!(version.raw(), "v1.20.3");
}
