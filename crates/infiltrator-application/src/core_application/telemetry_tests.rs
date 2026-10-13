//! test-intent: behavior
//! Telemetry blocked at the real port boundary cannot cross a core-stop/session fence.
use super::*;
use crate::core_application::test_execute_result_at_current_scope;
use infiltrator_ports::overview::OverviewSample;
use tokio::sync::oneshot;

struct PausedOverview {
    entered: Mutex<Option<oneshot::Sender<()>>>,
    release: Mutex<Option<oneshot::Receiver<OverviewSample>>>,
}
#[async_trait::async_trait]
impl OverviewReader for PausedOverview {
    async fn sample(&self) -> Result<OverviewSample, PortError> {
        let release = self.release.lock().unwrap().take().unwrap();
        self.entered
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .send(())
            .unwrap();
        release
            .await
            .map_err(|_| PortError::Failed("fixture disconnected".into()))
    }
}

#[tokio::test]
async fn stopped_core_rejects_the_actual_inflight_old_telemetry_result_without_restoring_running_state()
 {
    let (entered, observed) = oneshot::channel();
    let (release, waiting) = oneshot::channel();
    let app = CoreApplication::new_with_overview(
        Arc::new(FakeProcess {
            running: AtomicBool::new(true),
            fail_start: false,
            fail_stop: false,
        }),
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".into()),
        }),
        Arc::new(PausedOverview {
            entered: Mutex::new(Some(entered)),
            release: Mutex::new(Some(waiting)),
        }),
        runtime(),
    );
    super::install_lifecycle_capabilities(&app);
    assert!(app.adopt_if_running().await.unwrap());
    let reader = app.clone();
    let pending = tokio::spawn(async move { reader.read_telemetry().await });
    observed.await.unwrap();
    assert!(matches!(
        test_execute_result_at_current_scope(&app, CommandIntent::StopCore).await,
        CommandResult::Completed { .. }
    ));
    release
        .send(OverviewSample {
            lifecycle: CoreLifecycle::Running,
            upload_total: 100,
            download_total: 200,
            active_connections: 10,
            core_version: None,
            sampled_at_epoch_ms: Some(1),
        })
        .unwrap();
    assert_eq!(
        pending.await.unwrap().unwrap_err().code,
        ErrorCode::NotReady
    );
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Stopped);
    assert!(app.snapshot().sampled_at_epoch_ms.is_none());
}
