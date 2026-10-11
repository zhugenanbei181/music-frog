//! Kernel boot retry with external-controller port rotation (ledger:
//! "启动引导 3 次重试，失败轮换 external-controller 端口；就绪等待 15s；
//! rebuild 等端口释放 5s").
//!
//! Offline runtime materialization is a single shot: once it spawns the core it
//! never returns a handle on failure, and the application deliberately has no
//! `Drop`-stop — a readiness timeout would leave an unmanaged mihomo holding
//! the controller port and poisoning every later attempt. So the retry loop
//! cannot just call `bootstrap` repeatedly.
//!
//! Instead every *attempt* is a self-orchestrated probe that mirrors
//! `runtime.rs` line by line using public APIs only (ConfigManager ensure_*,
//! ServiceManager, CoreApplication, readiness) and **owns its process**: on
//! failure the session is stopped explicitly under a timeout. Once an attempt
//! proves the core startable and ready, `materialize` runs the real
//! the runtime materializer, which attaches to the now-running core
//! (fast path) and produces the full-fidelity runtime. Local config and asset
//! checks stay authoritative; remote enrichment is post-start only.
//!
//! Deliberate policy decisions:
//! - Readiness/start/resolve-binary failures are retryable; profile
//!   (config-content) failures are not — rotating a port cannot fix YAML.
//! - `materialize` failure is fatal: the core is healthy, killing it to retry
//!   a local attach would only destroy a good process.
//! - Proxy ports (mixed-port etc.) are never rotated: a silent change would
//!   repoint the user's system proxy. Only `external-controller` rotates,
//!   via the existing [`mihomo_config`] rotation (it picks its own port, so
//!   the injected picker is used solely to wait for port release).

use crate::composition::{core_application, desktop_backend, materialize_boot_host};
use crate::service::ServiceManager;
use crate::storage::doctor;
use crate::storage::{home_dir, settings_store, subscription_source, version_factory};
use crate::surface::SurfaceEngines;
use crate::system_dns_cache::DesktopSystemDnsCache;
use anyhow::anyhow;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::dns_cache_application::DnsCacheApplication;
use infiltrator_application::dns_latency_application::DnsLatencyApplication;
use infiltrator_application::dns_leak_application::{DnsLeakApplication, default_echo_sources};
use infiltrator_application::dns_query_application::DnsQueryApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::rule_list_application::RuleListApplication;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::snapshot_application::SnapshotApplication;
use infiltrator_application::speedtest_application::SpeedtestApplication;
use infiltrator_application::stun_probe_application::StunProbeApplication;
use infiltrator_contract::stun_probe::DEFAULT_STUN_SERVER;
use infiltrator_core::dns_latency_io::HttpDnsLatencyProber;
use infiltrator_core::dns_leak_io::HttpDnsLeakEchoProbe;
use infiltrator_core::settings_io::app_config_manager;
use infiltrator_core::snapshot_io::FileSnapshotStore;
use infiltrator_core::stun_io::UdpStunProbe;
use infiltrator_core::version_port::MihomoVersionPort;
use infiltrator_ports::core_binary_selection::{
    BundledBinarySource, CoreBinarySelection, CoreBinarySelectionPolicy, CoreBinarySelectionPort,
    CoreBinarySelectionRequest,
};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::host_runtime::HostRuntime;
use mihomo_api::controller_fake_ip_cache::MihomoControllerFakeIpCacheAdapter;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::port::is_port_available;
use mihomo_version::bundled_kernel::packaged_candidates;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{error, fmt};
use tokio::time::{Instant, sleep, timeout};

/// Ledger default: three boot attempts (initial try plus two retries).
pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;

/// Pause between attempts so a just-killed core can finish dying.
pub const RETRY_DELAY: Duration = Duration::from_millis(100);

/// Ceiling for waiting until the previous controller port is released.
pub const PORT_RELEASE_TIMEOUT: Duration = Duration::from_secs(5);

/// Desktop host policy that maps to the engine-neutral binary selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopBinaryPreference {
    ManagedWithBundledFallback,
    PreferBundled,
    RequireBundled,
}

