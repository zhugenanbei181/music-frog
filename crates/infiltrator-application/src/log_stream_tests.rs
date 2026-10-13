//! test-intent: behavior
use crate::log_application::LogApplication;
use crate::log_stream::{LogDriver, LogPump};
use crate::log_stream_test_support::{LogGateway, TestRuntime};
use futures_util::stream::{BoxStream, iter, pending};
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, CoreScope, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::{LogSession, LogStreamState};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot, CoreSnapshot};
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_stream::{RuntimeStreamEvent, RuntimeStreamTerminal};
use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;
use tokio::task::{spawn_blocking, yield_now};
use tokio::time::{advance, timeout};

fn lifecycle() -> CoreLifecycleSnapshot {
    CoreLifecycleSnapshot {
        identity: Some(CoreIdentity::new(
            EngineId::new("log-test").expect("valid test engine"),
            CoreInstanceId::new("session-test").expect("valid test instance"),
            None,
        )),
        lifecycle: CoreLifecycle::Running,
        generation: 1,
        session_token: Some(SessionToken::new(42)),
        ..Default::default()
    }
}
fn older_scope() -> CoreScope {
    CoreLifecycleSnapshot {
        generation: 0,
        session_token: Some(SessionToken::new(41)),
        ..lifecycle()
    }
    .scope()
    .expect("old core scope")
}
fn snapshot(core: &CoreLifecycleSnapshot) -> CoreSnapshot {
    CoreSnapshot {
        identity: core.identity.clone(),
        lifecycle: core.lifecycle.clone(),
        generation: core.generation,
        session_token: core.session_token,
        revision: 0,
        routing_intent: Default::default(),
        core_version: None,
        sampled_at_epoch_ms: None,
        failure: None,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
        watchdog: Default::default(),
    }
}
fn driver(
    logs: LogApplication,
    gateway: Arc<LogGateway>,
    state: Arc<Mutex<Option<CoreLifecycleSnapshot>>>,
) -> LogDriver {
    LogDriver {
        logs,
        gateway,
        runtime: Arc::new(TestRuntime),
        observe: Box::new(move || state.lock().unwrap().clone()),
    }
}
#[tokio::test(start_paused = true)]
async fn pending_open_is_cancelled_on_lifecycle_change_and_old_stream_is_never_consumed() {
    let logs = LogApplication::default();
    let state = Arc::new(Mutex::new(Some(lifecycle())));
    let gate = Arc::new(Notify::new());
    let gateway = Arc::new(LogGateway {
        opening_gate: Some(gate),
        ..Default::default()
    });
    let task = tokio::spawn(driver(logs.clone(), gateway.clone(), state.clone()).run());
    gateway.opened.notified().await;
    *state.lock().unwrap() = None;
    advance(Duration::from_millis(100)).await;
    task.await.unwrap();
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 1);
    assert!(logs.page(&snapshot(&lifecycle())).data.is_none());
}
#[tokio::test(start_paused = true)]
async fn permanent_open_failure_retains_taxonomy_and_does_not_retry_until_new_session() {
    let logs = LogApplication::default();
    let state = Arc::new(Mutex::new(Some(lifecycle())));
    let gateway = Arc::new(LogGateway {
        failure: Some(PortError::PermissionDenied("access denied".into())),
        ..Default::default()
    });
    let task = tokio::spawn(driver(logs.clone(), gateway.clone(), state.clone()).run());
    gateway.opened.notified().await;
    yield_now().await;
    let page = logs.page(&snapshot(&lifecycle()));
    assert!(
        matches!(page.status, PageStatus::Failed { failure } if failure.code == ErrorCode::Permission && !failure.retryable)
    );
    advance(Duration::from_secs(5)).await;
    yield_now().await;
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 1);
    state.lock().unwrap().as_mut().unwrap().generation = 2;
    advance(Duration::from_millis(100)).await;
    gateway.opened.notified().await;
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 2);
    *state.lock().unwrap() = None;
    advance(Duration::from_millis(100)).await;
    task.await.unwrap();
}
#[tokio::test(start_paused = true)]
async fn stream_end_preserves_records_and_new_session_retires_them_before_reconnecting() {
    let logs = LogApplication::default();
    let state = Arc::new(Mutex::new(Some(lifecycle())));
    let gateway = Arc::new(LogGateway::default());
    gateway.streams.lock().unwrap().push(Box::pin(iter([
        RuntimeStreamEvent::Connected,
        RuntimeStreamEvent::Item("WARN[1] retained".into()),
    ])));
    let task = tokio::spawn(driver(logs.clone(), gateway.clone(), state.clone()).run());
    gateway.opened.notified().await;
    yield_now().await;
    let data = logs.page(&snapshot(&lifecycle())).data.unwrap();
    assert_eq!(data.entries[0].message, "retained");
    assert_eq!(data.stream, LogStreamState::Cancelled);
    let next_core = CoreLifecycleSnapshot {
        generation: 2,
        session_token: Some(SessionToken::new(43)),
        ..lifecycle()
    };
    *state.lock().unwrap() = Some(next_core.clone());
    advance(Duration::from_millis(100)).await;
    gateway.opened.notified().await;
    assert!(logs.page(&snapshot(&next_core)).data.is_none());
    assert!(!logs.ingest(
        LogSession::new(1, SessionToken::new(42)),
        RuntimeStreamEvent::Item("late".into())
    ));
    *state.lock().unwrap() = None;
    advance(Duration::from_millis(100)).await;
    task.await.unwrap();
}
#[tokio::test]
async fn pump_drop_cancels_pending_io_and_releases_gateway_ownership() {
    let gateway = Arc::new(LogGateway::default());
    gateway.streams.lock().unwrap().push(Box::pin(pending()));
    let weak = Arc::downgrade(&gateway);
    let opened = gateway.opened.clone();
    let pump = LogPump::spawn(
        LogApplication::default(),
        gateway.clone(),
        Arc::new(TestRuntime),
        || Some(lifecycle()),
    )
    .unwrap();
    timeout(Duration::from_secs(5), opened.notified())
        .await
        .unwrap();
    drop(gateway);
    spawn_blocking(move || drop(pump)).await.unwrap();
    assert!(
        weak.upgrade().is_none(),
        "driver cannot retain the host after cancellation"
    );
}

