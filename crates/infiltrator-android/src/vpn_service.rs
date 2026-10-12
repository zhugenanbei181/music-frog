//! Android VpnService host adapter.
//!
//! Kotlin owns Context, VpnService.prepare, Builder.establish and the
//! foreground notification. Rust owns request validation and tun2proxy packet
//! processing, returning a typed start receipt before Kotlin publishes the
//! terminal lifecycle state.

#[cfg(target_os = "android")]
use crate::host_support::get_runtime;
use async_trait::async_trait;
#[cfg(any(target_os = "android", test))]
use infiltrator_contract::android_control::AndroidHostProcessRole;
use infiltrator_contract::android_control::VpnAuthorizationState;
#[cfg(any(target_os = "android", test))]
use infiltrator_contract::android_control::{
    VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerSnapshot,
};
#[cfg(not(target_os = "android"))]
use infiltrator_contract::capability::Capability;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnSessionSnapshot, VpnSessionState, VpnStartRequest,
    VpnSurfaceObservation,
};
#[cfg(target_os = "android")]
use infiltrator_contract::vpn::{
    VpnConfigurationApplyOutcome, VpnOwnerStopReceipt, VpnStartOutcome, VpnStopTarget,
};
use infiltrator_platform::android_bridge::{AndroidBridge, get_android_bridge};
use infiltrator_ports::error::PortError;
use infiltrator_ports::vpn_service::{VpnServicePort, VpnStartReceipt};
use std::sync::Arc;
#[cfg(target_os = "android")]
use std::thread::Builder;
#[cfg(any(target_os = "android", test))]
use std::time::Duration;
#[cfg(any(target_os = "android", test))]
mod worker;
#[cfg(any(target_os = "android", test))]
pub(crate) use worker::WorkerStopOutcome;

#[cfg(any(target_os = "android", test))]
const WORKER_STOP_TIMEOUT: Duration = Duration::from_secs(4);

#[cfg(any(target_os = "android", all(unix, test)))]
mod tun_fd;

#[cfg(target_os = "android")]
use tun_fd::{prepare_with_tun_fd, spawn_with_tun_fd};
#[cfg(target_os = "android")]
use worker::{
    WorkerControlError, WorkerReadyOutcome, WorkerStartFailure, WorkerStartReceipt, begin_worker,
    finish_worker, publish_worker_ready, report_first_pending, report_start_failure,
};

#[cfg(target_os = "android")]
fn run_tun2proxy_worker(
    args: tun2proxy::Args,
    mtu: u16,
    tun_fd: i32,
    stop_receiver: tokio::sync::oneshot::Receiver<()>,
    receipt_sender: tokio::sync::oneshot::Sender<WorkerStartReceipt>,
) -> i32 {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            report_start_failure(
                receipt_sender,
                WorkerStartFailure::RunLoopInitialization(format!(
                    "failed to create tun2proxy runtime: {error}"
                )),
            );
            return -1;
        }
    };

    let result = runtime.block_on(async move {
        let mut configuration = tun2::Configuration::default();
        configuration.raw_fd(tun_fd);
        configuration.close_fd_on_drop(false);
        let device = match tun2::create_as_async(&configuration) {
            Ok(device) => device,
            Err(error) => {
                let message = error.to_string();
                report_start_failure(
                    receipt_sender,
                    WorkerStartFailure::TunnelDeviceCreation(message.clone()),
                );
                return Err(message);
            }
        };

        let shutdown = tun2proxy::CancellationToken::new();
        let shutdown_from_stop = shutdown.clone();
        tokio::spawn(async move {
            let _ = stop_receiver.await;
            shutdown_from_stop.cancel();
        });

        // `tun2proxy::run` performs its synchronous stack setup before its
        // first await at `IpStack::accept()`. Because the AsyncDevice above
        // was created from this exact duplicated Android FD, its first
        // Pending poll is the first trustworthy worker-ready boundary.
        report_first_pending(
            async move {
                tun2proxy::run(device, mtu, args, shutdown)
                    .await
                    .map_err(|error| error.to_string())
            },
            receipt_sender,
        )
        .await
    });

    match result {
        Ok(()) => 0,
        Err(message) => {
            log::error!("Android tun2proxy worker exited: {message}");
            -2
        }
    }
}

