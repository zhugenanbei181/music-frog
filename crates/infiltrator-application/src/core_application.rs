use self::execution::closed_failure;
use crate::command_application::handler::CommandHandler;
use crate::command_capability::validate_terminal_scope;
use crate::core_capability_application::CoreCapabilityApplication;
use crate::log_application::LogApplication;
use crate::log_stream::LogPump;
use crate::outbound_application::preferences::OutboundUiPreferencesApplication;
use crate::speedtest_application::{
    SpeedtestApplication, SpeedtestCompletionAdmission, SpeedtestTaskOutcome,
};
use futures_util::lock;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization;
use infiltrator_contract::command::{CommandIntent, CommandResult, RequestId};
use infiltrator_contract::core_identity::{
    CoreCommandOutcome, CoreCommandRequest, CoreGeneration, CoreIdentity, CoreScope,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::DocumentInspection;
use infiltrator_contract::profile_document_identity::ProfileDocumentIdentity;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{
    CoreEvent, CoreLifecycle, CoreLifecycleSnapshot, CoreSnapshot,
};
use infiltrator_contract::speedtest::{SpeedtestRunId, SpeedtestSnapshot};
use infiltrator_domain::core_state;
use infiltrator_domain::core_state::{CoreState, CoreStateMachine};
use infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerLease;
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::core_lifecycle::{CoreLifecyclePort, CoreOwnerTerminationPort};
use infiltrator_ports::core_process::{CoreProcess, CoreProcessStartContext, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::OutboundPort;
use infiltrator_ports::outbound::OutboundScopeSource;
use infiltrator_ports::overview::OverviewReader;
use infiltrator_ports::profile_document_codec::ProfileDocumentSnippetOutcome;
use infiltrator_ports::profile_document_codec::ProfileMixinStudioProjection;
use infiltrator_ports::profile_document_codec::ProfileMixinToggleId;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, RwLock};
use std::time;

#[path = "core_watchdog.rs"]
mod watchdog_runtime;

#[path = "core_application/capability.rs"]
mod capability;
mod command_name;
mod dispatch;
mod execution;
mod runtime_projection;

const EVENT_CAPACITY: usize = 256;
const DISPATCH_CAPACITY: usize = 256;
const DEFAULT_READINESS_TIMEOUT: time::Duration = time::Duration::from_secs(15);
const DEFAULT_READINESS_POLL_INTERVAL: time::Duration = time::Duration::from_millis(250);
static NEXT_SESSION_NAMESPACE: AtomicU64 = AtomicU64::new(1);

/// Readiness retry policy expressed entirely in standard-library values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadinessPolicy {
    pub timeout: time::Duration,
    pub poll_interval: time::Duration,
}

impl Default for ReadinessPolicy {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_READINESS_TIMEOUT,
            poll_interval: DEFAULT_READINESS_POLL_INTERVAL,
        }
    }
}

struct StateMirror {
    state: CoreState,
    revision: u64,
}

enum ActiveProfileOwnerBindingState {
    Unbound,
    Bound(Arc<dyn ActiveProfileOwnerLease>),
    Releasing,
    Released,
}

struct Inner {
    logs: LogApplication,
    log_pump: Mutex<Option<LogPump>>,
    telemetry: lock::Mutex<telemetry::TelemetryState>,
    process: Arc<dyn CoreProcess>,
    readiness: Arc<dyn CoreReadiness>,
    overview: Option<Arc<dyn OverviewReader>>,
    readiness_policy: ReadinessPolicy,
    runtime: Arc<dyn ApplicationRuntime>,
    command_handler: RwLock<Option<Arc<dyn CommandHandler>>>,
    outbound_port: RwLock<Option<Arc<dyn OutboundPort>>>,
    outbound_ui_preferences: OutboundUiPreferencesApplication,
    speedtest: RwLock<Option<SpeedtestApplication>>,
    core_capabilities: RwLock<CoreCapabilityApplication>,
    dispatch_tx: SyncSender<DispatchedMessage>,
    commands: lock::Mutex<()>,
    operation: lock::Mutex<()>,
    closed: AtomicBool,
    active_command: AtomicBool,
    state: RwLock<StateMirror>,
    watchdog: Mutex<watchdog_runtime::WatchdogRuntime>,
    next_request_id: AtomicU64,
    session_namespace: u64,
    core_identity: RwLock<Option<CoreIdentity>>,
    active_profile_owner_binding: Mutex<ActiveProfileOwnerBindingState>,
    next_session_sequence: AtomicU64,
    events: Mutex<VecDeque<CoreEvent>>,
}

