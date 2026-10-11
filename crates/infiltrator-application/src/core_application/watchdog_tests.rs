use super::*;
use crate::core_application::test_execute_result_at_current_scope;
use infiltrator_contract::snapshot::CoreWatchdogState;
use infiltrator_domain::watchdog::WatchdogConfig;
use infiltrator_ports::core_watchdog::{CoreWatchdogPort, WatchdogTick};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::{sync::Arc, time};

#[tokio::test]
async fn watchdog_detects_exit_and_restarts_with_a_new_session() {
    let running = Arc::new(AtomicBool::new(false));
    let starts = Arc::new(AtomicU64::new(0));
    let process = Arc::new(WatchdogProcess {
        running: running.clone(),
        starts: starts.clone(),
        fail_starts: Arc::new(AtomicU64::new(0)),
    });
    let app = CoreApplication::new_with_policy(
        process,
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".to_string()),
        }),
        ReadinessPolicy {
            timeout: time::Duration::from_millis(100),
            poll_interval: time::Duration::from_millis(1),
        },
        runtime(),
    );
    install_lifecycle_capabilities(&app);
    app.configure_watchdog(WatchdogConfig {
        initial_retry_delay: time::Duration::ZERO,
        max_retry_delay: time::Duration::from_millis(1),
        max_restart_attempts: 2,
    });

    assert!(matches!(
        test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    let before = app.snapshot();
    running.store(false, Ordering::SeqCst);

    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app)
            .await
            .expect("detect exit"),
        WatchdogTick::Waiting {
            attempt: 1,
            retry_in_ms: 0,
            ..
        }
    ));
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Failed);

    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app)
            .await
            .expect("recover exit"),
        WatchdogTick::Recovered { attempts: 1, .. }
    ));
    let after = app.snapshot();
    assert_eq!(after.lifecycle, CoreLifecycle::Running);
    assert!(after.generation > before.generation);
    assert_ne!(after.session_token, before.session_token);
    assert!(matches!(
        after.watchdog.state,
        CoreWatchdogState::Recovered { attempts: 1 }
    ));
    assert_eq!(starts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn watchdog_restart_fails_closed_without_restart_capability() {
    let running = Arc::new(AtomicBool::new(false));
    let starts = Arc::new(AtomicU64::new(0));
    let app = CoreApplication::new_with_policy(
        Arc::new(WatchdogProcess {
            running: running.clone(),
            starts: starts.clone(),
            fail_starts: Arc::new(AtomicU64::new(0)),
        }),
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".to_string()),
        }),
        ReadinessPolicy {
            timeout: time::Duration::from_millis(100),
            poll_interval: time::Duration::from_millis(1),
        },
        runtime(),
    );
    install_lifecycle_capabilities(&app);
    app.configure_watchdog(WatchdogConfig {
        initial_retry_delay: time::Duration::ZERO,
        max_retry_delay: time::Duration::from_millis(1),
        max_restart_attempts: 1,
    });
    assert!(matches!(
        test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    running.store(false, Ordering::SeqCst);
    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app).await.unwrap(),
        WatchdogTick::Waiting { attempt: 1, .. }
    ));
    app.install_core_capability_application(CoreCapabilityApplication::without_provider())
        .expect("provider may be replaced while no command is active");

    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app).await.unwrap(),
        WatchdogTick::Tripped { attempts: 1, .. }
    ));
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert!(!running.load(Ordering::SeqCst));
}

#[tokio::test]
async fn watchdog_opens_the_circuit_after_repeated_restart_failures() {
    let running = Arc::new(AtomicBool::new(false));
    let starts = Arc::new(AtomicU64::new(0));
    let fail_starts = Arc::new(AtomicU64::new(0));
    let process = Arc::new(WatchdogProcess {
        running: running.clone(),
        starts: starts.clone(),
        fail_starts: fail_starts.clone(),
    });
    let app = CoreApplication::new_with_policy(
        process,
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".to_string()),
        }),
        ReadinessPolicy {
            timeout: time::Duration::from_millis(100),
            poll_interval: time::Duration::from_millis(1),
        },
        runtime(),
    );
    install_lifecycle_capabilities(&app);
    app.configure_watchdog(WatchdogConfig {
        initial_retry_delay: time::Duration::ZERO,
        max_retry_delay: time::Duration::from_millis(1),
        max_restart_attempts: 2,
    });

    assert!(matches!(
        test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    fail_starts.store(2, Ordering::SeqCst);
    let before = app.snapshot();
    running.store(false, Ordering::SeqCst);

    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app)
            .await
            .expect("detect exit"),
        WatchdogTick::Waiting { attempt: 1, .. }
    ));
    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app)
            .await
            .expect("first failed restart"),
        WatchdogTick::Waiting { attempt: 2, .. }
    ));
    assert!(matches!(
        CoreWatchdogPort::watchdog_tick(&app)
            .await
            .expect("second failed restart"),
        WatchdogTick::Tripped { attempts: 2, .. }
    ));
    assert!(matches!(
        app.snapshot().watchdog.state,
        CoreWatchdogState::Tripped { attempts: 2 }
    ));
    assert_eq!(app.snapshot().generation, before.generation + 2);
    assert_eq!(starts.load(Ordering::SeqCst), 3);
}