#[cfg(any(target_os = "android", test))]
pub(crate) fn stop_data_plane_worker() -> WorkerStopOutcome {
    worker::stop_worker_and_wait(WORKER_STOP_TIMEOUT)
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct AndroidVpnServicePort;

impl Default for AndroidVpnServicePort {
    fn default() -> Self {
        Self::shared()
    }
}

#[allow(dead_code)]
impl AndroidVpnServicePort {
    pub fn new() -> Self {
        Self
    }

    pub fn shared() -> Self {
        Self
    }

    fn bridge() -> Result<Arc<dyn AndroidBridge>, PortError> {
        get_android_bridge()
            .ok_or_else(|| PortError::Failed("android bridge is not ready".to_owned()))
    }

    #[cfg(any(target_os = "android", test))]
    fn owner_unavailable_stop(&self, message: impl Into<String>) -> PortError {
        let message = message.into();
        let worker_cleanup = match get_android_bridge().map(|bridge| bridge.process_role()) {
            Some(AndroidHostProcessRole::CoreOwner) => stop_data_plane_worker(),
            Some(AndroidHostProcessRole::UiClient)
            | Some(AndroidHostProcessRole::Unsupported)
            | None => WorkerStopOutcome::Unavailable,
        };
        let message = match worker_cleanup {
            WorkerStopOutcome::Stopped => message,
            WorkerStopOutcome::TimedOut => {
                format!("{message}; bounded local worker cleanup timed out")
            }
            WorkerStopOutcome::Unavailable => {
                format!("{message}; bounded local worker cleanup was unavailable")
            }
        };
        let failure = Failure::new(ErrorCode::NotReady, message, true);
        PortError::Rejected(failure)
    }

    #[cfg(target_os = "android")]
    fn owner_stop_failure(&self, code: ErrorCode, message: impl Into<String>) -> PortError {
        let failure = Failure::new(code, message, true);
        PortError::Rejected(failure)
    }

    #[cfg(target_os = "android")]
    async fn start_tun2proxy(
        &self,
        request: VpnStartRequest,
        bridge: Arc<dyn AndroidBridge>,
    ) -> Result<VpnStartReceipt, PortError> {
        let owner_snapshot = bridge
            .vpn_owner_snapshot()
            .await
            .map_err(|error| PortError::Failed(error.to_string()))?;
        match vpn_tunnel_start_gate(&owner_snapshot) {
            VpnTunnelStartGate::Established => {}
            VpnTunnelStartGate::AwaitingTunnel(state) => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    format!("Android VpnService tunnel is not established yet: {state:?}"),
                    true,
                )));
            }
            VpnTunnelStartGate::LifecycleConflict(VpnOwnerLifecycle::Unavailable { .. }) => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "Android VpnService owner is unavailable",
                    true,
                )));
            }
            VpnTunnelStartGate::LifecycleConflict(state) => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    format!("Android VpnService cannot start the data plane from {state:?}"),
                    false,
                )));
            }
        }

        // The FFI descriptor is borrowed from Kotlin. After foreground
        // validation, Rust duplicates it and owns that copy through every
        // configuration failure and the full worker call. tun2proxy borrows
        // the duplicate; it must not become a second close owner.
        let vpn_generation = request.prepared.vpn_generation;
        let configuration = &request.prepared.configuration;
        let (tun_fd, (args, mtu)) =
            prepare_with_tun_fd(request.tun_fd, |fd| -> Result<_, String> {
                let proxy = tun2proxy::ArgProxy::try_from(configuration.proxy_endpoint.as_str())
                    .map_err(|error| format!("invalid tun2proxy endpoint: {error}"))?;
                let mut args = tun2proxy::Args {
                    proxy,
                    tun_fd: Some(fd),
                    close_fd_on_drop: Some(false),
                    ipv6_enabled: configuration.ipv6,
                    setup: false,
                    ..Default::default()
                };
                if let Some(server) = configuration.dns_servers.first()
                    && let Ok(address) = server.parse()
                {
                    args.dns_addr = address;
                }
                let mtu = u16::try_from(configuration.mtu)
                    .map_err(|_| "VPN MTU does not fit tun2proxy".to_owned())?;
                Ok((args, mtu))
            })
            .map_err(|error| PortError::Failed(error.to_string()))?;
        let running = VpnSessionSnapshot::running(
            owner_snapshot.revision,
            configuration.mtu,
            configuration.routes.len(),
            configuration.dns_servers.clone(),
            configuration.ipv6,
            true,
        );
        let permit = begin_worker().map_err(|error| {
            let message = match error {
                WorkerControlError::Busy(activity) => {
                    format!("Android tun2proxy worker is already {activity:?}")
                }
                WorkerControlError::Unavailable => {
                    "Android tun2proxy worker lifecycle is unavailable".to_owned()
                }
            };
            PortError::Rejected(Failure::new(ErrorCode::NotReady, message, true))
        })?;
        let worker_generation = permit.generation;
        let stop_receiver = permit.stop_receiver;
        let receipt_sender = permit.receipt_sender;
        let receipt_receiver = permit.receipt_receiver;
        let raw_tun_fd = tun_fd.raw_fd();
        let exit_bridge = bridge.clone();
        let spawn_result = spawn_with_tun_fd(
            Builder::new().name("infiltrator-android-vpn".to_owned()),
            tun_fd,
            move || {
                let exit_code =
                    run_tun2proxy_worker(args, mtu, raw_tun_fd, stop_receiver, receipt_sender);
                if let Err(error) = finish_worker(worker_generation) {
                    log::error!("failed to finalize Android tun2proxy worker state: {error:?}");
                }
                if exit_code != 0 {
                    log::error!("tun2proxy worker exited unexpectedly with code {exit_code}");
                    get_runtime().spawn(async move {
                        match exit_bridge
                            .vpn_stop(VpnStopTarget::Generation(vpn_generation))
                            .await
                        {
                            Ok(
                                VpnOwnerStopReceipt::Stopped { .. }
                                | VpnOwnerStopReceipt::Superseded { .. }
                                | VpnOwnerStopReceipt::SupersededWithReadback { .. },
                            ) => {}
                            Ok(receipt) => {
                                log::error!("Android VPN worker-exit stop returned {:?}", receipt)
                            }
                            Err(error) => {
                                log::error!("failed to stop Android VPN after worker exit: {error}")
                            }
                        }
                    });
                }
            },
        );
        match spawn_result {
            Err(error) => {
                let _ = finish_worker(worker_generation);
                let failure = Failure::new(
                    ErrorCode::Internal,
                    format!("failed to spawn tun2proxy: {error}"),
                    true,
                );
                return Err(PortError::Io(failure.message));
            }
            Ok(_worker) => match receipt_receiver.await {
                Ok(WorkerStartReceipt::Started) => match publish_worker_ready(worker_generation) {
                    Ok(WorkerReadyOutcome::Published) => {
                        return Ok(VpnStartReceipt::Started(running));
                    }
                    Ok(WorkerReadyOutcome::StopInProgress) => {
                        return Err(PortError::Rejected(Failure::new(
                            ErrorCode::Canceled,
                            "VPN worker became ready after its stop was requested",
                            true,
                        )));
                    }
                    Ok(WorkerReadyOutcome::StaleGeneration) => {
                        return Err(PortError::Rejected(Failure::new(
                            ErrorCode::NotReady,
                            "VPN worker readiness receipt belongs to a stale generation",
                            true,
                        )));
                    }
                    Err(WorkerControlError::Busy(activity)) => {
                        return Err(PortError::Rejected(Failure::new(
                            ErrorCode::NotReady,
                            format!(
                                "VPN worker lifecycle changed to {activity:?} before readiness publication"
                            ),
                            true,
                        )));
                    }
                    Err(WorkerControlError::Unavailable) => {
                        return Err(PortError::Failed(
                            "VPN worker lifecycle became unavailable before readiness publication"
                                .to_owned(),
                        ));
                    }
                },
                Ok(WorkerStartReceipt::Failed(failure)) => {
                    let message = failure.to_string();
                    let failed = Failure::new(ErrorCode::NotReady, message, true);
                    return Err(PortError::Rejected(failed));
                }
                Err(_) => {
                    let failure = Failure::new(
                        ErrorCode::Internal,
                        "tun2proxy worker exited before sending a readiness receipt",
                        true,
                    );
                    return Err(PortError::Rejected(failure));
                }
            },
        }
    }
}