/// Poll interval while waiting for port release.
const PORT_RELEASE_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Ceiling for the explicit stop between attempts; the attempt must not hang
/// forever on a wedged process.
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// One failed (or skipped) boot attempt, for UI/telemetry consumption.
#[derive(Debug, Clone)]
pub struct BootAttempt {
    /// Controller URL in play during the attempt, once known.
    pub controller_url: Option<String>,
    /// Human-readable failure reason.
    pub error: String,
}

/// Successful boot: the runtime plus a trace of what it took to get there.
pub struct BootOutcome {
    pub runtime: Arc<dyn HostRuntime>,
    /// Attempts that failed before the successful one, in order.
    pub attempts: Vec<BootAttempt>,
    /// True when at least one controller-port rotation succeeded.
    pub rotated: bool,
}

/// Aggregated boot failure. `tried` lists every distinct controller port an
/// attempt actually ran with; `source` is the last attempt's error. Callers
/// can `downcast_ref::<BootError>()` for structured reporting/i18n.
#[derive(Debug)]
pub struct BootError {
    pub tried: Vec<u16>,
    pub source: anyhow::Error,
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.tried.is_empty() {
            write!(f, "mihomo bootstrap failed: {}", self.source)
        } else {
            let ports = self
                .tried
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            write!(
                f,
                "mihomo bootstrap failed after {} attempt(s), controller ports tried: [{}]: {}",
                self.tried.len(),
                ports,
                self.source
            )
        }
    }
}

impl error::Error for BootError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&*self.source)
    }
}

/// Tunables for [`bootstrap_with_retry_opts`]. The picker contract:
/// `picker(port) == Some(port)` iff `port` is bindable right now. The
/// production picker is a plain availability probe; the default
/// `find_available_port` from `mihomo_config` satisfies it too.
pub struct BootRetryOptions {
    pub max_attempts: u32,
    pub retry_delay: Duration,
    pub port_release_timeout: Duration,
    pub port_picker: Arc<dyn Fn(u16) -> Option<u16> + Send + Sync>,
}

impl Default for BootRetryOptions {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            retry_delay: RETRY_DELAY,
            port_release_timeout: PORT_RELEASE_TIMEOUT,
            port_picker: Arc::new(production_port_picker),
        }
    }
}

/// Retry-guided boot with ledger defaults (3 attempts, 100ms pause, 5s port
/// release wait). See [`bootstrap_with_retry_opts`] for the customizable form.
pub async fn bootstrap_with_retry(
    policy: CoreBinarySelectionPolicy,
) -> anyhow::Result<BootOutcome> {
    bootstrap_with_retry_opts(policy, BootRetryOptions::default()).await
}

/// Boot the desktop host and return only the inbound-safe runtime port.
pub async fn bootstrap_host_runtime(
    policy: CoreBinarySelectionPolicy,
) -> anyhow::Result<(Arc<dyn HostRuntime>, bool)> {
    let outcome = bootstrap_with_retry(policy).await?;
    Ok((outcome.runtime, outcome.rotated))
}

/// Desktop composition entry point used by UI/admin callers. Version
/// manager construction and home resolution stay inside the desktop host;
/// callers only choose the bundled-core policy and optional packaged paths.
pub async fn bootstrap_host_runtime_from_current_home(
    preference: DesktopBinaryPreference,
    bundled_candidates: &[PathBuf],
) -> anyhow::Result<(Arc<dyn HostRuntime>, bool)> {
    let data_dir = home_dir()?;
    let mut candidates = bundled_candidates.to_vec();
    candidates.extend(packaged_candidates()?);
    let bundled = BundledBinarySource {
        candidates,
        data_directory: data_dir,
    };
    let policy = match preference {
        DesktopBinaryPreference::ManagedWithBundledFallback => {
            CoreBinarySelectionPolicy::ManagedWithBundledFallback(bundled)
        }
        DesktopBinaryPreference::PreferBundled => CoreBinarySelectionPolicy::PreferBundled(bundled),
        DesktopBinaryPreference::RequireBundled => {
            CoreBinarySelectionPolicy::RequireBundled(bundled)
        }
    };
    bootstrap_host_runtime(policy).await
}

