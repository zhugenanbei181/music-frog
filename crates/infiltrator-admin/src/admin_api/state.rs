use super::events::AdminEventBus;
use super::models::{RebuildStatusResponse, RuntimeTrafficSnapshotResponse};
use infiltrator_application::cache_application::CacheApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::connection_application::ConnectionApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::network_application::NetworkApplication;
use infiltrator_application::outbound_application::OutboundApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::profile_reset_application::ProfileResetApplication;
use infiltrator_application::resource_application::ResourceApplication;
use infiltrator_application::runtime_query_application::RuntimeQueryApplication;
use infiltrator_application::sync_application::SyncApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::subscription_source::SubscriptionSource;
use lurker_core_contract::active_profile_reference::ActiveProfileReferenceTarget;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::probe::ProbeOutboundBatchRequest;
use lurker_core_contract::outbound::probe::ProbeOutboundRequest;
use lurker_core_contract::outbound::{
    inventory::{OutboundInventorySnapshot, OutboundObservationSnapshot, OutboundProbeSnapshot},
    probe::{OutboundProbeBatchSnapshot, OutboundSelectionRequest},
};
use lurker_core_contract::routing_intent::{RoutingIntent, RoutingIntentSnapshot};
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::outbound::OutboundPort;
use std::env::var;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreOwnerAction {
    StartCore,
    StopCore,
    RestartCore,
    SelectProfile(ActiveProfileReferenceTarget),
    RecomposeSelectedProfile,
    RetryOwnerSwitch,
}

#[async_trait::async_trait]
pub trait AdminApiContext: Clone + Send + Sync + 'static {
    async fn profile_application(&self) -> anyhow::Result<ProfileApplication>;
    async fn configuration_application(&self) -> anyhow::Result<ConfigurationApplication>;
    async fn doctor_application(&self) -> anyhow::Result<DoctorApplication>;
    async fn profile_reset_application(&self) -> anyhow::Result<ProfileResetApplication>;
    async fn cache_application(&self) -> anyhow::Result<CacheApplication>;
    async fn subscription_source(&self) -> anyhow::Result<Arc<dyn SubscriptionSource>>;
    async fn sync_application(&self) -> anyhow::Result<SyncApplication>;
    async fn version_application(&self) -> anyhow::Result<VersionApplication>;
    /// Public-egress probe owned by the host composition; the REST adapter
    /// never constructs an HTTP client itself.
    async fn network_application(&self) -> anyhow::Result<NetworkApplication> {
        Err(anyhow::anyhow!(
            "public IP probe is unavailable in this host"
        ))
    }
    /// Typed outbound operations always use the active Core owner's port and
    /// scope source. A host without both capabilities fails closed.
    async fn outbound_owner(&self) -> anyhow::Result<AdminOutboundOwner> {
        Err(anyhow::anyhow!(
            "typed outbound owner is unavailable in this host"
        ))
    }
    /// Owner-authored routing intent readback, including its observed Core
    /// scope and typed availability status.
    async fn routing_intent_snapshot(
        &self,
    ) -> Result<(Option<CoreScope>, RoutingIntentSnapshot), Failure> {
        Err(Failure::unsupported(
            "typed routing intent readback is unavailable in this host",
        ))
    }
    /// Apply a routing intent through the existing scoped Core command path.
    /// The supplied scope must be the one observed by the requesting client.
    async fn apply_routing_intent(
        &self,
        _scope: CoreScope,
        _intent: RoutingIntent,
    ) -> Result<(Option<CoreScope>, RoutingIntentSnapshot), Failure> {
        Err(Failure::unsupported(
            "typed routing intent control is unavailable in this host",
        ))
    }

    async fn apply_core_owner_action(&self, action: CoreOwnerAction) -> anyhow::Result<()>;
    async fn profile_controller_url(&self) -> anyhow::Result<Option<String>>;
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
    async fn runtime_running(&self) -> bool;
    async fn runtime_controller_url(&self) -> Option<String>;
    async fn connection_application(&self) -> anyhow::Result<ConnectionApplication> {
        Err(anyhow::anyhow!(
            "typed connection application is unavailable in this host"
        ))
    }
    async fn runtime_query_application(&self) -> anyhow::Result<RuntimeQueryApplication> {
        Err(anyhow::anyhow!(
            "typed runtime query application is unavailable in this host"
        ))
    }
    async fn resource_application(&self) -> anyhow::Result<ResourceApplication>;
    async fn system_proxy_enabled(&self) -> bool;
    async fn set_system_proxy_enabled(&self, enabled: bool) -> anyhow::Result<()>;
    async fn autostart_enabled(&self) -> bool;
    async fn set_autostart_enabled(&self, enabled: bool) -> anyhow::Result<()>;
    fn supports_system_proxy_control(&self) -> bool;
    fn supports_autostart_control(&self) -> bool;

    /// Read the host credential used by WebDAV. The REST layer never chooses
    /// a keyring implementation; each host supplies this capability.
    async fn webdav_password(&self) -> Option<String>;

    /// Persist the host credential used by WebDAV. A failed write must abort
    /// the settings update so the file and keyring cannot diverge silently.
    async fn set_webdav_password(&self, password: &str) -> anyhow::Result<()>;
}

