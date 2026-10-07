//! test-intent: behavior
use super::*;

#[tokio::test]
async fn close_all_uses_the_actual_runtime_port_and_preserves_typed_failure_for_retry() {
    let gateway = Arc::new(RecordingGeoGateway::default());
    let app = CommandApplication::new().with_runtime(gateway.clone());
    app.execute(CommandIntent::CloseAllConnections)
        .await
        .unwrap();
    assert_eq!(gateway.close_all_calls.load(Ordering::SeqCst), 1);
    gateway.deny_close_all.store(true, Ordering::SeqCst);
    let failure = app
        .execute(CommandIntent::CloseAllConnections)
        .await
        .unwrap_err();
    assert_eq!(
        failure,
        Failure::from(PortError::PermissionDenied(
            "grant connection control permission".into()
        ))
    );
    assert_eq!(gateway.close_all_calls.load(Ordering::SeqCst), 2);
    gateway.deny_close_all.store(false, Ordering::SeqCst);
    app.execute(CommandIntent::CloseAllConnections)
        .await
        .unwrap();
    assert_eq!(gateway.close_all_calls.load(Ordering::SeqCst), 3);
    assert!(gateway.probe_calls.lock().unwrap().is_empty());
    assert_eq!(gateway.switches.load(Ordering::SeqCst), 0);
    let failure = CommandApplication::new()
        .execute(CommandIntent::CloseAllConnections)
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(gateway.close_all_calls.load(Ordering::SeqCst), 3);
}
