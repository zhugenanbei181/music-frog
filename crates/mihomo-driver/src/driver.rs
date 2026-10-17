//! Driver-facing port implementations: process supervision and controller
//! readiness.
//!
//! Both implementations are thin policy layers over the engine mechanics in
//! [`crate::process`] and [`crate::api`]. They own exactly the facts the
//! application boundary needs — PID ownership, exact terminal readback, and
//! the live controller version — and expose nothing else.

#[cfg(feature = "http")]
use crate::api::client::MihomoClient;
#[cfg(feature = "process")]
use crate::process;
#[cfg(any(feature = "http", feature = "process"))]
use async_trait::async_trait;
#[cfg(feature = "process")]
use lurker_core_contract::core_process::CoreProcessStartContext;
#[cfg(feature = "http")]
use lurker_core_contract::identity::CoreVersion;
#[cfg(feature = "process")]
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, EngineId};
#[cfg(feature = "process")]
use lurker_core_contract::lifecycle::CoreLifecycle;
#[cfg(feature = "http")]
use lurker_core_ports::core_process::CoreReadiness;
#[cfg(feature = "process")]
use lurker_core_ports::core_process::{CoreProcess, CoreProcessStartAdmission};
#[cfg(any(feature = "http", feature = "process"))]
use lurker_core_ports::error::PortError;
#[cfg(feature = "process")]
use std::path::PathBuf;
#[cfg(any(feature = "http", feature = "process"))]
use std::result;
#[cfg(feature = "process")]
use std::sync::{Mutex, PoisonError};
#[cfg(feature = "process")]
use tokio::time::{Duration, sleep};

#[cfg(feature = "process")]
type Result<T> = result::Result<T, PortError>;

/// Proves controller readiness by asking the live core for its version.
///
/// The endpoint and secret stay private: the application boundary only sees
/// the neutral `CoreReadiness` port.
#[cfg(feature = "http")]
#[derive(Clone)]
pub struct ControllerReadiness {
    endpoint: String,
    secret: Option<String>,
}

#[cfg(feature = "http")]
impl ControllerReadiness {
    pub fn new(endpoint: impl Into<String>, secret: Option<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            secret,
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

#[cfg(feature = "http")]
#[async_trait]
impl CoreReadiness for ControllerReadiness {
    async fn probe(&self) -> result::Result<String, PortError> {
        let client = MihomoClient::new(&self.endpoint, self.secret.clone())
            .map_err(|error| PortError::Network(error.to_string()))?;
        client
            .get_version()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?;
        Ok(self.endpoint.clone())
    }

    async fn active_core_version(&self) -> result::Result<Option<CoreVersion>, PortError> {
        let client = MihomoClient::new(&self.endpoint, self.secret.clone())
            .map_err(|error| PortError::Network(error.to_string()))?;
        let version = client
            .get_version()
            .await
            .map_err(|error| PortError::Network(error.to_string()))?
            .version;
        CoreVersion::new(version)
            .map(Some)
            .map_err(|error| PortError::Failed(error.to_string()))
    }
}

/// Supervises one desktop engine process instance.
///
/// The PID recorded in the PID file is not proof of ownership: a live PID
/// must still match the configured binary, and a record owned by another
/// instance is treated as a candidate orphan.
#[cfg(feature = "process")]
pub struct ProcessCoreController {
    binary_path: PathBuf,
    config_path: PathBuf,
    pid_file: PathBuf,
    log_dir: PathBuf,
    /// PID owned by this controller instance. A different live PID in the
    /// record is treated as a candidate orphan after the instance lock is
    /// reacquired by a new host process.
    owned_pid: Mutex<Option<u32>>,
    // CORE-002: Windows-only guard. Holds the kill-on-close Job Object handle
    // for the currently spawned core. The handle is deliberately never closed
    // while this process lives (see `process::JobObjectHandle`), so when the
    // GUI process dies the kernel closes the last job handle and terminates
    // the core. On Linux/Unix the equivalent guarantee comes from
    // PR_SET_PDEATHSIG armed inside the child at spawn time.
    #[cfg(windows)]
    job: Mutex<Option<process::JobObjectHandle>>,
}

/// Evidence that a stop reached the exact terminal process state.
#[cfg(feature = "process")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessExitObservation {
    Exited,
    TimedOut,
}

#[cfg(feature = "process")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PidRecordObservation {
    Missing,
    Recorded(u32),
    OwnedWithoutRecord(u32),
    Conflict { recorded: u32, owned: u32 },
}

#[cfg(feature = "process")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PidRecordReconciliation {
    NoStaleRecord,
    RemovedDeadRecord { pid: u32 },
    StoppedOrphan { pid: u32 },
}