#[async_trait]
impl VpnServicePort for AndroidVpnServicePort {
    async fn owner_observation(&self) -> VpnSurfaceObservation {
        #[cfg(target_os = "android")]
        {
            return match Self::bridge() {
                Ok(bridge) => match bridge.vpn_owner_snapshot().await {
                    Ok(snapshot) => VpnSurfaceObservation::AndroidOwner(snapshot),
                    Err(error) => VpnSurfaceObservation::Unavailable {
                        failure: Failure::new(ErrorCode::NotReady, error.to_string(), true),
                    },
                },
                Err(error) => VpnSurfaceObservation::Unavailable {
                    failure: Failure::from(error),
                },
            };
        }

        #[cfg(not(target_os = "android"))]
        VpnSurfaceObservation::Unsupported {
            reason: "Android VpnService owner snapshots are unavailable on this host".to_owned(),
        }
    }

    async fn authorization_state(&self) -> Result<VpnAuthorizationState, PortError> {
        #[cfg(target_os = "android")]
        {
            return Self::bridge()?
                .vpn_authorization_state()
                .await
                .map_err(|error| PortError::Failed(error.to_string()));
        }

        #[cfg(not(target_os = "android"))]
        Err(PortError::unsupported(
            Capability::VpnService,
            "Android VPN authorization is unavailable on this host",
        ))
    }

