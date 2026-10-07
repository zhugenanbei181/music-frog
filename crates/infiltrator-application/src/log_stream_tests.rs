//! test-intent: behavior
use crate::log_application::LogApplication;
use crate::log_stream::{LogDriver, LogPump};
use crate::log_stream_test_support::{LogGateway, TestRuntime};
use futures_util::stream::{iter, pending};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::{LogSession, LogStreamState};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot, CoreSnapshot};
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;
use tokio::task::{spawn_blocking, yield_now};
use tokio::time::{advance, timeout};

fn lifecycle() -> CoreLifecycleSnapshot {
    CoreLifecycleSnapshot {
        lifecycle: CoreLifecycle::Running,
        generation: 1,
        session_token: Some(SessionToken::new(42)),
        ..Default::default()
    }
}
fn snapshot(core: &CoreLifecycleSnapshot) -> CoreSnapshot {
    CoreSnapshot {
        lifecycle: core.lifecycle.clone(),
        generation: core.generation,
        session_token: core.session_token,
        revision: 0,
        proxy_mode: None,
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
    assert!(matches!(data.stream, LogStreamState::Failed(_)));
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
        LogSession {
            generation: 1,
            token: SessionToken::new(42)
        },
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
        RuntimeStreamEvent::Failed(failure.clone()),
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