/// Admin's typed view of the already-running Core owner. Scope is sampled
/// from the injected owner for every operation; callers cannot construct or
/// guess one from engine names, controller URLs, or revision counters.
#[derive(Clone)]
pub struct AdminOutboundOwner {
    application: OutboundApplication,
    scope_source: Arc<dyn CoreScopeSource>,
}

impl AdminOutboundOwner {
    pub fn new(port: Arc<dyn OutboundPort>, scope_source: Arc<dyn CoreScopeSource>) -> Self {
        Self {
            application: OutboundApplication::new(port),
            scope_source,
        }
    }

    pub async fn observe(&self) -> Result<(CoreScope, OutboundObservationSnapshot), Failure> {
        let scope = self.active_scope()?;
        let observation = self.application.observation(scope.clone()).await?;
        self.verify_active_scope(&scope)?;
        Ok((scope, observation))
    }

    pub async fn inventory(&self) -> Result<(CoreScope, OutboundInventorySnapshot), Failure> {
        let scope = self.active_scope()?;
        let inventory = self.application.inventory(scope.clone()).await?;
        self.verify_active_scope(&scope)?;
        Ok((scope, inventory))
    }

    pub async fn select(
        &self,
        scope: &CoreScope,
        request: &OutboundSelectionRequest,
    ) -> Result<OutboundInventorySnapshot, Failure> {
        self.verify_active_scope(scope)?;
        let snapshot = self.application.select_request(scope, request).await?;
        self.verify_active_scope(scope)?;
        Ok(snapshot)
    }

    pub async fn probe(
        &self,
        scope: &CoreScope,
        request: &ProbeOutboundRequest,
    ) -> Result<OutboundProbeSnapshot, Failure> {
        self.verify_active_scope(scope)?;
        let snapshot = self.application.probe_request(scope, request).await?;
        self.verify_active_scope(scope)?;
        Ok(snapshot)
    }

    pub async fn probe_batch(
        &self,
        scope: &CoreScope,
        request: &ProbeOutboundBatchRequest,
    ) -> Result<OutboundProbeBatchSnapshot, Failure> {
        self.verify_active_scope(scope)?;
        let snapshot = self.application.probe_batch_request(scope, request).await?;
        self.verify_active_scope(scope)?;
        Ok(snapshot)
    }

    fn active_scope(&self) -> Result<CoreScope, Failure> {
        match self.scope_source.scope_observation() {
            CoreScopeObservation::Observed(scope) => Ok(scope),
            CoreScopeObservation::Unavailable => Err(Failure::new(
                ErrorCode::NotReady,
                "no active Core scope is available for Admin outbound operations",
                true,
            )),
        }
    }