    async fn request_start(&self) -> Result<VpnSessionSnapshot, PortError> {
        #[cfg(target_os = "android")]
        {
            let bridge = Self::bridge()?;
            let outcome = bridge
                .vpn_start()
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?;
            match outcome {
                VpnStartOutcome::PermissionRequired => {
                    let snapshot = bridge
                        .vpn_owner_snapshot()
                        .await
                        .map_err(|error| PortError::Failed(error.to_string()))?;
                    Ok(vpn_session_projection(&snapshot))
                }
                VpnStartOutcome::OwnerUnavailable => Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "Android VpnService owner is unavailable",
                    true,
                ))),
                VpnStartOutcome::Failed => Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "Android VpnService start transaction failed",
                    true,
                ))),
                VpnStartOutcome::TimedOut => Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "Android VpnService start transaction timed out",
                    true,
                ))),
                VpnStartOutcome::Running => {
                    let snapshot = bridge
                        .vpn_owner_snapshot()
                        .await
                        .map_err(|error| PortError::Failed(error.to_string()))?;
                    Ok(vpn_session_projection(&snapshot))
                }
            }
        }

        #[cfg(not(target_os = "android"))]
        {
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }

    async fn prepare(
        &self,
        prepared: PreparedVpnConfiguration,
    ) -> Result<VpnSessionSnapshot, PortError> {
        #[cfg(target_os = "android")]
        {
            let bridge = Self::bridge()?;
            if !prepared
                .core_scope
                .session_token
                .is_some_and(|token| token.is_valid())
            {
                return Err(PortError::Failed(
                    "prepared VPN Core session is unavailable".to_owned(),
                ));
            }
            match bridge
                .vpn_apply_configuration(&prepared)
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?
            {
                VpnConfigurationApplyOutcome::Accepted => {}
                VpnConfigurationApplyOutcome::Rejected => {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::Internal,
                        "native VpnService rejected route/DNS/MTU configuration",
                        false,
                    )));
                }
                VpnConfigurationApplyOutcome::OwnerUnavailable => {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::NotReady,
                        "Android VpnService owner is unavailable while applying Builder configuration",
                        true,
                    )));
                }
            }
            let snapshot = bridge
                .vpn_owner_snapshot()
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?;
            return Ok(vpn_session_projection(&snapshot));
        }

        #[cfg(not(target_os = "android"))]
        {
            let _ = prepared;
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }

    async fn start(&self, request: VpnStartRequest) -> Result<VpnStartReceipt, PortError> {
        #[cfg(target_os = "android")]
        {
            let owner_snapshot = Self::bridge()?
                .vpn_owner_snapshot()
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?;
            if owner_snapshot.is_running() {
                return Ok(VpnStartReceipt::AlreadyRunning(vpn_session_projection(
                    &owner_snapshot,
                )));
            }
            return self.start_tun2proxy(request, Self::bridge()?).await;
        }

        #[cfg(not(target_os = "android"))]
        {
            let _ = request;
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }

    async fn stop(&self) -> Result<VpnSessionSnapshot, PortError> {
        #[cfg(target_os = "android")]
        {
            let bridge = match Self::bridge() {
                Ok(bridge) => bridge,
                Err(error) => {
                    return Err(self.owner_unavailable_stop(format!(
                        "Android VpnService owner is unavailable during stop: {error}"
                    )));
                }
            };
            let receipt = match bridge.vpn_stop(VpnStopTarget::Current).await {
                Ok(receipt) => receipt,
                Err(error) => {
                    return Err(self.owner_unavailable_stop(format!(
                        "Android VpnService owner stop request failed: {error}"
                    )));
                }
            };
            let snapshot = match receipt {
                VpnOwnerStopReceipt::Stopped { snapshot } => snapshot,
                VpnOwnerStopReceipt::OwnerUnavailable { failure }
                | VpnOwnerStopReceipt::OwnerUnavailableWithReadback { failure, .. } => {
                    return Err(self.owner_unavailable_stop(failure.message));
                }
                VpnOwnerStopReceipt::Failed { failure }
                | VpnOwnerStopReceipt::FailedWithReadback { failure, .. }
                | VpnOwnerStopReceipt::TimedOut { failure }
                | VpnOwnerStopReceipt::TimedOutWithReadback { failure, .. }
                | VpnOwnerStopReceipt::Superseded { failure }
                | VpnOwnerStopReceipt::SupersededWithReadback { failure, .. }
                | VpnOwnerStopReceipt::Canceled { failure }
                | VpnOwnerStopReceipt::CanceledWithReadback { failure, .. } => {
                    return Err(PortError::Rejected(failure));
                }
            };
            if !snapshot.is_stopped() {
                let code = match snapshot.vpn_state {
                    VpnOwnerLifecycle::Unavailable { .. } => ErrorCode::NotReady,
                    _ => ErrorCode::InvalidState,
                };
                return Err(self.owner_stop_failure(
                    code,
                    format!(
                        "Android VpnService stop snapshot is not terminal with all resources released: state={:?}, readiness={:?}, resources={:?}",
                        snapshot.vpn_state, snapshot.readiness, snapshot.resources
                    ),
                ));
            }
            return Ok(vpn_session_projection(&snapshot));
        }

        #[cfg(not(target_os = "android"))]
        {
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }

    async fn revoke(&self) -> Result<VpnSessionSnapshot, PortError> {
        #[cfg(target_os = "android")]
        {
            let snapshot = Self::bridge()?
                .vpn_owner_snapshot()
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?;
            return match snapshot.vpn_state {
                VpnOwnerLifecycle::Revoked => Ok(vpn_session_projection(&snapshot)),
                state => Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    format!("Android VpnService owner has not published Revoked: {state:?}"),
                    true,
                ))),
            };
        }

        #[cfg(not(target_os = "android"))]
        {
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }

    async fn snapshot(&self) -> Result<VpnSessionSnapshot, PortError> {
        #[cfg(target_os = "android")]
        {
            let bridge = Self::bridge()?;
            let snapshot = bridge
                .vpn_owner_snapshot()
                .await
                .map_err(|error| PortError::Failed(error.to_string()))?;
            return Ok(vpn_session_projection(&snapshot));
        }

        #[cfg(not(target_os = "android"))]
        {
            Err(PortError::unsupported(
                Capability::VpnService,
                "Android VpnService is unavailable on this host",
            ))
        }
    }
}

