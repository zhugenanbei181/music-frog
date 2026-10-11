//! Concrete adapter wiring for native and cross-platform application hosts.
//!
//! The application crate owns orchestration and private runtime details. This
//! crate owns engine provider composition, including the Mihomo REST adapter
//! and the opt-in sing-box backend provider. Product roots select and bind one
//! provider; UI crates do not construct engine clients or Core owners.

use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::core_capability_application::CoreCapabilityApplication;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::offline_startup_application::OfflineStartupApplication;
use infiltrator_application::overview::{OverviewConfig, OverviewPump, UnavailableOverviewReader};
use infiltrator_contract::core_capabilities::{
    ConnectionOperation, CoreLifecycleOperation, CoreOperation, GeoOperation, LogsOperation,
    OutboundOperation, ProviderOperation, ProxyControlOperation,
};
use infiltrator_contract::surface::HostKind;
use infiltrator_ios::{IosBridge, IosHostAdapter};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep, ApplicationTaskAdmission,
    ApplicationTaskId, ApplicationTaskShutdown,
};
use infiltrator_ports::core_watchdog::CoreWatchdogPort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::OverviewReader;
use mihomo_api::client::MihomoClient;
use mihomo_api::core_capability_provider::MihomoCoreCapabilityProvider;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_api::readiness::ControllerReadiness;
use mihomo_api::routing_intent::MihomoRoutingIntentAdapter;
use std::num::NonZeroU64;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval, sleep};

/// The watchdog probes process liveness four times per second. This keeps a
/// dead core inside the three-second recovery budget without busy spinning.
pub const CORE_WATCHDOG_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Owns the host scheduler task that drives the application watchdog.
/// Dropping it aborts the task so a closed host cannot keep probing a core.
pub struct CoreWatchdogHandle {
    task: JoinHandle<()>,
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
        let mut interval = interval(CORE_WATCHDOG_POLL_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
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
    runtime: Runtime,
    background: SyncSender<BackgroundTaskMessage>,
    background_state: Arc<(Mutex<BackgroundSchedulerState>, Condvar)>,
    next_task_id: AtomicU64,
}

enum BackgroundTaskMessage {
    Run {
        task_id: ApplicationTaskId,
        future: ApplicationFuture,
    },
    Shutdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackgroundSchedulerState {
    Open,
    Occupied { task_id: ApplicationTaskId },
    Closing,
    Closed,
    WorkerFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShutdownDisposition {
    Initiated,
    Awaiting,
    AlreadyClosed,
    WorkerFailed,
}

impl TokioApplicationRuntime {
    pub fn new() -> Result<Self, String> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let background_runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let (background, receiver) = sync_channel(1);
        let background_state =
            Arc::new((Mutex::new(BackgroundSchedulerState::Open), Condvar::new()));
        let worker_state = Arc::clone(&background_state);
        thread::Builder::new()
            .name("infiltrator-background-task".to_owned())
            .spawn(move || background_task_worker(background_runtime, receiver, worker_state))
            .map_err(|error| error.to_string())?;
        Ok(Self {
            runtime,
            background,
            background_state,
            next_task_id: AtomicU64::new(1),
        })
    }
}

impl ApplicationRuntime for TokioApplicationRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        self.runtime.block_on(future);
    }

