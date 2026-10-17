//! Process lifecycle helpers shared by the first start and the switch host.
//!
//! Every helper fails closed: a started process that cannot reach readiness or
//! bind its owner scope is stopped again, and stop success is only reported
//! after an exact `Stopped`/no-PID readback.

use super::{RuntimeFailurePhase, RuntimeScopeSource};
use futures_timer::Delay;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion,
};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::{CoreProcessStartAdmission, CoreReadiness};
use lurker_core_ports::engine_backend::EngineBackendPorts;
use lurker_core_ports::error::PortError;
use std::sync::Arc;
use std::time::{Duration, Instant};

const READINESS_RETRY_INTERVAL: Duration = Duration::from_millis(100);

/// A start failure with the exact phase that failed.
pub(super) struct ProcessStartFailure {
    pub(super) phase: RuntimeFailurePhase,
    pub(super) cause: PortError,
}

/// A stop failure with the exact phase that failed.
pub(super) struct ProcessStopFailure {
    pub(super) phase: RuntimeFailurePhase,
    pub(super) cause: PortError,
}

/// Require a stable Core identity, validate the optional configured instance
/// and ask the process adapter to admit a fresh launch.
pub(super) async fn prepare_standalone(
    backend: &EngineBackendPorts,
    expected_instance: Option<&CoreInstanceId>,
) -> Result<(CoreProcessStartContext, CoreIdentity), ProcessStartFailure> {
    let process = backend.process();
    let identity = process.core_identity().ok_or_else(|| ProcessStartFailure {
        phase: RuntimeFailurePhase::Identity,
        cause: PortError::Failed(format!(
            "{} backend process has no Core identity",
            backend.engine_id().as_str()
        )),
    })?;
    if identity.engine != *backend.engine_id() {
        return Err(ProcessStartFailure {
            phase: RuntimeFailurePhase::Identity,
            cause: PortError::Failed(format!(
                "{} backend process reports identity for {}",
                backend.engine_id().as_str(),
                identity.engine.as_str()
            )),
        });
    }
    if let Some(expected_instance) = expected_instance
        && identity.instance != *expected_instance
    {
        return Err(ProcessStartFailure {
            phase: RuntimeFailurePhase::Identity,
            cause: PortError::Failed(format!(
                "{} backend process reports instance {} instead of {}",
                backend.engine_id().as_str(),
                identity.instance.as_str(),
                expected_instance.as_str()
            )),
        });
    }
    let context = CoreProcessStartContext::standalone(identity.clone());
    match process.prepare_start(&context).await {
        Ok(CoreProcessStartAdmission::LaunchNewProcess) => {}
        Ok(CoreProcessStartAdmission::ReuseExactOwnedProcess) => {
            return Err(ProcessStartFailure {
                phase: RuntimeFailurePhase::PrepareStart,
                cause: PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "runtime start requires a fresh Core process, not an already owned one",
                    false,
                )),
            });
        }
        Err(cause) => {
            return Err(ProcessStartFailure {
                phase: RuntimeFailurePhase::PrepareStart,
                cause,
            });
        }
    }
    Ok((context, identity))
}

/// Start the admitted process, wait for readiness, read its version and bind
/// the exact owner scope into the backend runtime operations.
pub(super) async fn start_and_bind_owner(
    backend: &EngineBackendPorts,
    context: CoreProcessStartContext,
    identity: CoreIdentity,
    timeout: Duration,
) -> Result<(CoreScope, Option<CoreVersion>), ProcessStartFailure> {
    let process = backend.process();
    process
        .start(context)
        .await
        .map_err(|cause| ProcessStartFailure {
            phase: RuntimeFailurePhase::StartProcess,
            cause,
        })?;
    let version = match wait_for_readiness(backend.readiness().as_ref(), timeout).await {
        Ok(version) => version,
        Err(cause) => {
            let _ = process.stop().await;
            return Err(ProcessStartFailure {
                phase: RuntimeFailurePhase::Readiness,
                cause,
            });
        }
    };
    let scope = CoreScope {
        identity: CoreIdentity::new(identity.engine, identity.instance, version.clone()),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let source: Arc<RuntimeScopeSource> = Arc::new(RuntimeScopeSource {
        scope: scope.clone(),
    });
    if let Err(cause) = backend.bind_owner_core_scope(source) {
        let _ = process.stop().await;
        return Err(ProcessStartFailure {
            phase: RuntimeFailurePhase::BindOwnerScope,
            cause,
        });
    }
    Ok((scope, version))
}

/// Stop the exact process and require the exact `Stopped`/no-PID terminal.
pub(super) async fn stop_process(backend: &EngineBackendPorts) -> Result<(), ProcessStopFailure> {
    let process = backend.process();
    process.stop().await.map_err(|cause| ProcessStopFailure {
        phase: RuntimeFailurePhase::StopProcess,
        cause,
    })?;
    match process.status().await {
        Ok(CoreLifecycle::Stopped) => {}
        Ok(other) => {
            return Err(ProcessStopFailure {
                phase: RuntimeFailurePhase::StopTerminal,
                cause: PortError::Failed(format!(
                    "{} process reported {other:?} instead of Stopped after stop",
                    backend.engine_id().as_str()
                )),
            });
        }
        Err(cause) => {
            return Err(ProcessStopFailure {
                phase: RuntimeFailurePhase::StopTerminal,
                cause,
            });
        }
    }
    match process.pid().await {
        None => Ok(()),
        Some(pid) => Err(ProcessStopFailure {
            phase: RuntimeFailurePhase::StopTerminal,
            cause: PortError::Failed(format!(
                "{} process still reports pid {pid} after its Stopped terminal",
                backend.engine_id().as_str()
            )),
        }),
    }
}

/// Probe until the adapter reports readiness or the bounded deadline passes.
///
/// A zero timeout still permits one probe; every later retry is bounded by the
/// remaining budget and the fixed retry interval.
async fn wait_for_readiness(
    readiness: &dyn CoreReadiness,
    timeout: Duration,
) -> Result<Option<CoreVersion>, PortError> {
    let now = Instant::now();
    let deadline = now.checked_add(timeout).unwrap_or(now);
    loop {
        let error = match readiness.probe().await {
            Ok(_) => return readiness.active_core_version().await,
            Err(error) => error,
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(error);
        }
        Delay::new(remaining.min(READINESS_RETRY_INTERVAL)).await;
    }
}