/// [`bootstrap_with_retry`] with explicit retry behavior (tests, rebuild
/// flows that need different timings).
pub async fn bootstrap_with_retry_opts(
    policy: CoreBinarySelectionPolicy,
    options: BootRetryOptions,
) -> anyhow::Result<BootOutcome> {
    let engine_id = MihomoVersionPort::engine_identifier();
    let version_factory = version_factory()?;
    let binary_selection = version_factory.binary_selection_port(&engine_id)?;
    let engine = ProductionEngine {
        engine_id,
        binary_selection,
        selection_policy: policy,
        binary_state: Mutex::new(SelectedBinaryState::Awaiting),
    };
    let config = RetryLoopConfig {
        max_attempts: options.max_attempts,
        retry_delay: options.retry_delay,
        port_release_timeout: options.port_release_timeout,
    };
    let (runtime, attempts, rotated) =
        run_boot_retry(&engine, config, options.port_picker.as_ref()).await?;
    Ok(BootOutcome {
        runtime,
        attempts,
        rotated,
    })
}

/// One attempt's failure plus what the retry loop needs to act on it.
pub(crate) struct AttemptFailure {
    pub error: anyhow::Error,
    pub controller_url: Option<String>,
    pub controller_port: Option<u16>,
    /// False failures abort the whole boot immediately.
    pub retryable: bool,
}

impl AttemptFailure {
    fn new(error: anyhow::Error, controller_url: Option<&str>, retryable: bool) -> Self {
        let controller_url = controller_url.map(str::to_string);
        let controller_port = controller_url.as_deref().and_then(controller_port_from_url);
        Self {
            error,
            controller_url,
            controller_port,
            retryable,
        }
    }
}

/// The pluggable boot seam: production implementation wraps the real
/// lifecycle, tests substitute a scripted one.
pub(crate) trait BootEngine {
    type Runtime;

    /// Run one full boot attempt. `Ok` means a core process is alive and its
    /// controller proved ready; the engine has already cleaned up on `Err`.
    async fn attempt(&self) -> Result<(), AttemptFailure>;

    /// Rotate the external-controller port in the active profile; returns
    /// the new port.
    async fn rotate_controller_port(&self) -> anyhow::Result<u16>;

    /// Materialize the final runtime on top of a proven-ready core.
    async fn materialize(&self) -> anyhow::Result<Self::Runtime>;
}

pub(crate) struct RetryLoopConfig {
    pub max_attempts: u32,
    pub retry_delay: Duration,
    pub port_release_timeout: Duration,
}

/// Drive attempts with rotation between failures. Returns the runtime, the
/// failed-attempt trace and whether any rotation succeeded.
pub(crate) async fn run_boot_retry<E: BootEngine>(
    engine: &E,
    config: RetryLoopConfig,
    port_picker: &(dyn Fn(u16) -> Option<u16> + Send + Sync),
) -> anyhow::Result<(E::Runtime, Vec<BootAttempt>, bool)> {
    let max_attempts = config.max_attempts.max(1);
    let mut attempts = Vec::new();
    let mut tried_ports: Vec<u16> = Vec::new();
    let mut rotated = false;
    let mut last_error: Option<anyhow::Error> = None;

    for round in 1..=max_attempts {
        log::info!("mihomo boot attempt {round}/{max_attempts}");
        match engine.attempt().await {
            Ok(()) => {
                // Core proven startable and ready. Materialize failure is
                // fatal on purpose: a retry would kill a healthy core to
                // repeat a problem no port rotation can fix.
                let runtime = engine.materialize().await.map_err(|error| {
                    anyhow::Error::new(BootError {
                        tried: tried_ports.clone(),
                        source: error,
                    })
                })?;
                return Ok((runtime, attempts, rotated));
            }
            Err(failure) => {
                log::warn!(
                    "boot attempt {round}/{max_attempts} failed: {:#}",
                    failure.error
                );
                if let Some(port) = failure.controller_port
                    && !tried_ports.contains(&port)
                {
                    tried_ports.push(port);
                }
                attempts.push(BootAttempt {
                    controller_url: failure.controller_url.clone(),
                    error: failure.error.to_string(),
                });
                if !failure.retryable {
                    return Err(anyhow::Error::new(BootError {
                        tried: tried_ports,
                        source: failure.error,
                    }));
                }
                last_error = Some(failure.error);

                if round < max_attempts {
                    match engine.rotate_controller_port().await {
                        Ok(new_port) => {
                            rotated = true;
                            log::info!("rotated external-controller port to {new_port}");
                        }
                        Err(error) => {
                            log::warn!(
                                "controller port rotation failed, retrying on current port: {error:#}"
                            );
                        }
                    }
                    if let Some(port) = failure.controller_port {
                        wait_for_port_release(port, config.port_release_timeout, port_picker).await;
                    }
                    sleep(config.retry_delay).await;
                }
            }
        }
    }

    Err(anyhow::Error::new(BootError {
        tried: tried_ports,
        source: last_error.unwrap_or_else(|| anyhow!("bootstrap failed for an unknown reason")),
    }))
}

