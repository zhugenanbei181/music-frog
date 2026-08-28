use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use infiltrator_http::{HttpClient, build_http_client, build_raw_http_client};
use mihomo_api::MihomoClient;

use super::events::AdminEventBus;
use super::models::{RebuildStatusResponse, RuntimeTrafficSnapshotResponse};

use infiltrator_core::AppSettings;

#[async_trait::async_trait]
pub trait AdminApiContext: Clone + Send + Sync + 'static {
    async fn rebuild_runtime(&self) -> anyhow::Result<()>;
    async fn set_use_bundled_core(&self, enabled: bool);
    async fn refresh_core_version_info(&self);
    async fn latest_stable_core(&self) -> anyhow::Result<(String, String)>;
    async fn notify_subscription_update(
        &self,
        profile: String,
        success: bool,
        message: Option<String>,
    );
    async fn editor_path(&self) -> Option<String>;
    async fn set_editor_path(&self, path: Option<String>);
    async fn pick_editor_path(&self) -> Option<String>;
    async fn open_profile_in_editor(&self, profile_name: &str) -> anyhow::Result<()>;
    async fn get_app_settings(&self) -> AppSettings;
    async fn save_app_settings(&self, settings: AppSettings) -> anyhow::Result<()>;
    async fn runtime_running(&self) -> bool;
    async fn runtime_controller_url(&self) -> Option<String>;
    async fn stop_runtime(&self) -> anyhow::Result<()>;
    async fn runtime_client(&self) -> anyhow::Result<MihomoClient>;
    async fn system_proxy_enabled(&self) -> bool;
    async fn set_system_proxy_enabled(&self, enabled: bool) -> anyhow::Result<()>;
    async fn autostart_enabled(&self) -> bool;
    async fn set_autostart_enabled(&self, enabled: bool) -> anyhow::Result<()>;
    fn supports_system_proxy_control(&self) -> bool;
    fn supports_autostart_control(&self) -> bool;
}

#[derive(Default)]
pub struct RebuildStatus {
    in_progress: AtomicBool,
    last_error: Mutex<Option<String>>,
    last_reason: Mutex<Option<String>>,
}

impl RebuildStatus {
    pub fn snapshot(&self) -> RebuildStatusResponse {
        let last_error = self
            .last_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let last_reason = self
            .last_reason
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        RebuildStatusResponse {
            in_progress: self.in_progress.load(Ordering::SeqCst),
            last_error,
            last_reason,
        }
    }

    pub fn mark_start(&self, reason: &str) {
        self.in_progress.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.last_reason.lock() {
            *guard = Some(reason.to_string());
        }
    }

    pub fn mark_success(&self) {
        self.in_progress.store(false, Ordering::SeqCst);
    }

    pub fn mark_error(&self, err: String) {
        self.in_progress.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = Some(err);
        }
    }
}

#[derive(Default)]
pub struct RuntimeTrafficState {
    initialized: bool,
    last_at: Option<Instant>,
    last_up_total: u64,
    last_down_total: u64,
    peak_up_rate: u64,
    peak_down_rate: u64,
}

impl RuntimeTrafficState {
    pub fn snapshot(
        &mut self,
        upload_total: u64,
        download_total: u64,
        connections: usize,
    ) -> RuntimeTrafficSnapshotResponse {
        let now = Instant::now();
        let mut up_rate = 0;
        let mut down_rate = 0;

        if self.initialized {
            if let Some(last_at) = self.last_at {
                let elapsed_secs = now.saturating_duration_since(last_at).as_secs_f64();
                if elapsed_secs > 0.0 {
                    let up_delta = upload_total.saturating_sub(self.last_up_total);
                    let down_delta = download_total.saturating_sub(self.last_down_total);
                    up_rate = ((up_delta as f64) / elapsed_secs).round() as u64;
                    down_rate = ((down_delta as f64) / elapsed_secs).round() as u64;
                }
            }
        } else {
            self.initialized = true;
        }

        self.last_at = Some(now);
        self.last_up_total = upload_total;
        self.last_down_total = download_total;
        self.peak_up_rate = self.peak_up_rate.max(up_rate);
        self.peak_down_rate = self.peak_down_rate.max(down_rate);

        RuntimeTrafficSnapshotResponse {
            up_rate,
            down_rate,
            up_total: upload_total,
            down_total: download_total,
            up_peak: self.peak_up_rate,
            down_peak: self.peak_down_rate,
            connections,
        }
    }
}

#[derive(Clone)]
pub struct AdminApiState<C> {
    pub ctx: C,
    pub http_client: HttpClient,
    pub raw_http_client: HttpClient,
    pub rebuild_status: Arc<RebuildStatus>,
    pub runtime_traffic: Arc<Mutex<RuntimeTrafficState>>,
    pub events: AdminEventBus,
}

impl<C: AdminApiContext> AdminApiState<C> {
    pub fn new(ctx: C, events: AdminEventBus) -> Self {
        let http_client = build_http_client();
        let raw_http_client = build_raw_http_client(&http_client);
        let rebuild_status = Arc::new(RebuildStatus::default());
        let runtime_traffic = Arc::new(Mutex::new(RuntimeTrafficState::default()));
        Self {
            ctx,
            http_client,
            raw_http_client,
            rebuild_status,
            runtime_traffic,
            events,
        }
    }

    pub fn traffic_snapshot(
        &self,
        upload_total: u64,
        download_total: u64,
        connections: usize,
    ) -> RuntimeTrafficSnapshotResponse {
        let mut guard = self
            .runtime_traffic
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.snapshot(upload_total, download_total, connections)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{RebuildStatus, RuntimeTrafficState};

    #[test]
    fn rebuild_status_transitions() {
        let status = RebuildStatus::default();
        let snapshot = status.snapshot();
        assert!(!snapshot.in_progress);
        assert!(snapshot.last_error.is_none());
        assert!(snapshot.last_reason.is_none());

        status.mark_start("import-activate");
        let snapshot = status.snapshot();
        assert!(snapshot.in_progress);
        assert_eq!(snapshot.last_reason.as_deref(), Some("import-activate"));
        assert!(snapshot.last_error.is_none());

        status.mark_error("boom".to_string());
        let snapshot = status.snapshot();
        assert!(!snapshot.in_progress);
        assert_eq!(snapshot.last_error.as_deref(), Some("boom"));
        assert_eq!(snapshot.last_reason.as_deref(), Some("import-activate"));
    }

    #[test]
    fn runtime_traffic_snapshot_tracks_rates_and_peaks() {
        let mut state = RuntimeTrafficState::default();
        let first = state.snapshot(1_000, 2_000, 3);
        assert_eq!(first.up_rate, 0);
        assert_eq!(first.down_rate, 0);
        assert_eq!(first.up_peak, 0);
        assert_eq!(first.down_peak, 0);

        std::thread::sleep(Duration::from_millis(20));
        let second = state.snapshot(1_600, 2_800, 4);
        assert!(second.up_rate > 0);
        assert!(second.down_rate > 0);
        assert_eq!(second.up_peak, second.up_rate);
        assert_eq!(second.down_peak, second.down_rate);
    }
}
