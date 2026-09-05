//! Concrete adapter wiring for native and cross-platform application hosts.
//!
//! The application crate owns orchestration and private runtime details. This
//! crate owns the decision that a particular product composition uses the
//! Mihomo REST adapter. UI crates may consume the returned application pump,
//! but do not construct `MihomoClient` themselves.

use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::overview::{OverviewConfig, OverviewPump, UnavailableOverviewReader};
use infiltrator_application::offline_startup_application::OfflineStartupApplication;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_ios::{IosBridge, IosHostAdapter};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::core_watchdog::CoreWatchdogPort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::OverviewReader;
use mihomo_api::client::MihomoClient;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_api::readiness::ControllerReadiness;
use std::sync::Arc;
use std::time::Duration;

/// The watchdog probes process liveness four times per second. This keeps a
/// dead core inside the three-second recovery budget without busy spinning.
pub const CORE_WATCHDOG_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Owns the host scheduler task that drives the application watchdog.
/// Dropping it aborts the task so a closed host cannot keep probing a core.
pub struct CoreWatchdogHandle {
    task: tokio::task::JoinHandle<()>,
}

impl Drop for CoreWatchdogHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Start the concrete scheduler at a composition root. The application still
/// owns the process probe, state machine, backoff and restart transaction;
/// Tokio appears only here as the host scheduler implementation.
pub fn spawn_core_watchdog(application: Arc<CoreApplication>) -> CoreWatchdogHandle {
    let task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(CORE_WATCHDOG_POLL_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = application.watchdog_tick().await {
                log::warn!("core watchdog poll failed: {error}");
            }
        }
    });
    CoreWatchdogHandle { task }
}

/// Tokio-backed implementation of the runtime capability required by the
/// application layer. Tokio is deliberately constructed here, at a
/// composition root, rather than in `infiltrator-application`.
pub struct TokioApplicationRuntime {
    runtime: tokio::runtime::Runtime,
}

impl TokioApplicationRuntime {
    pub fn new() -> Result<Self, String> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map(|runtime| Self { runtime })
            .map_err(|error| error.to_string())
    }
}

impl ApplicationRuntime for TokioApplicationRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        self.runtime.block_on(future);
    }

    fn sleep(&self, duration: std::time::Duration) -> ApplicationSleep<'_> {
        Box::pin(tokio::time::sleep(duration))
    }
}

pub fn tokio_application_runtime() -> Result<Arc<dyn ApplicationRuntime>, String> {
    Ok(Arc::new(TokioApplicationRuntime::new()?))
}

/// Build the standard Mihomo-backed Overview pump for a product composition.
pub fn spawn_mihomo_overview(config: OverviewConfig) -> OverviewPump {
    let runtime =
        tokio_application_runtime().expect("Tokio application runtime must be constructible");
    let reader: Arc<dyn OverviewReader> =
        match mihomo_api::client::MihomoClient::new(&config.endpoint, config.secret.clone()) {
            Ok(client) => Arc::new(mihomo_api::overview::ControllerOverviewReader::new(client)),
            Err(error) => Arc::new(UnavailableOverviewReader::new(PortError::Network(
                error.to_string(),
            ))),
        };
    OverviewPump::spawn(reader, config.sample_interval, runtime)
}

/// Assemble the shared application for an iOS host. The native bridge is the
/// only iOS-specific input; NetworkExtension and Swift lifecycle details stay
/// behind `IosBridge`.
pub fn ios_core_application<B>(
    bridge: B,
    controller_url: impl Into<String>,
    secret: Option<String>,
) -> Result<CoreApplication, String>
where
    B: IosBridge + 'static,
{
    let controller_url = controller_url.into();
    let client =
        MihomoClient::new(&controller_url, secret.clone()).map_err(|error| error.to_string())?;
    let runtime = tokio_application_runtime()?;
    let host = std::sync::Arc::new(IosHostAdapter::new(bridge));
    let application = CoreApplication::new_with_overview(
        host.clone(),
        std::sync::Arc::new(ControllerReadiness::new(
            controller_url.clone(),
            secret.clone(),
        )),
        std::sync::Arc::new(ControllerOverviewReader::new(client.clone())),
        runtime,
    );
    application.install_command_handler(std::sync::Arc::new(
        CommandApplication::new()
            .with_runtime(std::sync::Arc::new(client))
            .with_mtu(MtuApplication::new(host)),
    ));
    Ok(application)
}

/// Compose the iOS native host's local-only startup proof for either UI.
pub fn ios_offline_startup_application<B>(bridge: B) -> OfflineStartupApplication
where
    B: IosBridge + 'static,
{
    OfflineStartupApplication::new(Arc::new(IosHostAdapter::new(bridge)))
}

/// Compose the iOS native physical-link MTU observer for either UI surface.
pub fn ios_mtu_application<B>(bridge: B) -> MtuApplication
where
    B: IosBridge + 'static,
{
    MtuApplication::new(Arc::new(IosHostAdapter::new(bridge)))
}

/// Assemble the iOS application together with its host scheduler. Native
/// callers that want automatic crash recovery should retain the returned
/// handle for as long as the app/extension owns the core session.
pub fn ios_core_application_with_watchdog<B>(
    bridge: B,
    controller_url: impl Into<String>,
    secret: Option<String>,
) -> Result<(Arc<CoreApplication>, CoreWatchdogHandle), String>
where
    B: IosBridge + 'static,
{
    let application = Arc::new(ios_core_application(bridge, controller_url, secret)?);
    let watchdog = spawn_core_watchdog(application.clone());
    Ok((application, watchdog))
}