    fn spawn_background(&self, future: ApplicationFuture) -> ApplicationTaskAdmission {
        let task_id =
            match self
                .next_task_id
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                    next.checked_add(1)
                }) {
                Ok(value) => match NonZeroU64::new(value) {
                    Some(value) => ApplicationTaskId::new(value),
                    None => return ApplicationTaskAdmission::IdExhausted,
                },
                Err(_) => return ApplicationTaskAdmission::IdExhausted,
            };
        let (state, changed) = &*self.background_state;
        let mut state = state.lock().expect("background scheduler state lock");
        match *state {
            BackgroundSchedulerState::Open => {
                *state = BackgroundSchedulerState::Occupied { task_id };
                match self
                    .background
                    .try_send(BackgroundTaskMessage::Run { task_id, future })
                {
                    Ok(()) => ApplicationTaskAdmission::Scheduled { task_id },
                    Err(TrySendError::Full(_)) => {
                        *state = BackgroundSchedulerState::WorkerFailed;
                        changed.notify_all();
                        ApplicationTaskAdmission::WorkerFailed
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        *state = BackgroundSchedulerState::WorkerFailed;
                        changed.notify_all();
                        ApplicationTaskAdmission::WorkerFailed
                    }
                }
            }
            BackgroundSchedulerState::Occupied { task_id } => {
                ApplicationTaskAdmission::AtCapacity {
                    occupying_task_id: task_id,
                }
            }
            BackgroundSchedulerState::Closing => ApplicationTaskAdmission::Closing,
            BackgroundSchedulerState::Closed => ApplicationTaskAdmission::Closed,
            BackgroundSchedulerState::WorkerFailed => ApplicationTaskAdmission::WorkerFailed,
        }
    }

    fn shutdown_background_tasks(&self) -> ApplicationTaskShutdown {
        let (state, changed) = &*self.background_state;
        let mut state = state.lock().expect("background scheduler state lock");
        let disposition = match *state {
            BackgroundSchedulerState::Open => {
                *state = BackgroundSchedulerState::Closing;
                match self.background.try_send(BackgroundTaskMessage::Shutdown) {
                    Ok(()) => ShutdownDisposition::Initiated,
                    Err(TrySendError::Full(_)) => {
                        *state = BackgroundSchedulerState::WorkerFailed;
                        changed.notify_all();
                        ShutdownDisposition::WorkerFailed
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        *state = BackgroundSchedulerState::Closed;
                        changed.notify_all();
                        ShutdownDisposition::AlreadyClosed
                    }
                }
            }
            BackgroundSchedulerState::Occupied { .. } => {
                *state = BackgroundSchedulerState::Closing;
                ShutdownDisposition::Initiated
            }
            BackgroundSchedulerState::Closing => ShutdownDisposition::Awaiting,
            BackgroundSchedulerState::Closed => ShutdownDisposition::AlreadyClosed,
            BackgroundSchedulerState::WorkerFailed => ShutdownDisposition::WorkerFailed,
        };
        if matches!(disposition, ShutdownDisposition::WorkerFailed) {
            return ApplicationTaskShutdown::WorkerFailed;
        }
        while matches!(*state, BackgroundSchedulerState::Closing) {
            state = changed
                .wait(state)
                .expect("background scheduler state lock");
        }
        match *state {
            BackgroundSchedulerState::Closed => match disposition {
                ShutdownDisposition::Initiated => ApplicationTaskShutdown::Closed,
                ShutdownDisposition::Awaiting | ShutdownDisposition::AlreadyClosed => {
                    ApplicationTaskShutdown::AlreadyClosed
                }
                ShutdownDisposition::WorkerFailed => ApplicationTaskShutdown::WorkerFailed,
            },
            BackgroundSchedulerState::WorkerFailed => ApplicationTaskShutdown::WorkerFailed,
            BackgroundSchedulerState::Open | BackgroundSchedulerState::Occupied { .. } => {
                ApplicationTaskShutdown::WorkerFailed
            }
            BackgroundSchedulerState::Closing => unreachable!("closing state was awaited"),
        }
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(sleep(duration))
    }
}

