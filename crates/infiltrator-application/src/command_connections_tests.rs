//! test-intent: behavior
use super::*;
use crate::command_application::handler::ValidatedCoreCommandScope;
use crate::connection_application::ConnectionApplication;
use crate::core_capability_test_support::{core_scope, with_test_capabilities};
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};

struct UnavailableCoreScope;

impl CoreScopeSource for UnavailableCoreScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Unavailable
    }
}

#[tokio::test]
async fn close_all_uses_the_actual_runtime_port_and_preserves_typed_failure_for_retry() {
    let gateway = Arc::new(RecordingConnectionGateway::default());
    let connections =
        ConnectionApplication::new(Arc::new(UnavailableCoreScope), gateway.connection_port());
    let app = with_test_capabilities(CommandApplication::new().with_connections(connections));
    app.handle_scoped(
        ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
        CommandIntent::CloseAllConnections,
    )
    .await
    .unwrap();
    assert_eq!(gateway.close_all_calls.load(Ordering::SeqCst), 1);
    gateway.deny_close_all.store(true, Ordering::SeqCst);
    let failure = app
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::CloseAllConnections,
        )
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
    app.handle_scoped(
        ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
        CommandIntent::CloseAllConnections,
    )
    .await
    .unwrap();
    assert_eq!(gateway.close_all_calls.load(Ordering::SeqCst), 3);
    let failure = with_test_capabilities(CommandApplication::new())
        .handle_scoped(
            ValidatedCoreCommandScope::from_core_application(Some(core_scope())),
            CommandIntent::CloseAllConnections,
        )
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(gateway.close_all_calls.load(Ordering::SeqCst), 3);
}