#[cfg(any(target_os = "android", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
enum VpnTunnelStartGate {
    Established,
    AwaitingTunnel(VpnOwnerLifecycle),
    LifecycleConflict(VpnOwnerLifecycle),
}

#[cfg(any(target_os = "android", test))]
fn vpn_tunnel_start_gate(snapshot: &VpnOwnerSnapshot) -> VpnTunnelStartGate {
    match (&snapshot.vpn_state, snapshot.readiness) {
        (VpnOwnerLifecycle::TunnelEstablished, VpnOwnerReadiness::AwaitingDataPlane) => {
            VpnTunnelStartGate::Established
        }
        (
            VpnOwnerLifecycle::StartRequested
            | VpnOwnerLifecycle::Starting
            | VpnOwnerLifecycle::Foreground,
            _,
        ) => VpnTunnelStartGate::AwaitingTunnel(snapshot.vpn_state.clone()),
        (VpnOwnerLifecycle::PermissionRequired, _)
        | (VpnOwnerLifecycle::TunnelEstablished, _)
        | (VpnOwnerLifecycle::Running, _)
        | (VpnOwnerLifecycle::Stopping, _)
        | (VpnOwnerLifecycle::Stopped, _)
        | (VpnOwnerLifecycle::Revoked, _)
        | (VpnOwnerLifecycle::Failed { .. }, _)
        | (VpnOwnerLifecycle::Unavailable { .. }, _) => {
            VpnTunnelStartGate::LifecycleConflict(snapshot.vpn_state.clone())
        }
    }
}