pub(super) struct DispatchedCommand {
    request_id: RequestId,
    request: CoreCommandRequest,
    reply: Sender<CoreCommandOutcome>,
}

pub(super) enum DispatchedMessage {
    Command(DispatchedCommand),
    SpeedtestCompleted {
        scope: CoreScope,
        run_id: SpeedtestRunId,
        outcome: SpeedtestTaskOutcome,
    },
}

/// Explicit state of a surface's observed Core scope. A missing scope is a
/// first-class fail-closed state, not a flag paired with a guessed identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreScopeObservation {
    Unavailable,
    Observed(CoreScope),
}

/// The single application owner of Core lifecycle operations.
///
/// `CoreApplication` serializes side-effecting commands, drives the pure
/// domain reducer, and publishes bounded contract events. It is intentionally
/// constructed with ports rather than `MihomoClient`, `ConfigManager`, or an
/// operating-system implementation.
#[derive(Clone)]
pub struct CoreApplication {
    inner: Arc<Inner>,
}

impl OutboundScopeSource for CoreApplication {
    fn current_scope(&self) -> Option<CoreScope> {
        let snapshot = self.snapshot();
        match &snapshot.lifecycle {
            CoreLifecycle::Ready | CoreLifecycle::Running => snapshot.scope(),
            CoreLifecycle::Stopped
            | CoreLifecycle::Starting
            | CoreLifecycle::Stopping
            | CoreLifecycle::Failed => None,
        }
    }
}

impl CoreApplication {
    /// Return the exact persisted active-profile owner binding installed when
    /// this application was composed. The bound generation is stable across
    /// later process restarts and must not be reconstructed from a snapshot.
    pub fn active_profile_reference_write_authorization(
        &self,
    ) -> Result<ActiveProfileReferenceWriteAuthorization, Failure> {
        let state = self
            .inner
            .active_profile_owner_binding
            .lock()
            .map_err(|_| {
                Failure::new(
                    ErrorCode::Internal,
                    "active Core owner binding lock is poisoned",
                    false,
                )
            })?;
        match &*state {
            ActiveProfileOwnerBindingState::Bound(binding) => {
                Ok(ActiveProfileReferenceWriteAuthorization::BoundOwner {
                    identity: binding.identity().clone(),
                    generation: binding.generation(),
                })
            }
            ActiveProfileOwnerBindingState::Unbound => Err(Failure::new(
                ErrorCode::NotReady,
                "Core has no active-profile owner binding",
                true,
            )),
            ActiveProfileOwnerBindingState::Releasing => Err(Failure::new(
                ErrorCode::NotReady,
                "Core active-profile owner binding is being released",
                true,
            )),
            ActiveProfileOwnerBindingState::Released => Err(Failure::new(
                ErrorCode::InvalidState,
                "Core active-profile owner binding has been released",
                false,
            )),
        }
    }

