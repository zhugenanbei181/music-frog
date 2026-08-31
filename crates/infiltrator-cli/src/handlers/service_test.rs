use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::traits::CoreController;

use super::{Lifecycle, run_lifecycle, status_message};

/// Mock controller mirroring ProcessCoreController semantics: start fails
/// while running, stop fails while stopped.
struct MockController {
    running: AtomicBool,
    pid: Option<u32>,
}

impl MockController {
    fn new(pid: Option<u32>) -> Self {
        Self {
            running: AtomicBool::new(false),
            pid,
        }
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl CoreController for MockController {
    async fn start(&self) -> Result<()> {
        if self.is_running() {
            return Err(MihomoError::Service("Service is already running".to_string()));
        }
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        if !self.is_running() {
            return Err(MihomoError::Service("Service is not running".to_string()));
        }
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn controller_url(&self) -> Option<String> {
        None
    }

    async fn pid(&self) -> Option<u32> {
        self.pid
    }
}

#[tokio::test]
async fn start_succeeds_when_stopped_and_fails_when_running() {
    let controller = MockController::new(None);
    let message = run_lifecycle(Lifecycle::Start, &controller).await.unwrap();
    assert_eq!(message, "Service started");
    assert!(controller.is_running());

    let err = run_lifecycle(Lifecycle::Start, &controller)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("already running"), "{err}");
}

#[tokio::test]
async fn stop_fails_when_not_running_and_succeeds_after_start() {
    let controller = MockController::new(None);
    let err = run_lifecycle(Lifecycle::Stop, &controller).await.unwrap_err();
    assert!(err.to_string().contains("not running"), "{err}");

    run_lifecycle(Lifecycle::Start, &controller).await.unwrap();
    let message = run_lifecycle(Lifecycle::Stop, &controller).await.unwrap();
    assert_eq!(message, "Service stopped");
    assert!(!controller.is_running());
}

#[tokio::test]
async fn restart_starts_a_stopped_service_without_stopping_twice() {
    let controller = MockController::new(Some(4242));
    let message = run_lifecycle(Lifecycle::Restart, &controller).await.unwrap();
    assert_eq!(message, "Service restarted");
    assert!(controller.is_running());
}

#[tokio::test]
async fn status_reports_pid_of_running_service() {
    let controller = MockController::new(Some(4242));
    let message = run_lifecycle(Lifecycle::Status, &controller).await.unwrap();
    assert_eq!(message, "Service is stopped");

    run_lifecycle(Lifecycle::Start, &controller).await.unwrap();
    let message = run_lifecycle(Lifecycle::Status, &controller).await.unwrap();
    assert_eq!(message, "Service is running (pid 4242)");
}

#[test]
fn status_message_covers_running_without_pid() {
    assert_eq!(status_message(true, None), "Service is running");
    assert_eq!(status_message(false, Some(1)), "Service is stopped");
}