/// Poll until `picker(port)` reports the port bindable again, bounded by
/// `timeout` (ledger: "等端口释放 5s"). Best effort: a timeout is logged, not
/// fatal — rotation already picked a free port regardless.
async fn wait_for_port_release(
    port: u16,
    timeout: Duration,
    picker: &(dyn Fn(u16) -> Option<u16> + Send + Sync),
) {
    let deadline = Instant::now() + timeout;
    while picker(port) != Some(port) {
        if Instant::now() >= deadline {
            log::warn!("controller port {port} not released within {timeout:?}; continuing");
            return;
        }
        sleep(PORT_RELEASE_POLL_INTERVAL).await;
    }
}

/// Extract the port from a normalized controller URL
/// (`http://127.0.0.1:9090`, `127.0.0.1:9090`, `[::1]:9090`, ...).
fn controller_port_from_url(url: &str) -> Option<u16> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host_port = rest.split(['/', '?']).next()?;
    host_port
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse().ok())
}

/// Production picker: `Some(port)` iff the port is bindable on localhost now.
fn production_port_picker(port: u16) -> Option<u16> {
    is_port_available(port).then_some(port)
}

/// Production engine over the real lifecycle (pid-file process control makes
/// cross-attempt stop/attach valid).
enum SelectedBinaryState {
    Awaiting,
    Selected(CoreBinarySelection),
}

struct ProductionEngine {
    engine_id: infiltrator_contract::core_identity::EngineId,
    binary_selection: Arc<dyn CoreBinarySelectionPort>,
    selection_policy: CoreBinarySelectionPolicy,
    binary_state: Mutex<SelectedBinaryState>,
}

impl BootEngine for ProductionEngine {
    type Runtime = Arc<dyn HostRuntime>;