#[cfg(feature = "process")]
impl ProcessCoreController {
    /// Uses an explicit product-owned data directory. The driver never
    /// resolves a product home path itself.
    pub fn with_home(binary_path: PathBuf, config_path: PathBuf, home: PathBuf) -> Self {
        let pid_file = home.join("mihomo.pid");
        Self::with_pid_file_and_log_dir(binary_path, config_path, pid_file, home)
    }

    /// Uses an explicit PID record; the log directory is the record's parent.
    pub fn with_pid_file(binary_path: PathBuf, config_path: PathBuf, pid_file: PathBuf) -> Self {
        let log_dir = pid_file
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        Self::with_pid_file_and_log_dir(binary_path, config_path, pid_file, log_dir)
    }

    fn with_pid_file_and_log_dir(
        binary_path: PathBuf,
        config_path: PathBuf,
        pid_file: PathBuf,
        log_dir: PathBuf,
    ) -> Self {
        Self {
            binary_path,
            config_path,
            pid_file,
            log_dir,
            owned_pid: Mutex::new(None),
            #[cfg(windows)]
            job: Mutex::new(None),
        }
    }

    async fn wait_for_exit(pid: u32) -> ProcessExitObservation {
        if process::wait_for_process_exit(pid).await {
            ProcessExitObservation::Exited
        } else {
            ProcessExitObservation::TimedOut
        }
    }

    async fn pid_record_observation(&self) -> Result<PidRecordObservation> {
        let owned = *self
            .owned_pid
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match process::read_pid_file(&self.pid_file).await {
            Ok(recorded) => match owned {
                Some(owned) if owned != recorded => {
                    Ok(PidRecordObservation::Conflict { recorded, owned })
                }
                Some(_) | None => Ok(PidRecordObservation::Recorded(recorded)),
            },
            Err(PortError::NotFound(_)) => match owned {
                Some(pid) => Ok(PidRecordObservation::OwnedWithoutRecord(pid)),
                None => Ok(PidRecordObservation::Missing),
            },
            Err(error) => Err(error),
        }
    }

    async fn read_running_pid(&self) -> Result<Option<u32>> {
        let pid = match self.pid_record_observation().await? {
            PidRecordObservation::Missing => return Ok(None),
            PidRecordObservation::Recorded(pid) | PidRecordObservation::OwnedWithoutRecord(pid) => {
                pid
            }
            PidRecordObservation::Conflict { recorded, owned } => {
                return Err(PortError::Failed(format!(
                    "PID record {recorded} conflicts with owned core process {owned}"
                )));
            }
        };

        if !process::is_process_alive(pid) {
            self.remove_pid_file_for(pid).await?;
            return Ok(None);
        }
        if !process::process_matches_binary(pid, &self.binary_path) {
            return Err(PortError::Failed(format!(
                "PID record {pid} does not belong to the configured core binary"
            )));
        }
        Ok(Some(pid))
    }