    /// Tie the selected engine reference to this single Core owner. The guard
    /// is released only after `close` successfully reaches terminal state.
    pub fn install_active_profile_owner_binding(
        &self,
        binding: Box<dyn ActiveProfileOwnerLease>,
    ) -> Result<(), Failure> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(closed_failure());
        }
        let mut state = self
            .inner
            .active_profile_owner_binding
            .lock()
            .expect("active profile owner binding lock");
        match &*state {
            ActiveProfileOwnerBindingState::Unbound => {
                *state = ActiveProfileOwnerBindingState::Bound(Arc::from(binding));
                Ok(())
            }
            ActiveProfileOwnerBindingState::Bound(_) => Err(Failure::new(
                ErrorCode::InvalidState,
                "Core owner already has an active profile reference binding",
                false,
            )),
            ActiveProfileOwnerBindingState::Released => Err(Failure::new(
                ErrorCode::InvalidState,
                "a closed Core owner cannot acquire a profile reference binding",
                false,
            )),
            ActiveProfileOwnerBindingState::Releasing => Err(Failure::new(
                ErrorCode::NotReady,
                "Core owner profile binding release is still in progress",
                true,
            )),
        }
    }

    pub fn new(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        Self::build(
            process,
            readiness,
            None,
            ReadinessPolicy::default(),
            runtime,
        )
    }

    /// Construct a lifecycle-only application with an explicit readiness
    /// policy. Tests and embedders can choose a smaller budget without
    /// changing the production default.
    pub fn new_with_policy(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        readiness_policy: ReadinessPolicy,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        Self::build(process, readiness, None, readiness_policy, runtime)
    }

    /// Construct the application with the optional Overview reader wired in
    /// for telemetry sampling.
    pub fn new_with_overview(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        overview: Arc<dyn OverviewReader>,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        Self::build(
            process,
            readiness,
            Some(overview),
            ReadinessPolicy::default(),
            runtime,
        )
    }

    /// Construct an application with both an Overview port and an explicit
    /// readiness policy.
    pub fn new_with_overview_and_policy(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        overview: Arc<dyn OverviewReader>,
        readiness_policy: ReadinessPolicy,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        Self::build(
            process,
            readiness,
            Some(overview),
            readiness_policy,
            runtime,
        )
    }

    fn build(
        process: Arc<dyn CoreProcess>,
        readiness: Arc<dyn CoreReadiness>,
        overview: Option<Arc<dyn OverviewReader>>,
        readiness_policy: ReadinessPolicy,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        let (dispatch_tx, dispatch_rx) = sync_channel(DISPATCH_CAPACITY);
        let core_identity = process.core_identity();
        let session_namespace = NEXT_SESSION_NAMESPACE
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        let inner = Arc::new(Inner {
            logs: LogApplication::default(),
            log_pump: Mutex::new(None),
            telemetry: lock::Mutex::new(telemetry::TelemetryState::default()),
            process,
            readiness,
            overview,
            readiness_policy,
            runtime: Arc::clone(&runtime),
            command_handler: RwLock::new(None),
            outbound_port: RwLock::new(None),
            outbound_ui_preferences: OutboundUiPreferencesApplication::default(),
            speedtest: RwLock::new(None),
            core_capabilities: RwLock::new(CoreCapabilityApplication::without_provider()),
            dispatch_tx,
            commands: lock::Mutex::new(()),
            operation: lock::Mutex::new(()),
            closed: AtomicBool::new(false),
            active_command: AtomicBool::new(false),
            state: RwLock::new(StateMirror {
                state: CoreState::Idle { generation: 0 },
                revision: 0,
            }),
            watchdog: Mutex::new(watchdog_runtime::WatchdogRuntime::default()),
            next_request_id: AtomicU64::new(1),
            session_namespace,
            core_identity: RwLock::new(core_identity),
            active_profile_owner_binding: Mutex::new(ActiveProfileOwnerBindingState::Unbound),
            next_session_sequence: AtomicU64::new(1),
            events: Mutex::new(VecDeque::with_capacity(EVENT_CAPACITY)),
        });
        runtime_projection::spawn_dispatch_worker(Arc::downgrade(&inner), dispatch_rx, runtime);
        Self { inner }
    }

    /// Install the non-lifecycle command facade used by a full product
    /// composition. Lifecycle and routing-intent commands remain owned by this
    /// application; every other intent is delegated to the handler or
    /// rejected with a typed Unsupported failure.
    pub fn install_command_handler(&self, handler: Arc<dyn CommandHandler>) {
        let mut slot = self
            .inner
            .command_handler
            .write()
            .expect("command handler lock");
        assert!(
            !self.inner.closed.load(Ordering::Acquire),
            "cannot configure a closed application"
        );
        *slot = Some(handler);
    }

    /// Inspect an editor-owned buffer through the configured engine codec.
    /// This synchronous path is reserved for pure format operations and does
    /// not enter the Core command queue or contact a running engine.
    pub fn inspect_profile_buffer(
        &self,
        identity: &ProfileDocumentIdentity,
        content: &str,
    ) -> Result<DocumentInspection, Failure> {
        self.editor_command_handler()?
            .inspect_profile_buffer(identity, content)
    }

    /// Insert a codec-owned snippet into an editor buffer without mutating
    /// storage or runtime state.
    pub fn insert_profile_snippet(
        &self,
        identity: &ProfileDocumentIdentity,
        content: &str,
        snippet_id: &str,
        line: usize,
        column: usize,
    ) -> Result<ProfileDocumentSnippetOutcome, Failure> {
        self.editor_command_handler()?
            .insert_profile_snippet(identity, content, snippet_id, line, column)
    }

    /// Run the composed document and Mixin codec preflight without applying
    /// the returned mutation to storage or a running engine.
    pub fn inspect_mixin_draft(
        &self,
        identity: &ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<(), Failure> {
        self.editor_command_handler()?
            .inspect_mixin_draft(identity, document_content, mixin_yaml)
    }

    pub fn inspect_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
    ) -> Result<bool, Failure> {
        self.editor_command_handler()?
            .inspect_mixin_toggle(identity, mixin_yaml, toggle_id)
    }

    pub fn set_mixin_toggle(
        &self,
        identity: &ProfileDocumentIdentity,
        mixin_yaml: &str,
        toggle_id: ProfileMixinToggleId,
        enabled: bool,
    ) -> Result<String, Failure> {
        self.editor_command_handler()?
            .set_mixin_toggle(identity, mixin_yaml, toggle_id, enabled)
    }

    pub fn inspect_mixin_studio(
        &self,
        identity: &ProfileDocumentIdentity,
        document_content: &str,
        mixin_yaml: &str,
    ) -> Result<ProfileMixinStudioProjection, Failure> {
        self.editor_command_handler()?
            .inspect_mixin_studio(identity, document_content, mixin_yaml)
    }

    fn editor_command_handler(&self) -> Result<Arc<dyn CommandHandler>, Failure> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "application is closed",
                false,
            ));
        }
        self.inner
            .command_handler
            .read()
            .expect("command handler lock")
            .clone()
            .ok_or_else(|| Failure::unsupported("profile editor codec is not composed"))
    }

    /// Register the owner-scoped outbound port shared by command dispatch and
    /// surface observation. The port is installed once so its adapter-local
    /// inventory revision cannot fork between consumers.
    pub fn install_outbound_port(&self, port: Arc<dyn OutboundPort>) -> Result<(), Failure> {
        let mut slot = self
            .inner
            .outbound_port
            .write()
            .expect("outbound port lock");
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(closed_failure());
        }
        if slot.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "outbound port is already installed for this core owner",
                false,
            ));
        }
        *slot = Some(port);
        Ok(())
    }

    /// Install the single speedtest state owner associated with this Core actor.
    pub fn install_speedtest_application(
        &self,
        application: SpeedtestApplication,
    ) -> Result<(), Failure> {
        let mut slot = self
            .inner
            .speedtest
            .write()
            .expect("speedtest application lock");
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(closed_failure());
        }
        if slot.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "speedtest application is already installed for this Core owner",
                false,
            ));
        }
        *slot = Some(application);
        Ok(())
    }

    /// Read the canonical speedtest projection from the Core-owned run state.
    pub fn speedtest_snapshot(&self) -> SpeedtestSnapshot {
        self.inner
            .speedtest
            .read()
            .expect("speedtest application lock")
            .as_ref()
            .map(SpeedtestApplication::snapshot)
            .unwrap_or_else(|| {
                SpeedtestSnapshot::unavailable(Failure::unsupported(
                    "speedtest is not configured for this Core owner",
                ))
            })
    }

    pub(crate) fn speedtest_application(&self) -> Result<SpeedtestApplication, Failure> {
        self.inner
            .speedtest
            .read()
            .expect("speedtest application lock")
            .clone()
            .ok_or_else(|| Failure::unsupported("speedtest is not configured for this Core owner"))
    }

    pub(super) fn finish_speedtest_task(
        &self,
        scope: CoreScope,
        run_id: SpeedtestRunId,
        outcome: SpeedtestTaskOutcome,
    ) -> SpeedtestCompletionAdmission {
        let Ok(speedtest) = self.speedtest_application() else {
            return SpeedtestCompletionAdmission::IgnoredStaleRun {
                completed: run_id,
                current: None,
            };
        };
        let current_scope = self.snapshot().scope();
        speedtest.finish(run_id, &scope, current_scope.as_ref(), outcome)
    }

    /// Clone the installed owner port for composition into commands and
    /// read-only surface projections.
    pub fn outbound_port(&self) -> Option<Arc<dyn OutboundPort>> {
        self.inner
            .outbound_port
            .read()
            .expect("outbound port lock")
            .clone()
    }

    /// Clone the owner-scoped outbound preference state shared by typed
    /// commands and surface readback.
    pub fn outbound_ui_preferences_application(&self) -> OutboundUiPreferencesApplication {
        self.inner.outbound_ui_preferences.clone()
    }

    pub fn log_application(&self) -> LogApplication {
        self.inner.logs.clone()
    }
    pub fn install_log_gateway(&self, gateway: Arc<dyn RuntimeGateway>) -> Result<(), Failure> {
        let mut slot = self.inner.log_pump.lock().expect("log driver");
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(closed_failure());
        }
        slot.take();
        let weak = Arc::downgrade(&self.inner);
        let pump = LogPump::spawn(
            self.inner.logs.clone(),
            gateway,
            self.inner.runtime.clone(),
            move || {
                weak.upgrade()
                    .map(|inner| CoreApplication { inner }.snapshot().lifecycle_snapshot())
            },
        )?;
        *slot = Some(pump);
        Ok(())
    }
    /// Return the latest immutable contract projection.
    pub fn snapshot(&self) -> CoreSnapshot {
        let mirror = self.inner.state.read().expect("core state lock");
        runtime_projection::snapshot_from_state(
            &mirror.state,
            mirror.revision,
            self.watchdog_snapshot(),
            self.current_core_identity(),
        )
    }

    /// Current lifecycle generation used to fence delayed surface work.
    pub fn generation(&self) -> u64 {
        self.snapshot().generation
    }

    /// Identity of the active core session. Unlike generation, this also
    /// separates sessions created by different application instances.
    pub fn session_token(&self) -> Option<SessionToken> {
        self.snapshot().session_token
    }

    /// Reject delayed work that belongs to a previous core session.
    pub fn check_session(&self, session_token: SessionToken) -> Result<(), Failure> {
        if self.session_token() == Some(session_token) {
            Ok(())
        } else {
            Err(Failure::new(
                ErrorCode::InvalidState,
                "stale core session token",
                false,
            ))
        }
    }

    /// Drain a bounded batch of application events for a frame-driven or FFI
    /// surface. The queue drops the oldest event when it reaches capacity.
    pub fn drain_events(&self) -> Vec<CoreEvent> {
        let mut events = self.inner.events.lock().expect("core event queue lock");
        events.drain(..).collect()
    }

    /// Adopt a host process that was started by another host lifecycle
    /// callback. This is the composition-root seam for Android `VpnService`
    /// and desktop boot attach flows; it never calls `start` on the process.
    /// An adopted process that fails readiness is stopped before failure is
    /// published, so an unverified child cannot remain silently active.
    pub async fn adopt_if_running(&self) -> Result<bool, Failure> {
        let _operation = self.inner.operation.lock().await;
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(closed_failure());
        }
        if !matches!(self.current_state(), CoreState::Idle { .. }) {
            return Ok(false);
        }

        let status = self.inner.process.status().await.map_err(Failure::from)?;
        if !matches!(status, CoreLifecycle::Running | CoreLifecycle::Ready) {
            return Ok(false);
        }

        self.clear_active_core_version();
        let session_token = self.allocate_session_token();
        self.apply_domain_event(core_state::CoreEvent::StartRequested { session_token });
        match self
            .wait_for_readiness(session_token, self.inner.readiness_policy.timeout)
            .await
        {
            Ok(endpoint) => {
                self.refresh_active_core_version().await;
                self.apply_domain_event(core_state::CoreEvent::ReadinessSuccess {
                    session_token,
                    endpoint,
                });
                Ok(true)
            }
            Err(error) => {
                let failure = self.stop_after_readiness_failure(error).await;
                self.apply_domain_event(core_state::CoreEvent::StartFailed {
                    session_token,
                    error: failure.message.clone(),
                });
                Err(failure)
            }
        }
    }

    fn allocate_request_id(&self) -> RequestId {
        RequestId::new(self.inner.next_request_id.fetch_add(1, Ordering::Relaxed))
    }

    fn allocate_session_token(&self) -> SessionToken {
        let sequence = self
            .inner
            .next_session_sequence
            .fetch_add(1, Ordering::Relaxed)
            .max(1);
        SessionToken::new((u128::from(self.inner.session_namespace) << 64) | u128::from(sequence))
    }

    async fn start_locked(&self) -> Result<(), Failure> {
        if !matches!(
            self.current_state(),
            CoreState::Idle { .. } | CoreState::Failed { .. }
        ) {
            return Err(runtime_projection::invalid_state_failure("start"));
        }

        self.clear_active_core_version();

        // A manual start begins a fresh watchdog window. The recovery path
        // marks itself `Restarting` before calling this method, so its retry
        // counters remain intact until the new session is proven ready.
        self.reset_watchdog_for_start();

        let session_token = self.allocate_session_token();
        self.apply_domain_event(core_state::CoreEvent::StartRequested { session_token });
        let Some(identity) = self.inner.process.core_identity() else {
            let failure = Failure::new(
                ErrorCode::InvalidState,
                "Core process launch requires a stable Core identity",
                false,
            );
            self.apply_domain_event(core_state::CoreEvent::StartFailed {
                session_token,
                error: failure.message.clone(),
            });
            return Err(failure);
        };
        let start_context = CoreProcessStartContext::Managed {
            identity,
            generation: CoreGeneration::new(self.generation()),
            session_token,
        };
        let admission = match self.inner.process.prepare_start(&start_context).await {
            Ok(admission) => admission,
            Err(error) => {
                let failure = Failure::from(error);
                self.apply_domain_event(core_state::CoreEvent::StartFailed {
                    session_token,
                    error: failure.message.clone(),
                });
                return Err(failure);
            }
        };
        let owner_binding = {
            let state = self
                .inner
                .active_profile_owner_binding
                .lock()
                .expect("active profile owner binding lock");
            match &*state {
                ActiveProfileOwnerBindingState::Bound(binding) => Some(Arc::clone(binding)),
                ActiveProfileOwnerBindingState::Unbound
                | ActiveProfileOwnerBindingState::Releasing
                | ActiveProfileOwnerBindingState::Released => None,
            }
        };
        if let Some(binding) = owner_binding {
            if let Err(error) = binding.record_start_context(&start_context).await {
                let failure = Failure::from(error);
                self.apply_domain_event(core_state::CoreEvent::StartFailed {
                    session_token,
                    error: failure.message.clone(),
                });
                return Err(failure);
            }
        }
        if admission == infiltrator_ports::core_process::CoreProcessStartAdmission::LaunchNewProcess
        {
            if let Err(error) = self.inner.process.start(start_context).await {
                let message = error.to_string();
                self.apply_domain_event(core_state::CoreEvent::StartFailed {
                    session_token,
                    error: message,
                });
                return Err(Failure::from(error));
            }
        }
        match self
            .wait_for_readiness(session_token, self.inner.readiness_policy.timeout)
            .await
        {
            Ok(endpoint) => {
                self.refresh_active_core_version().await;
                self.apply_domain_event(core_state::CoreEvent::ReadinessSuccess {
                    session_token,
                    endpoint,
                });
                Ok(())
            }
            Err(error) => {
                let failure = self.stop_after_readiness_failure(error).await;
                self.apply_domain_event(core_state::CoreEvent::StartFailed {
                    session_token,
                    error: failure.message.clone(),
                });
                Err(failure)
            }
        }
    }

    async fn stop_locked(&self) -> Result<(), Failure> {
        if matches!(self.current_state(), CoreState::Idle { .. }) {
            return self.inner.process.stop().await.map_err(Failure::from);
        }

        let Some(session_token) = self.session_token() else {
            return Err(runtime_projection::invalid_state_failure("stop"));
        };
        self.apply_domain_event(core_state::CoreEvent::StopRequested { session_token });
        if let Err(error) = self.inner.process.stop().await {
            let message = error.to_string();
            self.apply_domain_event(core_state::CoreEvent::StopFailed {
                session_token,
                error: message,
            });
            return Err(Failure::from(error));
        }

        self.apply_domain_event(core_state::CoreEvent::StopCompleted { session_token });
        self.reset_watchdog_after_stop();
        Ok(())
    }

    async fn restart_locked(&self) -> Result<(), Failure> {
        self.stop_locked().await?;
        self.start_locked().await
    }

    fn current_state(&self) -> CoreState {
        self.inner
            .state
            .read()
            .expect("core state lock")
            .state
            .clone()
    }

    fn apply_domain_event(&self, event: core_state::CoreEvent) {
        let snapshot = {
            let mut mirror = self.inner.state.write().expect("core state lock");
            let (state, warning) = CoreStateMachine::step(&mirror.state, event);
            mirror.state = state;
            mirror.revision = mirror.revision.saturating_add(1);
            if let Some(warning) = warning {
                log::warn!(target: "infiltrator-application", "core domain transition warning: {warning}");
            }
            runtime_projection::snapshot_from_state(
                &mirror.state,
                mirror.revision,
                self.watchdog_snapshot(),
                self.current_core_identity(),
            )
        };
        self.inner.logs.observe_core(&snapshot);
        if let Some(speedtest) = self
            .inner
            .speedtest
            .read()
            .expect("speedtest application lock")
            .as_ref()
        {
            let scope = snapshot.scope();
            speedtest.invalidate_scope(scope.as_ref());
        }
        self.push_event(CoreEvent::SnapshotUpdated(snapshot));
    }

    fn push_event(&self, event: CoreEvent) {
        let mut events = self.inner.events.lock().expect("core event queue lock");
        if events.len() == EVENT_CAPACITY {
            events.pop_front();
        }
        events.push_back(event);
    }

    async fn stop_after_readiness_failure(&self, error: PortError) -> Failure {
        let mut failure = Failure::from(error);
        if let Err(cleanup_error) = self.inner.process.stop().await {
            failure.message.push_str(&format!(
                "; stopping the core after readiness failure also failed: {cleanup_error}"
            ));
        }
        failure
    }

    async fn wait_for_readiness(
        &self,
        session_token: SessionToken,
        timeout: time::Duration,
    ) -> Result<String, PortError> {
        let deadline = time::Instant::now() + timeout;
        loop {
            if self.session_token() != Some(session_token) {
                return Err(PortError::Failed("stale core session token".to_string()));
            }
            match self.inner.process.status().await {
                Ok(CoreLifecycle::Starting)
                | Ok(CoreLifecycle::Ready)
                | Ok(CoreLifecycle::Running) => {}
                Ok(_) => {
                    return Err(PortError::Failed(
                        "core process exited before readiness".to_string(),
                    ));
                }
                Err(error) => return Err(error),
            }

            let probe_error = match self.inner.readiness.probe().await {
                Ok(endpoint) => return Ok(endpoint),
                Err(error) => error,
            };

            if time::Instant::now() >= deadline {
                return Err(probe_error);
            }
            self.inner
                .runtime
                .sleep(self.inner.readiness_policy.poll_interval)
                .await;
        }
    }

    /// Produce a typed fail-closed result when a surface has no observed Core
    /// scope. This does not inspect the current owner scope and performs no
    /// command work.
    pub fn reject_missing_scope(&self, intent: CommandIntent) -> CoreCommandOutcome {
        let request_id = self.allocate_request_id();
        let failure = Failure::new(
            ErrorCode::InvalidState,
            "requesting surface has not observed an active core scope",
            false,
        );
        self.push_event(CoreEvent::CommandFailed {
            scope: None,
            request_id,
            kind: intent.kind(),
            failure: failure.clone(),
        });
        CoreCommandOutcome {
            scope: None,
            result: CommandResult::Rejected {
                request_id,
                failure,
            },
        }
    }
}

