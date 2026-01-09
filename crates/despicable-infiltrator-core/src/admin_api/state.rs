use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use log::warn;
use reqwest::Client;

use super::models::RebuildStatusResponse;

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
    pub http_client: Client,
    pub raw_http_client: Client,
    pub rebuild_status: Arc<RebuildStatus>,
}

impl<C: AdminApiContext> AdminApiState<C> {
    pub fn new(ctx: C) -> Self {
        let http_client = Client::builder()
            .user_agent("Mihomo-Despicable-Infiltrator")
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|err| {
                warn!("failed to build http client: {err}");
                Client::new()
            });
        let raw_http_client = Client::builder()
            .user_agent("Mihomo-Despicable-Infiltrator")
            .timeout(Duration::from_secs(30))
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .unwrap_or_else(|err| {
                warn!("failed to build raw http client: {err}");
                http_client.clone()
            });
        let rebuild_status = Arc::new(RebuildStatus::default());
        Self {
            ctx,
            http_client,
            raw_http_client,
            rebuild_status,
        }
    }
}