#[cfg(any(target_os = "android", test))]
fn vpn_session_projection(snapshot: &VpnOwnerSnapshot) -> VpnSessionSnapshot {
    let state = match (&snapshot.vpn_state, snapshot.readiness) {
        (VpnOwnerLifecycle::PermissionRequired, _)
        | (_, VpnOwnerReadiness::PermissionRequired) => VpnSessionState::PermissionRequired,
        (VpnOwnerLifecycle::StartRequested, _)
        | (VpnOwnerLifecycle::Starting, _)
        | (VpnOwnerLifecycle::Foreground, _)
        | (VpnOwnerLifecycle::TunnelEstablished, _)
        | (_, VpnOwnerReadiness::AwaitingForeground)
        | (_, VpnOwnerReadiness::AwaitingTunnel)
        | (_, VpnOwnerReadiness::AwaitingDataPlane) => VpnSessionState::Starting,
        (VpnOwnerLifecycle::Running, VpnOwnerReadiness::Ready) => VpnSessionState::Running,
        (VpnOwnerLifecycle::Stopping, _) | (_, VpnOwnerReadiness::Stopping) => {
            VpnSessionState::Stopping
        }
        (VpnOwnerLifecycle::Stopped, VpnOwnerReadiness::CleanupPending)
        | (VpnOwnerLifecycle::Revoked, VpnOwnerReadiness::CleanupPending)
        | (_, VpnOwnerReadiness::CleanupPending) => VpnSessionState::Failed {
            failure: Failure::new(
                ErrorCode::NotReady,
                snapshot
                    .resources
                    .iter()
                    .find_map(|resource| match &resource.state {
                        infiltrator_contract::android_control::VpnOwnerResourceState::CleanupPending {
                            failure_message,
                            ..
                        } => Some(failure_message.clone()),
                        infiltrator_contract::android_control::VpnOwnerResourceState::Absent
                        | infiltrator_contract::android_control::VpnOwnerResourceState::Held {
                            ..
                        } => None,
                    })
                    .unwrap_or_else(|| "VPN owner cleanup is pending".to_owned()),
                true,
            ),
        },
        (VpnOwnerLifecycle::Stopped, _) => VpnSessionState::Stopped,
        (VpnOwnerLifecycle::Revoked, _) => VpnSessionState::Revoked,
        (VpnOwnerLifecycle::Failed { failure_message }, _) => VpnSessionState::Failed {
            failure: Failure::new(
                ErrorCode::InvalidState,
                failure_message.clone(),
                true,
            ),
        },
        (VpnOwnerLifecycle::Unavailable { failure_message }, _) => VpnSessionState::Unsupported {
            reason: failure_message.clone(),
        },
        (_, VpnOwnerReadiness::Unavailable) => VpnSessionState::Unsupported {
            reason: "Android VpnService owner is unavailable".to_owned(),
        },
        (_, VpnOwnerReadiness::Failed) => VpnSessionState::Failed {
            failure: Failure::new(
                ErrorCode::InvalidState,
                "VPN owner published failed readiness without failure detail",
                true,
            ),
        },
        (VpnOwnerLifecycle::Running, _) => VpnSessionState::Failed {
            failure: Failure::new(
                ErrorCode::NotReady,
                "VPN owner lifecycle and readiness disagree",
                true,
            ),
        },
    };
    VpnSessionSnapshot {
        state,
        foreground: snapshot.foreground,
        mtu: snapshot.mtu,
        route_count: snapshot.route_count as usize,
        dns_servers: snapshot.dns_servers.clone(),
        ipv6: snapshot.ipv6,
        revision: snapshot.revision,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::android_control::{
        VpnOwnerCoreReadback, VpnOwnerResourceFact, VpnOwnerResourceKind, VpnOwnerResourceState,
    };
    use infiltrator_contract::snapshot::CoreLifecycle;
    use infiltrator_contract::vpn::VpnGeneration;

    fn owner_snapshot(
        state: VpnOwnerLifecycle,
        readiness: VpnOwnerReadiness,
        resources: Vec<VpnOwnerResourceFact>,
    ) -> VpnOwnerSnapshot {
        VpnOwnerSnapshot {
            owner_session_id: "owner-session".to_owned(),
            revision: 7,
            vpn_generation: VpnGeneration::new(4),
            foreground: matches!(
                &state,
                VpnOwnerLifecycle::Foreground
                    | VpnOwnerLifecycle::TunnelEstablished
                    | VpnOwnerLifecycle::Running
                    | VpnOwnerLifecycle::Stopping
            ),
            vpn_state: state,
            readiness,
            resources,
            mtu: Some(1500),
            route_count: 1,
            dns_servers: vec!["1.1.1.1".to_owned()],
            ipv6: false,
            core: VpnOwnerCoreReadback::Available(
                infiltrator_contract::snapshot::CoreLifecycleSnapshot {
                    identity: None,
                    lifecycle: CoreLifecycle::Stopped,
                    generation: 0,
                    session_token: None,
                    revision: 0,
                    failure: None,
                },
            ),
        }
    }

    #[test]
    fn data_plane_start_requires_the_owner_tunnel_and_readiness_facts() {
        assert_eq!(
            vpn_tunnel_start_gate(&owner_snapshot(
                VpnOwnerLifecycle::TunnelEstablished,
                VpnOwnerReadiness::AwaitingDataPlane,
                Vec::new(),
            )),
            VpnTunnelStartGate::Established,
        );
        assert_eq!(
            vpn_tunnel_start_gate(&owner_snapshot(
                VpnOwnerLifecycle::Foreground,
                VpnOwnerReadiness::AwaitingTunnel,
                Vec::new(),
            )),
            VpnTunnelStartGate::AwaitingTunnel(VpnOwnerLifecycle::Foreground),
        );
        assert_eq!(
            vpn_tunnel_start_gate(&owner_snapshot(
                VpnOwnerLifecycle::Running,
                VpnOwnerReadiness::Ready,
                Vec::new(),
            )),
            VpnTunnelStartGate::LifecycleConflict(VpnOwnerLifecycle::Running),
        );
    }

    #[test]
    fn owner_projection_preserves_revision_generation_and_resource_metadata() {
        let snapshot = vpn_session_projection(&owner_snapshot(
            VpnOwnerLifecycle::Running,
            VpnOwnerReadiness::Ready,
            vec![VpnOwnerResourceFact {
                kind: VpnOwnerResourceKind::Worker,
                state: VpnOwnerResourceState::Held { generation: 4 },
            }],
        ));
        assert_eq!(snapshot.state, VpnSessionState::Running);
        assert!(snapshot.is_running());
        assert_eq!(snapshot.revision, 7);
        assert_eq!(snapshot.mtu, Some(1500));
        assert_eq!(snapshot.route_count, 1);
        assert_eq!(snapshot.dns_servers, vec!["1.1.1.1".to_owned()]);
    }

    #[test]
    fn cleanup_pending_resource_cannot_project_as_stopped() {
        let snapshot = vpn_session_projection(&owner_snapshot(
            VpnOwnerLifecycle::Stopped,
            VpnOwnerReadiness::CleanupPending,
            vec![VpnOwnerResourceFact {
                kind: VpnOwnerResourceKind::TunnelDescriptor,
                state: VpnOwnerResourceState::CleanupPending {
                    generation: 4,
                    failure_message: "descriptor close failed".to_owned(),
                },
            }],
        ));
        let VpnSessionState::Failed { failure } = snapshot.state else {
            panic!("pending owner resources must remain visible as failed cleanup");
        };
        assert_eq!(failure.message, "descriptor close failed");
    }

    #[test]
    fn owner_unavailable_stop_is_bounded_and_does_not_forge_a_snapshot() {
        let port = AndroidVpnServicePort::new();
        let error = port.owner_unavailable_stop("VpnService owner is unavailable");
        let PortError::Rejected(failure) = error else {
            panic!("owner unavailability must remain a typed stop failure");
        };
        assert_eq!(failure.code, ErrorCode::NotReady);
        assert!(failure.message.contains("VpnService owner is unavailable"));
    }

    #[cfg(not(target_os = "android"))]
    #[tokio::test]
    async fn non_android_host_returns_typed_vpn_unsupported() {
        let port = AndroidVpnServicePort::new();
        let failure = port
            .request_start()
            .await
            .expect_err("desktop test host must not emulate Android VpnService");
        assert!(matches!(
            failure,
            PortError::Unsupported {
                capability: Capability::VpnService,
                ..
            }
        ));
    }
}