fn background_task_worker(
    runtime: Runtime,
    receiver: Receiver<BackgroundTaskMessage>,
    shared_state: Arc<(Mutex<BackgroundSchedulerState>, Condvar)>,
) {
    while let Ok(message) = receiver.recv() {
        match message {
            BackgroundTaskMessage::Run { task_id, future } => {
                let (state, changed) = &*shared_state;
                let mut state = state.lock().expect("background scheduler state lock");
                match *state {
                    BackgroundSchedulerState::Occupied {
                        task_id: active_task,
                    } if active_task == task_id => {
                        drop(state);
                        let task_result =
                            catch_unwind(AssertUnwindSafe(|| runtime.block_on(future)));
                        state = shared_state
                            .0
                            .lock()
                            .expect("background scheduler state lock");
                        match (task_result, *state) {
                            (Err(_), BackgroundSchedulerState::Closed)
                            | (Err(_), BackgroundSchedulerState::WorkerFailed) => {}
                            (Err(_), _) => *state = BackgroundSchedulerState::WorkerFailed,
                            (
                                Ok(()),
                                BackgroundSchedulerState::Occupied {
                                    task_id: active_task,
                                },
                            ) if active_task == task_id => *state = BackgroundSchedulerState::Open,
                            (Ok(()), BackgroundSchedulerState::Closing) => {
                                *state = BackgroundSchedulerState::Closed;
                            }
                            (
                                Ok(()),
                                BackgroundSchedulerState::WorkerFailed
                                | BackgroundSchedulerState::Closed,
                            ) => {}
                            (Ok(()), BackgroundSchedulerState::Open)
                            | (Ok(()), BackgroundSchedulerState::Occupied { .. }) => {
                                *state = BackgroundSchedulerState::WorkerFailed;
                            }
                        }
                        changed.notify_all();
                        if matches!(
                            *state,
                            BackgroundSchedulerState::Closed
                                | BackgroundSchedulerState::WorkerFailed
                        ) {
                            return;
                        }
                    }
                    BackgroundSchedulerState::Closing => {
                        *state = BackgroundSchedulerState::Closed;
                        changed.notify_all();
                        return;
                    }
                    BackgroundSchedulerState::Open
                    | BackgroundSchedulerState::Occupied { .. }
                    | BackgroundSchedulerState::Closed
                    | BackgroundSchedulerState::WorkerFailed => {
                        *state = BackgroundSchedulerState::WorkerFailed;
                        changed.notify_all();
                        return;
                    }
                }
            }
            BackgroundTaskMessage::Shutdown => {
                let (state, changed) = &*shared_state;
                let mut state = state.lock().expect("background scheduler state lock");
                match *state {
                    BackgroundSchedulerState::Closing => {
                        *state = BackgroundSchedulerState::Closed;
                    }
                    BackgroundSchedulerState::Closed => {}
                    BackgroundSchedulerState::Open
                    | BackgroundSchedulerState::Occupied { .. }
                    | BackgroundSchedulerState::WorkerFailed => {
                        *state = BackgroundSchedulerState::WorkerFailed;
                    }
                }
                changed.notify_all();
                return;
            }
        }
    }
    let (state, changed) = &*shared_state;
    let mut state = state.lock().expect("background scheduler state lock");
    *state = match *state {
        BackgroundSchedulerState::Closing | BackgroundSchedulerState::Closed => {
            BackgroundSchedulerState::Closed
        }
        BackgroundSchedulerState::Open
        | BackgroundSchedulerState::Occupied { .. }
        | BackgroundSchedulerState::WorkerFailed => BackgroundSchedulerState::WorkerFailed,
    };
    changed.notify_all();
}

pub fn tokio_application_runtime() -> Result<Arc<dyn ApplicationRuntime>, String> {
    Ok(Arc::new(TokioApplicationRuntime::new()?))
}

/// Build the standard Mihomo-backed Overview pump for a product composition.
pub fn spawn_mihomo_overview(config: OverviewConfig) -> OverviewPump {
    let runtime =
        tokio_application_runtime().expect("Tokio application runtime must be constructible");
    let reader: Arc<dyn OverviewReader> =
        match MihomoClient::new(&config.endpoint, config.secret.clone()) {
            Ok(client) => Arc::new(ControllerOverviewReader::new(client)),
            Err(error) => Arc::new(UnavailableOverviewReader::new(PortError::Network(
                error.to_string(),
            ))),
        };
    OverviewPump::spawn(reader, config.sample_interval, runtime)
}

#[cfg(test)]
mod background_runtime_tests;

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
    let host = Arc::new(IosHostAdapter::new(bridge));
    let application = CoreApplication::new_with_overview(
        host.clone(),
        Arc::new(ControllerReadiness::new(
            controller_url.clone(),
            secret.clone(),
        )),
        Arc::new(ControllerOverviewReader::new(client.clone())),
        runtime.clone(),
    );
    application
        .install_log_gateway(Arc::new(client.clone()))
        .map_err(|failure| failure.message)?;
    let core_capabilities =
        CoreCapabilityApplication::new(Arc::new(MihomoCoreCapabilityProvider::new(
            HostKind::Ios,
            [
                CoreOperation::Outbound(OutboundOperation::Select),
                CoreOperation::Outbound(OutboundOperation::Probe),
                CoreOperation::Connection(ConnectionOperation::Close),
                CoreOperation::Logs(LogsOperation::SetLevel),
                CoreOperation::Geo(GeoOperation::UpdateDatabases),
                CoreOperation::Provider(ProviderOperation::ListRule),
                CoreOperation::Provider(ProviderOperation::UpdateRule),
                CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
                CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
                CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
                CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
                CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
            ],
        )));
    application
        .install_core_capability_application(core_capabilities.clone())
        .map_err(|failure| failure.message)?;
    application.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_core_capabilities(core_capabilities)
            .with_host_capabilities(host.clone())
            .with_logs(application.log_application())
            .with_routing_intent_port(Arc::new(MihomoRoutingIntentAdapter::new(client.clone())))
            .with_core_log_level_port(Arc::new(client.clone()))
            .with_runtime(Arc::new(client))
            .with_application_runtime(runtime)
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

pub mod engine_backend_factory;
pub mod snapshot_restore_fixture;