#[cfg(test)]
pub(crate) fn test_request_at_current_scope(
    application: &CoreApplication,
    intent: CommandIntent,
) -> CoreCommandRequest {
    CoreCommandRequest {
        scope: application
            .snapshot()
            .scope()
            .expect("Core application test fixture must expose its owner identity"),
        intent,
    }
}

#[cfg(test)]
pub(crate) async fn test_execute_at_current_scope(
    application: &CoreApplication,
    intent: CommandIntent,
) -> CoreCommandOutcome {
    application
        .execute_scoped(test_request_at_current_scope(application, intent))
        .await
}

#[cfg(test)]
pub(crate) async fn test_execute_result_at_current_scope(
    application: &CoreApplication,
    intent: CommandIntent,
) -> CommandResult {
    test_execute_at_current_scope(application, intent)
        .await
        .result
}

#[cfg(test)]
pub(crate) fn test_dispatch_at_current_scope(
    application: &CoreApplication,
    intent: CommandIntent,
) -> (RequestId, Receiver<CoreCommandOutcome>) {
    application.dispatch_scoped_tracked(test_request_at_current_scope(application, intent))
}

/// Inbound command client bound to one immutable scope observed by a UI
/// surface. A new client is created when that surface accepts a newer scope.
#[derive(Clone)]
pub struct ScopedCoreCommandClient {
    application: CoreApplication,
    observed_scope: CoreScope,
}