    async fn remove_pid_file_for(&self, expected_pid: u32) -> Result<()> {
        match process::read_pid_file(&self.pid_file).await {
            Ok(recorded) if recorded == expected_pid => {
                process::remove_pid_file(&self.pid_file).await
            }
            Ok(recorded) => Err(PortError::Failed(format!(
                "refusing to remove PID record {recorded} while closing process {expected_pid}"
            ))),
            Err(PortError::NotFound(_)) => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn clear_owned_pid(&self, expected_pid: u32) {
        let mut owned = self
            .owned_pid
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if *owned == Some(expected_pid) {
            *owned = None;
        }
    }

    async fn stop_owned_process(&self, pid: u32) -> result::Result<(), PortError> {
        if !process::is_process_alive(pid) {
            self.remove_pid_file_for(pid).await?;
            self.clear_owned_pid(pid);
            return Ok(());
        }
        if !process::process_matches_binary(pid, &self.binary_path) {
            return Err(PortError::Failed(format!(
                "refusing to stop unrelated process {pid} as the configured core"
            )));
        }

        process::kill_process(pid)?;
        match Self::wait_for_exit(pid).await {
            ProcessExitObservation::Exited => {
                self.remove_pid_file_for(pid).await?;
                self.clear_owned_pid(pid);
                Ok(())
            }
            ProcessExitObservation::TimedOut => Err(PortError::Failed(format!(
                "core process {pid} did not exit after the stop deadline"
            ))),
        }
    }

    async fn reconcile_stale_pid_record(
        &self,
    ) -> result::Result<PidRecordReconciliation, PortError> {
        let pid = match process::read_pid_file(&self.pid_file).await {
            Ok(pid) => pid,
            Err(PortError::NotFound(_)) => {
                return Ok(PidRecordReconciliation::NoStaleRecord);
            }
            Err(error) => return Err(error),
        };
        let owned_pid = *self
            .owned_pid
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if owned_pid == Some(pid) {
            return Ok(PidRecordReconciliation::NoStaleRecord);
        }

        if process::is_process_alive(pid) {
            if !process::process_matches_binary(pid, &self.binary_path) {
                return Err(PortError::Failed(format!(
                    "refusing to stop unrelated process {pid} recorded in mihomo.pid"
                )));
            }
            process::kill_process(pid)?;
            log::warn!("reclaimed orphaned mihomo process {pid}");
            if Self::wait_for_exit(pid).await == ProcessExitObservation::TimedOut {
                return Err(PortError::Failed(format!(
                    "orphaned mihomo process {pid} did not exit after cleanup"
                )));
            }
            self.remove_pid_file_for(pid).await?;
            return Ok(PidRecordReconciliation::StoppedOrphan { pid });
        }
        self.remove_pid_file_for(pid).await?;
        Ok(PidRecordReconciliation::RemovedDeadRecord { pid })
    }
}

#[cfg(feature = "process")]
#[async_trait]
impl CoreProcess for ProcessCoreController {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").ok()?,
            CoreInstanceId::new("desktop-active").ok()?,
            None,
        ))
    }

    async fn prepare_start(
        &self,
        context: &CoreProcessStartContext,
    ) -> result::Result<CoreProcessStartAdmission, PortError> {
        let identity = self
            .core_identity()
            .ok_or_else(|| PortError::Failed("desktop Core identity is unavailable".into()))?;
        if !context.identity().same_instance(&identity) {
            return Err(PortError::Failed(
                "Core start context targets a different desktop process".into(),
            ));
        }
        self.reconcile_stale_pid_record().await?;
        if self.read_running_pid().await?.is_some() {
            return Err(PortError::Failed("Service is already running".to_string()));
        }
        Ok(CoreProcessStartAdmission::LaunchNewProcess)
    }

    async fn start(&self, context: CoreProcessStartContext) -> result::Result<(), PortError> {
        match self.prepare_start(&context).await? {
            CoreProcessStartAdmission::LaunchNewProcess => {}
            CoreProcessStartAdmission::ReuseExactOwnedProcess => return Ok(()),
        }

        let spawned =
            process::spawn_daemon(&self.binary_path, &self.config_path, &self.log_dir).await?;
        *self
            .owned_pid
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(spawned.pid);
        if let Err(error) = process::write_pid_file(&self.pid_file, spawned.pid).await {
            let _ = process::kill_process(spawned.pid);
            *self
                .owned_pid
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = None;
            return Err(error);
        }

        // CORE-002 (Windows): keep the kill-on-close Job Object handle alive
        // for the controller's lifetime. Replacing a previous handle on restart
        // leaves the old (empty) job handle open; stale handles are reclaimed
        // when this process exits.
        #[cfg(windows)]
        if let Some(job) = spawned.job {
            let mut slot = self.job.lock().unwrap_or_else(PoisonError::into_inner);
            *slot = Some(job);
        }

        sleep(Duration::from_millis(500)).await;

        if !process::is_process_alive(spawned.pid) {
            process::remove_pid_file(&self.pid_file).await?;
            *self
                .owned_pid
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = None;
            return Err(PortError::Failed("Service failed to start".to_string()));
        }

        Ok(())
    }

    async fn stop(&self) -> result::Result<(), PortError> {
        match self.pid_record_observation().await? {
            PidRecordObservation::Missing => Ok(()),
            PidRecordObservation::Conflict { recorded, owned } => Err(PortError::Failed(format!(
                "PID record {recorded} conflicts with owned core process {owned}"
            ))),
            PidRecordObservation::OwnedWithoutRecord(pid) => self.stop_owned_process(pid).await,
            PidRecordObservation::Recorded(pid) => {
                let owned = *self
                    .owned_pid
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                if owned == Some(pid) {
                    self.stop_owned_process(pid).await
                } else {
                    self.reconcile_stale_pid_record().await.map(|_| ())
                }
            }
        }
    }

    async fn status(&self) -> result::Result<CoreLifecycle, PortError> {
        if self.read_running_pid().await?.is_some() {
            Ok(CoreLifecycle::Running)
        } else {
            Ok(CoreLifecycle::Stopped)
        }
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }

    async fn pid(&self) -> Option<u32> {
        match process::read_pid_file(&self.pid_file).await {
            Ok(pid) => Some(pid),
            Err(err) => {
                log::warn!("failed to read pid file: {err}");
                None
            }
        }
    }
}

#[cfg(all(test, feature = "http"))]
#[path = "readiness_tests.rs"]
mod readiness_tests;

#[cfg(all(test, feature = "process", target_os = "linux"))]
#[path = "process/controller_tests.rs"]
mod controller_tests;