#[tokio::test(start_paused = true)]
async fn permanent_event_failure_preserves_records_and_cannot_be_overwritten_by_stream_end() {
    let logs = LogApplication::default();
    let state = Arc::new(Mutex::new(Some(lifecycle())));
    let gateway = Arc::new(LogGateway::default());
    let failure = Failure::new(
        ErrorCode::Authentication,
        "controller credential rejected",
        false,
    );
    gateway.streams.lock().unwrap().push(Box::pin(iter([
        RuntimeStreamEvent::Connected,
        RuntimeStreamEvent::Item("INFO[1] retained".into()),
        RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Failed(failure.clone())),
    ])));
    let task = tokio::spawn(driver(logs.clone(), gateway.clone(), state.clone()).run());
    gateway.opened.notified().await;
    yield_now().await;
    let page = logs.page(&snapshot(&lifecycle()));
    assert_eq!(
        page.status,
        PageStatus::Failed {
            failure: failure.clone()
        }
    );
    let data = page.data.unwrap();
    assert_eq!(data.entries[0].message, "retained");
    assert_eq!(data.stream, LogStreamState::Failed(failure));
    advance(Duration::from_secs(5)).await;
    yield_now().await;
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 1);
    *state.lock().unwrap() = None;
    advance(Duration::from_millis(100)).await;
    task.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn stale_gateway_scope_is_rejected_before_its_stream_can_write_current_logs() {
    let logs = LogApplication::default();
    let current = lifecycle();
    let state = Arc::new(Mutex::new(Some(current.clone())));
    let stale_stream: BoxStream<'static, RuntimeStreamEvent<String>> =
        Box::pin(iter([RuntimeStreamEvent::Item(
            "stale gateway record".into(),
        )]));
    let gateway = Arc::new(LogGateway {
        current_scope: Some(older_scope()),
        streams: Arc::new(Mutex::new(vec![stale_stream])),
        ..Default::default()
    });
    let task = tokio::spawn(driver(logs.clone(), gateway.clone(), state.clone()).run());

    gateway.opened.notified().await;
    yield_now().await;

    let page = logs.page(&snapshot(&current));
    assert!(page.data.is_none());
    assert!(matches!(
        page.status,
        PageStatus::Failed { failure } if failure.code == ErrorCode::Internal
    ));
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 1);

    *state.lock().unwrap() = None;
    advance(Duration::from_millis(100)).await;
    task.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn old_stream_event_scope_cannot_append_to_current_log_buffer() {
    let logs = LogApplication::default();
    let current = lifecycle();
    let current_scope = current.scope().expect("current core scope");
    let state = Arc::new(Mutex::new(Some(current.clone())));
    let old_stream: BoxStream<'static, RuntimeStreamEvent<String>> = Box::pin(iter([
        RuntimeStreamEvent::Connected,
        RuntimeStreamEvent::Item("old stream record".into()),
    ]));
    let gateway = Arc::new(LogGateway {
        current_scope: Some(current_scope.clone()),
        event_scopes: Arc::new(Mutex::new(VecDeque::from([current_scope, older_scope()]))),
        streams: Arc::new(Mutex::new(vec![old_stream])),
        ..Default::default()
    });
    let task = tokio::spawn(driver(logs.clone(), gateway.clone(), state.clone()).run());

    gateway.opened.notified().await;
    yield_now().await;

    let page = logs.page(&snapshot(&current));
    let data = page.data.expect("current connected event is scoped");
    assert!(matches!(
        page.status,
        PageStatus::Failed { failure } if failure.code == ErrorCode::InvalidState
    ));
    assert!(data.entries.is_empty());
    assert_eq!(data.total_entries, 0);

    *state.lock().unwrap() = None;
    advance(Duration::from_millis(100)).await;
    task.await.unwrap();
}
