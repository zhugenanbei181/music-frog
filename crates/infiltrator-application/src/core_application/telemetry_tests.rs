//! test-intent: behavior
//! Telemetry blocked at the real port boundary cannot cross a core-stop/session fence.
use super::*;
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
    async fn set_mode(&self, mode: ProxyMode) -> Result<ProxyMode, PortError> {
        Ok(mode)
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
    assert!(app.adopt_if_running().await.unwrap());
    let reader = app.clone();
    let pending = tokio::spawn(async move { reader.read_telemetry().await });
    observed.await.unwrap();
    assert!(matches!(
        app.execute(CommandIntent::StopCore).await,
        CommandResult::Completed { .. }
    ));
    release
        .send(OverviewSample {
            lifecycle: CoreLifecycle::Running,
            mode: Some(ProxyMode::Global),
            upload_total: 100,
            download_total: 200,
            active_connections: 10,
            memory_bytes: Some(4000),
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
