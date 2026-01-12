use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use infiltrator_http::{build_http_client, build_raw_http_client, HttpClient};

use super::models::RebuildStatusResponse;
use super::events::AdminEventBus;

use infiltrator_core::AppSettings;

#[async_trait::async_trait]
pub trait AdminApiContext: Clone + Send + Sync + 'static {
    async fn rebuild_runtime(&self) -> anyhow::Result<()>;
    async fn set_use_bundled_core(&self, enabled: bool);
    async fn refresh_core_version_info(&self);
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

#[derive(Clone)]
pub struct AdminApiState<C> {
    pub ctx: C,
    pub http_client: HttpClient,
    pub raw_http_client: HttpClient,
    pub rebuild_status: Arc<RebuildStatus>,
    pub events: AdminEventBus,
}

impl<C: AdminApiContext> AdminApiState<C> {
    pub fn new(ctx: C, events: AdminEventBus) -> Self {
        let http_client = build_http_client();
        let raw_http_client = build_raw_http_client(&http_client);
        let rebuild_status = Arc::new(RebuildStatus::default());
        Self {
            ctx,
            http_client,
            raw_http_client,
            rebuild_status,
            events,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RebuildStatus;

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
}