impl ScopedCoreCommandClient {
    pub fn new(application: CoreApplication, observed_scope: CoreScope) -> Self {
        Self {
            application,
            observed_scope,
        }
    }

    pub fn observed_scope(&self) -> &CoreScope {
        &self.observed_scope
    }

    pub async fn execute(&self, intent: CommandIntent) -> CoreCommandOutcome {
        let outcome = self
            .application
            .execute_scoped(CoreCommandRequest {
                scope: self.observed_scope.clone(),
                intent: intent.clone(),
            })
            .await;
        validate_terminal_scope(&self.observed_scope, &intent, outcome)
    }

    pub fn dispatch_tracked(
        &self,
        intent: CommandIntent,
    ) -> (RequestId, Receiver<CoreCommandOutcome>) {
        self.application
            .dispatch_scoped_tracked(CoreCommandRequest {
                scope: self.observed_scope.clone(),
                intent,
            })
    }

    /// Read only while the owner's current scope still equals the scope this
    /// surface observed. Lifecycle transitions that advance generation must
    /// bind a new client from their terminal outcome or accepted snapshot.
    pub fn snapshot(&self) -> Result<CoreSnapshot, Failure> {
        let snapshot = self.application.snapshot();
        if snapshot.scope().as_ref() != Some(&self.observed_scope) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "observed core scope is stale; refresh the surface snapshot",
                false,
            ));
        }
        Ok(snapshot)
    }
}