    async fn attempt(&self) -> Result<(), AttemptFailure> {
        {
            let mut state = self.binary_state.lock().map_err(|_| {
                AttemptFailure::new(
                    anyhow!("Mihomo binary selection state lock is poisoned"),
                    None,
                    false,
                )
            })?;
            *state = SelectedBinaryState::Awaiting;
        }
        // Config preparation. Fatal on failure: YAML-level breakage is not
        // something a port rotation can fix (mirrors runtime.rs:60-66).
        let cm = Arc::new(app_config_manager().await.map_err(|error| {
            AttemptFailure::new(anyhow!("load config manager: {error:#}"), None, false)
        })?);
        cm.ensure_default_config().await.map_err(|error| {
            AttemptFailure::new(anyhow!("prepare default profile: {error}"), None, false)
        })?;
        cm.validate_current_profile().await.map_err(|error| {
            AttemptFailure::new(anyhow!("validate local profile: {error}"), None, false)
        })?;
        cm.ensure_proxy_ports().await.map_err(|error| {
            AttemptFailure::new(anyhow!("prepare proxy ports: {error}"), None, false)
        })?;
        let controller_url = cm.ensure_external_controller().await.map_err(|error| {
            AttemptFailure::new(anyhow!("prepare external controller: {error}"), None, false)
        })?;
        let config_path = cm.get_current_path().await.map_err(|error| {
            AttemptFailure::new(
                anyhow!("resolve current profile path: {error}"),
                None,
                false,
            )
        })?;
        let controller = Some(controller_url.as_str());

        // Retryable: binary source/readiness failures can recover on a later
        // attempt. Selection and its version evidence come from the engine's
        // version-scoped adapter factory.
        let selection = self
            .binary_selection
            .select(CoreBinarySelectionRequest {
                engine_id: self.engine_id.clone(),
                policy: self.selection_policy.clone(),
            })
            .await
            .map_err(|error| {
                AttemptFailure::new(
                    anyhow!("select Mihomo core binary: {error}"),
                    controller,
                    true,
                )
            })?;
        let binary = selection.binary_path.clone();
        {
            let mut state = self.binary_state.lock().map_err(|_| {
                AttemptFailure::new(
                    anyhow!("Mihomo binary selection state lock is poisoned"),
                    controller,
                    false,
                )
            })?;
            *state = SelectedBinaryState::Selected(selection);
        }

        // Mirror of runtime.rs:70-77: service + application with a lazily
        // resolving endpoint source, so a rotated port is picked up by every
        // readiness probe without being passed around.
        let service_manager = ServiceManager::new(binary, config_path);
        let endpoints = Arc::new(ProfileEndpointSource::new(cm.clone()));
        let endpoint = endpoints.resolve().await.map_err(|error| {
            AttemptFailure::new(
                anyhow!("resolve controller endpoint: {error}"),
                controller,
                false,
            )
        })?;
        let backend = desktop_backend(
            &service_manager,
            &endpoint.url,
            endpoint.secret.clone(),
            &MihomoVersionPort::engine_identifier(),
        )
        .map_err(|error| {
            AttemptFailure::new(
                anyhow!("create engine backend ports: {error}"),
                controller,
                false,
            )
        })?;
        let client = backend.client.clone();
        let apply_guard = Arc::new(tokio::sync::Mutex::new(()));
        let profiles =
            ProfileApplication::new(cm.clone()).with_document_codec(backend.ports.configuration());
        let configuration = ConfigurationApplication::from_profile_application(profiles.clone());
        let application = Arc::new(
            core_application(
                &service_manager,
                backend,
                endpoint.secret.clone(),
                SurfaceEngines {
                    profiles,
                    configuration,
                    snapshots: SnapshotApplication::new(
                        cm.clone(),
                        Arc::new(FileSnapshotStore::new(cm.config_dir().to_path_buf())),
                    ),
                    scripts: ScriptApplication::new(),
                    script_exports: ScriptExportApplication::without_host_port(),
                    doctor: DoctorApplication::new(Arc::new(doctor().map_err(|error| {
                        AttemptFailure::new(
                            anyhow!("construct diagnostic host: {error}"),
                            controller,
                            false,
                        )
                    })?)),
                    rule_tracer: RuleTracerApplication::new(),
                    rule_list: RuleListApplication::default(),
                    dns_query: DnsQueryApplication::new(Some(Arc::new(client.clone()))),
                    dns_cache: DnsCacheApplication::new(
                        Some(Arc::new(MihomoControllerFakeIpCacheAdapter::new(
                            client.clone(),
                        ))),
                        Some(Arc::new(DesktopSystemDnsCache::new())),
                    ),
                    // DUAL-14-10: the boot path composes the same real prober the
                    // runtime does, so a surface started from the retry ladder
                    // measures instead of reporting a placeholder.
                    dns_latency: DnsLatencyApplication::new(Some(Arc::new(
                        HttpDnsLatencyProber::new(),
                    ))),
                    // DUAL-14-08: mirror of runtime.rs: the boot path injects the
                    // same real echo adapter and the same real default TXT echo
                    // authorities, so a surface started from the retry ladder
                    // cross-checks instead of reporting a placeholder.
                    dns_leak: DnsLeakApplication::new(
                        Some(Arc::new(HttpDnsLeakEchoProbe::new())),
                        default_echo_sources(),
                    ),
                    // DUAL-14-09 (re-scoped): mirror of runtime.rs: the boot path
                    // injects the same real UDP STUN adapter and public default
                    // server, so a surface started from the retry ladder probes
                    // this host's UDP egress instead of reporting a placeholder.
                    stun_probe: StunProbeApplication::new(
                        Some(Arc::new(UdpStunProbe::new())),
                        DEFAULT_STUN_SERVER,
                    ),
                },
                SpeedtestApplication::new(SettingsApplication::new(
                    settings_store().await.map_err(|error| {
                        AttemptFailure::new(
                            anyhow!("construct probe settings store: {error}"),
                            controller,
                            false,
                        )
                    })?,
                )),
                cm.clone(),
                Arc::new(subscription_source()),
                cm.clone(),
                endpoints.clone() as Arc<dyn EndpointSource>,
                apply_guard,
            )
            .map_err(|error| {
                AttemptFailure::new(
                    anyhow!("build core application: {error}"),
                    controller,
                    false,
                )
            })?,
        );

        // Mirror of runtime.rs:82-95: attach to a running instance by proving
        // it answers, or start fresh and let application own readiness (15s).
        let readiness: Result<(), AttemptFailure> = if service_manager.is_running().await {
            log::info!("boot: attaching to running mihomo service");
            application
                .adopt_if_running()
                .await
                .map(|_| ())
                .map_err(|error| {
                    AttemptFailure::new(
                        anyhow!("running instance not ready: {}", error.message),
                        controller,
                        true,
                    )
                })
        } else {
            log::info!("boot: starting mihomo service");
            CoreLifecyclePort::start(application.as_ref())
                .await
                .map(|_| ())
                .map_err(|error| {
                    AttemptFailure::new(anyhow!("mihomo core not ready: {error}"), controller, true)
                })
        };
        if let Err(failure) = readiness {
            // Application has no Drop-stop: without this the core keeps the
            // controller port and every later attempt attaches to the same
            // zombie. Bounded stop; a wedged process is logged and left for
            // the next attempt (whose ensure_external_controller will dodge
            // its port).
            stop_application_quietly(application.as_ref()).await;
            return Err(failure);
        }

        // The core stays up (it is ready) for materialize to attach.
        Ok(())
    }