    fn verify_active_scope(&self, expected: &CoreScope) -> Result<(), Failure> {
        match self.scope_source.scope_observation() {
            CoreScopeObservation::Observed(current) if &current == expected => Ok(()),
            CoreScopeObservation::Observed(_) | CoreScopeObservation::Unavailable => {
                Err(Failure::new(
                    ErrorCode::InvalidState,
                    "Admin outbound operation belongs to a retired Core scope",
                    false,
                ))
            }
        }
    }
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

#[derive(Clone, Copy, Default)]
enum RuntimeTrafficBaseline {
    #[default]
    Unobserved,
    Observed {
        at: Instant,
        upload_total: u64,
        download_total: u64,
    },
}

#[derive(Default)]
pub struct RuntimeTrafficState {
    baseline: RuntimeTrafficBaseline,
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
        let (up_rate, down_rate) = match self.baseline {
            RuntimeTrafficBaseline::Unobserved => (0, 0),
            RuntimeTrafficBaseline::Observed {
                at,
                upload_total: last_up_total,
                download_total: last_down_total,
            } => {
                let elapsed_secs = now.saturating_duration_since(at).as_secs_f64();
                if elapsed_secs > 0.0 {
                    let up_delta = upload_total.saturating_sub(last_up_total);
                    let down_delta = download_total.saturating_sub(last_down_total);
                    (
                        ((up_delta as f64) / elapsed_secs).round() as u64,
                        ((down_delta as f64) / elapsed_secs).round() as u64,
                    )
                } else {
                    (0, 0)
                }
            }
        };

        self.baseline = RuntimeTrafficBaseline::Observed {
            at: now,
            upload_total,
            download_total,
        };
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
    pub rebuild_status: Arc<RebuildStatus>,
    pub runtime_traffic: Arc<Mutex<RuntimeTrafficState>>,
    pub events: AdminEventBus,
    pub auth_token: Option<String>,
}

impl<C: AdminApiContext> AdminApiState<C> {
    pub fn new(ctx: C, events: AdminEventBus) -> Self {
        let auth_token = var("INFILTRATOR_ADMIN_TOKEN")
            .or_else(|_| var("INFILTRATOR_AUTH_TOKEN"))
            .ok()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        Self::with_auth_token(ctx, events, auth_token)
    }

    pub fn with_auth_token(ctx: C, events: AdminEventBus, auth_token: Option<String>) -> Self {
        let rebuild_status = Arc::new(RebuildStatus::default());
        let runtime_traffic = Arc::new(Mutex::new(RuntimeTrafficState::default()));
        let auth_token = auth_token
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        Self {
            ctx,
            rebuild_status,
            runtime_traffic,
            events,
            auth_token,
        }
    }

    pub fn set_auth_token(&mut self, token: Option<String>) {
        self.auth_token = token
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
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
    use super::{RebuildStatus, RuntimeTrafficBaseline, RuntimeTrafficState};
    #[cfg(test)]
    use std::thread::sleep;
    use std::time::Duration;

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
        assert!(matches!(state.baseline, RuntimeTrafficBaseline::Unobserved));
        let first = state.snapshot(1_000, 2_000, 3);
        assert!(matches!(
            state.baseline,
            RuntimeTrafficBaseline::Observed {
                upload_total: 1_000,
                download_total: 2_000,
                ..
            }
        ));
        assert_eq!(first.up_rate, 0);
        assert_eq!(first.down_rate, 0);
        assert_eq!(first.up_peak, 0);
        assert_eq!(first.down_peak, 0);

        sleep(Duration::from_millis(20));
        let second = state.snapshot(1_600, 2_800, 4);
        assert!(second.up_rate > 0);
        assert!(second.down_rate > 0);
        assert_eq!(second.up_peak, second.up_rate);
        assert_eq!(second.down_peak, second.down_rate);
    }
}