#[async_trait::async_trait]
impl CoreLifecyclePort for CoreApplication {
    fn core_identity(&self) -> Option<CoreIdentity> {
        self.current_core_identity()
    }

    fn lifecycle(&self) -> CoreLifecycle {
        self.snapshot().lifecycle
    }

    fn generation(&self) -> u64 {
        CoreApplication::generation(self)
    }

    fn session_token(&self) -> Option<SessionToken> {
        CoreApplication::session_token(self)
    }

    fn lifecycle_snapshot(&self) -> CoreLifecycleSnapshot {
        self.snapshot().lifecycle_snapshot()
    }

    async fn start(&self) -> Result<u64, PortError> {
        match self
            .execute_lifecycle_with_id(self.allocate_request_id(), CommandIntent::StartCore)
            .await
        {
            CommandResult::Completed { .. } => Ok(self.generation()),
            CommandResult::Rejected { failure, .. } => Err(PortError::Failed(failure.message)),
            CommandResult::Produced { .. } => Err(PortError::Failed(
                "Unexpected typed result for a lifecycle/control command".into(),
            )),
            CommandResult::Accepted { .. } => Err(PortError::Failed(
                "application start unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    async fn stop(&self) -> Result<(), PortError> {
        match self
            .execute_lifecycle_with_id(self.allocate_request_id(), CommandIntent::StopCore)
            .await
        {
            CommandResult::Completed { .. } => Ok(()),
            CommandResult::Rejected { failure, .. } => Err(PortError::Failed(failure.message)),
            CommandResult::Produced { .. } => Err(PortError::Failed(
                "Unexpected typed result for a lifecycle/control command".into(),
            )),
            CommandResult::Accepted { .. } => Err(PortError::Failed(
                "application stop unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    async fn restart(&self) -> Result<u64, PortError> {
        match self
            .execute_lifecycle_with_id(self.allocate_request_id(), CommandIntent::RestartCore)
            .await
        {
            CommandResult::Completed { .. } => Ok(self.generation()),
            CommandResult::Rejected { failure, .. } => Err(PortError::Failed(failure.message)),
            CommandResult::Produced { .. } => Err(PortError::Failed(
                "Unexpected typed result for a lifecycle/control command".into(),
            )),
            CommandResult::Accepted { .. } => Err(PortError::Failed(
                "application restart unexpectedly returned Accepted".to_string(),
            )),
        }
    }

    fn begin_reload(&self) -> Result<SessionToken, PortError> {
        let CoreState::Running { session_token, .. } = self.current_state() else {
            return Err(PortError::Failed(
                "core is not running for hot reload".to_string(),
            ));
        };
        self.apply_domain_event(core_state::CoreEvent::ReloadRequested { session_token });
        Ok(session_token)
    }

    fn complete_reload(&self, session_token: SessionToken) -> Result<(), PortError> {
        self.check_session(session_token)
            .map_err(|failure| PortError::Failed(failure.message))?;
        self.apply_domain_event(core_state::CoreEvent::ReloadSuccess { session_token });
        Ok(())
    }

    fn fail_reload(&self, session_token: SessionToken, error: String) -> Result<(), PortError> {
        self.check_session(session_token)
            .map_err(|failure| PortError::Failed(failure.message))?;
        self.apply_domain_event(core_state::CoreEvent::ReloadFailed {
            session_token,
            error,
        });
        Ok(())
    }

    async fn wait_for_ready(
        &self,
        generation: u64,
        timeout: time::Duration,
    ) -> Result<(), PortError> {
        if self.generation() != generation {
            return Err(PortError::Failed(
                "stale application generation".to_string(),
            ));
        }
        let Some(session_token) = self.session_token() else {
            return Err(PortError::Failed("core session is not active".to_string()));
        };
        self.wait_for_readiness(session_token, timeout)
            .await
            .map(|_| ())
    }

    async fn wait_for_ready_session(
        &self,
        generation: u64,
        session_token: SessionToken,
        timeout: time::Duration,
    ) -> Result<(), PortError> {
        if self.generation() != generation {
            return Err(PortError::Failed(
                "stale application generation".to_string(),
            ));
        }
        self.check_session(session_token)
            .map_err(|failure| PortError::Failed(failure.message))?;
        self.wait_for_readiness(session_token, timeout)
            .await
            .map(|_| ())
    }
}

#[async_trait::async_trait]
impl CoreOwnerTerminationPort for CoreApplication {
    fn active_profile_reference_write_authorization(
        &self,
    ) -> Result<ActiveProfileReferenceWriteAuthorization, PortError> {
        CoreApplication::active_profile_reference_write_authorization(self)
            .map_err(PortError::Rejected)
    }

    async fn terminate_owner(&self) -> Result<CoreLifecycleSnapshot, PortError> {
        self.close().await.map_err(PortError::Rejected)?;
        Ok(self.snapshot().lifecycle_snapshot())
    }
}

#[path = "core_telemetry.rs"]
mod telemetry;
#[cfg(test)]
#[path = "core_application_tests.rs"]
pub(crate) mod tests;