    async fn rotate_controller_port(&self) -> anyhow::Result<u16> {
        let cm = app_config_manager().await?;
        let url = cm
            .rotate_external_controller()
            .await
            .map_err(|error| anyhow!("{error}"))?;
        controller_port_from_url(&url)
            .ok_or_else(|| anyhow!("rotated controller URL has no port: {url}"))
    }

    async fn materialize(&self) -> anyhow::Result<Arc<dyn HostRuntime>> {
        let selection = {
            let state = self
                .binary_state
                .lock()
                .map_err(|_| anyhow!("Mihomo binary selection state lock is poisoned"))?;
            match &*state {
                SelectedBinaryState::Awaiting => {
                    anyhow::bail!("Mihomo runtime cannot materialize before binary selection")
                }
                SelectedBinaryState::Selected(selection) => selection.clone(),
            }
        };
        materialize_boot_host(selection).await
    }
}

async fn stop_application_quietly(application: &CoreApplication) {
    match timeout(STOP_TIMEOUT, CoreLifecyclePort::stop(application)).await {
        Ok(Ok(())) => log::info!("stopped mihomo core after failed boot attempt"),
        Ok(Err(error)) => {
            log::warn!("failed to stop mihomo core after failed attempt: {error}")
        }
        Err(_) => log::warn!(
            "timed out stopping mihomo core after failed attempt; it may linger until the next attempt"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const TEST_PORT: u16 = 9090;

    enum ScriptedAttempt {
        Ok,
        Fail {
            retryable: bool,
            port: Option<u16>,
            message: &'static str,
        },
    }

    struct MockEngine {
        script: Mutex<VecDeque<ScriptedAttempt>>,
        attempts_run: AtomicUsize,
        materialized: AtomicUsize,
        rotations: AtomicUsize,
        rotate_error: Option<&'static str>,
        materialize_error: Option<&'static str>,
    }

    impl MockEngine {
        fn new(script: Vec<ScriptedAttempt>) -> Self {
            Self {
                script: Mutex::new(script.into_iter().collect()),
                attempts_run: AtomicUsize::new(0),
                materialized: AtomicUsize::new(0),
                rotations: AtomicUsize::new(0),
                rotate_error: None,
                materialize_error: None,
            }
        }

        fn with_rotate_error(mut self, error: &'static str) -> Self {
            self.rotate_error = Some(error);
            self
        }

        fn with_materialize_error(mut self, error: &'static str) -> Self {
            self.materialize_error = Some(error);
            self
        }

        fn ok() -> Self {
            Self::new(vec![ScriptedAttempt::Ok])
        }
    }

    impl BootEngine for MockEngine {
        type Runtime = usize;

        async fn attempt(&self) -> Result<(), AttemptFailure> {
            self.attempts_run.fetch_add(1, Ordering::SeqCst);
            let step = self.script.lock().expect("script lock").pop_front();
            match step {
                None | Some(ScriptedAttempt::Ok) => Ok(()),
                Some(ScriptedAttempt::Fail {
                    retryable,
                    port,
                    message,
                }) => {
                    let url = port.map(|p| format!("http://127.0.0.1:{p}"));
                    Err(AttemptFailure::new(
                        anyhow!("{message}"),
                        url.as_deref(),
                        retryable,
                    ))
                }
            }
        }

        async fn rotate_controller_port(&self) -> anyhow::Result<u16> {
            let round = self.rotations.fetch_add(1, Ordering::SeqCst);
            match self.rotate_error {
                Some(error) => Err(anyhow!("{error}")),
                None => Ok(TEST_PORT + 1 + round as u16),
            }
        }

        async fn materialize(&self) -> anyhow::Result<usize> {
            match self.materialize_error {
                Some(error) => Err(anyhow!("{error}")),
                None => Ok(self.materialized.fetch_add(1, Ordering::SeqCst) + 1),
            }
        }
    }

    fn test_config(max_attempts: u32) -> RetryLoopConfig {
        RetryLoopConfig {
            max_attempts,
            retry_delay: Duration::from_millis(1),
            port_release_timeout: Duration::from_millis(10),
        }
    }

    fn always_free(port: u16) -> Option<u16> {
        Some(port)
    }

    async fn run(
        engine: &MockEngine,
        max_attempts: u32,
    ) -> anyhow::Result<(usize, Vec<BootAttempt>, bool)> {
        run_boot_retry(engine, test_config(max_attempts), &always_free).await
    }

    #[tokio::test]
    async fn retry_succeeds_after_rotation() {
        let engine = MockEngine::new(vec![
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(TEST_PORT),
                message: "controller not ready within 15.0s",
            },
            ScriptedAttempt::Ok,
        ]);

        let (runtime, attempts, rotated) = run(&engine, 3).await.expect("second attempt boots");

        assert_eq!(runtime, 1);
        assert_eq!(attempts.len(), 1);
        assert_eq!(
            attempts[0].controller_url.as_deref(),
            Some("http://127.0.0.1:9090")
        );
        assert!(attempts[0].error.contains("not ready"));
        assert!(rotated);
        assert_eq!(engine.rotations.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn all_attempts_fail_aggregates_boot_error() {
        let engine = MockEngine::new(vec![
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(9091),
                message: "timeout a",
            },
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(9092),
                message: "timeout b",
            },
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(9093),
                message: "timeout c",
            },
        ]);

        let error = run(&engine, 3).await.expect_err("all attempts fail");

        let boot_error = error.downcast_ref::<BootError>().expect("BootError");
        assert_eq!(boot_error.tried, vec![9091, 9092, 9093]);
        assert!(boot_error.source.to_string().contains("timeout c"));
        assert_eq!(engine.attempts_run.load(Ordering::SeqCst), 3);
        assert_eq!(engine.rotations.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn fatal_failure_stops_immediately() {
        let engine = MockEngine::new(vec![ScriptedAttempt::Fail {
            retryable: false,
            port: Some(TEST_PORT),
            message: "invalid profile YAML",
        }]);

        let error = run(&engine, 3).await.expect_err("fatal aborts");

        let boot_error = error.downcast_ref::<BootError>().expect("BootError");
        assert_eq!(boot_error.tried, vec![TEST_PORT]);
        assert_eq!(engine.attempts_run.load(Ordering::SeqCst), 1);
        assert_eq!(engine.rotations.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn rotation_failure_still_retries_without_rotated_flag() {
        let engine = MockEngine::new(vec![
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(TEST_PORT),
                message: "controller not ready",
            },
            ScriptedAttempt::Ok,
        ])
        .with_rotate_error("no available ports found");

        let (runtime, attempts, rotated) = run(&engine, 2)
            .await
            .expect("retry continues past rotation failure");

        assert_eq!(runtime, 1);
        assert_eq!(attempts.len(), 1);
        assert!(!rotated);
        assert_eq!(engine.rotations.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn materialize_failure_is_fatal_and_keeps_tried_ports() {
        // First attempt fails (retryable), second proves ready, materialize
        // blows up: the loop must surface the local validation error, not retry.
        let engine = MockEngine::new(vec![
            ScriptedAttempt::Fail {
                retryable: true,
                port: Some(TEST_PORT),
                message: "controller not ready",
            },
            ScriptedAttempt::Ok,
        ])
        .with_materialize_error("local startup validation failed");

        let error = run(&engine, 3).await.expect_err("materialize fatal");
        let boot_error = error.downcast_ref::<BootError>().expect("BootError");
        assert_eq!(boot_error.tried, vec![TEST_PORT]);
        assert!(boot_error.source.to_string().contains("local startup"));
        assert_eq!(engine.attempts_run.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn zero_attempts_is_clamped_to_one() {
        let engine = MockEngine::ok();
        let (runtime, attempts, rotated) = run(&engine, 0).await.expect("clamped boot");
        assert_eq!(runtime, 1);
        assert!(attempts.is_empty());
        assert!(!rotated);
    }

    #[tokio::test]
    async fn port_release_wait_returns_once_picker_reports_free() {
        let polls = Arc::new(AtomicUsize::new(0));
        let observed = polls.clone();
        let picker = move |port: u16| -> Option<u16> {
            let seen = observed.fetch_add(1, Ordering::SeqCst);
            if seen < 2 { None } else { Some(port) }
        };

        let started = Instant::now();
        wait_for_port_release(TEST_PORT, Duration::from_secs(5), &picker).await;
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(polls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn port_release_wait_gives_up_after_timeout() {
        let picker = |_port: u16| -> Option<u16> { None };

        let started = Instant::now();
        wait_for_port_release(TEST_PORT, Duration::from_millis(50), &picker).await;
        assert!(started.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn production_picker_respects_real_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
        let port = listener.local_addr().expect("addr").port();

        assert_eq!(production_port_picker(port), None);

        drop(listener);
        assert_eq!(production_port_picker(port), Some(port));
    }

    #[test]
    fn controller_port_from_url_parses_normalized_urls() {
        assert_eq!(
            controller_port_from_url("http://127.0.0.1:9090"),
            Some(9090)
        );
        assert_eq!(
            controller_port_from_url("https://host.example:8080/"),
            Some(8080)
        );
        assert_eq!(controller_port_from_url("127.0.0.1:9091"), Some(9091));
        assert_eq!(
            controller_port_from_url("http://127.0.0.1:9090/extra"),
            Some(9090)
        );
        assert_eq!(controller_port_from_url("[::1]:9092"), Some(9092));
        assert_eq!(controller_port_from_url("http://localhost"), None);
        assert_eq!(controller_port_from_url("http://127.0.0.1:notaport"), None);
    }

    #[test]
    fn boot_error_display_and_source() {
        let error = BootError {
            tried: vec![9090, 9091],
            source: anyhow!("controller not ready within 15.0s"),
        };

        let display = error.to_string();
        assert!(display.contains("2 attempt(s)"));
        assert!(display.contains("9090, 9091"));
        assert!(display.contains("controller not ready within 15.0s"));

        let source = error::Error::source(&error).expect("source");
        assert_eq!(source.to_string(), "controller not ready within 15.0s");
    }

    #[test]
    fn boot_error_display_without_ports() {
        let error = BootError {
            tried: vec![],
            source: anyhow!("load config manager: no home"),
        };
        let display = error.to_string();
        assert!(display.starts_with("mihomo bootstrap failed: "));
        assert!(display.contains("no home"));
    }

    #[test]
    fn default_options_match_ledger() {
        let options = BootRetryOptions::default();
        assert_eq!(options.max_attempts, 3);
        assert_eq!(options.retry_delay, Duration::from_millis(100));
        assert_eq!(options.port_release_timeout, Duration::from_secs(5));
        assert_eq!((options.port_picker)(TEST_PORT), Some(TEST_PORT));
    }
}
