use crate::paths::get_home_dir;
use async_trait::async_trait;
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::{
    CoreProcess, CoreProcessStartAdmission, CoreProcessStartContext,
};
use infiltrator_ports::data_dir::DataDirProvider;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::{MihomoError, Result};
use std::path::PathBuf;
use std::result;
use std::sync::{Mutex, PoisonError};
use tokio::task::spawn_blocking;
use tokio::time::{Duration, sleep};
pub struct ProcessCoreController {
    binary_path: PathBuf,
    config_path: PathBuf,
    pid_file: PathBuf,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PidRecordObservation {
    Missing,
    Recorded(u32),
    OwnedWithoutRecord(u32),
    Conflict { recorded: u32, owned: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessExitObservation {
    Exited,
    TimedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PidRecordReconciliation {
    NoStaleRecord,
    RemovedDeadRecord { pid: u32 },
    StoppedOrphan { pid: u32 },
}

async fn wait_for_process_exit(pid: u32) -> ProcessExitObservation {
    for _ in 0..20 {
        if !process::is_process_alive(pid) {
            return ProcessExitObservation::Exited;
        }
        sleep(Duration::from_millis(50)).await;
    }
    if process::is_process_alive(pid) {
        ProcessExitObservation::TimedOut
    } else {
        ProcessExitObservation::Exited
    }
}

impl ProcessCoreController {
    pub fn new(binary_path: PathBuf, config_path: PathBuf) -> Self {
        let home = get_home_dir().unwrap_or_else(|_| PathBuf::from("."));
        let pid_file = home.join("mihomo.pid");

        Self {
            binary_path,
            config_path,
            pid_file,
            owned_pid: Mutex::new(None),
            #[cfg(windows)]
            job: Mutex::new(None),
        }
    }

    pub fn with_home(binary_path: PathBuf, config_path: PathBuf, home: PathBuf) -> Self {
        let pid_file = home.join("mihomo.pid");

        Self {
            binary_path,
            config_path,
            pid_file,
            owned_pid: Mutex::new(None),
            #[cfg(windows)]
            job: Mutex::new(None),
        }
    }

    pub fn with_pid_file(binary_path: PathBuf, config_path: PathBuf, pid_file: PathBuf) -> Self {
        Self {
            binary_path,
            config_path,
            pid_file,
            owned_pid: Mutex::new(None),
            #[cfg(windows)]
            job: Mutex::new(None),
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
            Err(MihomoError::NotFound(_)) => match owned {
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
                return Err(MihomoError::Service(format!(
                    "PID record {recorded} conflicts with owned core process {owned}"
                )));
            }
        };

        if !process::is_process_alive(pid) {
            self.remove_pid_file_for(pid).await?;
            return Ok(None);
        }
        if !process::process_matches_binary(pid, &self.binary_path) {
            return Err(MihomoError::Service(format!(
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
            Ok(recorded) => Err(MihomoError::Service(format!(
                "refusing to remove PID record {recorded} while closing process {expected_pid}"
            ))),
            Err(MihomoError::NotFound(_)) => Ok(()),
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
            self.remove_pid_file_for(pid)
                .await
                .map_err(map_port_error)?;
            self.clear_owned_pid(pid);
            return Ok(());
        }
        if !process::process_matches_binary(pid, &self.binary_path) {
            return Err(PortError::Failed(format!(
                "refusing to stop unrelated process {pid} as the configured core"
            )));
        }

        process::kill_process(pid).map_err(map_port_error)?;
        match wait_for_process_exit(pid).await {
            ProcessExitObservation::Exited => {
                self.remove_pid_file_for(pid)
                    .await
                    .map_err(map_port_error)?;
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
            Err(MihomoError::NotFound(_)) => {
                return Ok(PidRecordReconciliation::NoStaleRecord);
            }
            Err(error) => return Err(map_port_error(error)),
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
            process::kill_process(pid).map_err(map_port_error)?;
            log::warn!("reclaimed orphaned mihomo process {pid}");
            if wait_for_process_exit(pid).await == ProcessExitObservation::TimedOut {
                return Err(PortError::Failed(format!(
                    "orphaned mihomo process {pid} did not exit after cleanup"
                )));
            }
            self.remove_pid_file_for(pid)
                .await
                .map_err(map_port_error)?;
            return Ok(PidRecordReconciliation::StoppedOrphan { pid });
        }
        self.remove_pid_file_for(pid)
            .await
            .map_err(map_port_error)?;
        Ok(PidRecordReconciliation::RemovedDeadRecord { pid })
    }
}

fn map_port_error(error: MihomoError) -> PortError {
    match error {
        MihomoError::Io(error) => PortError::Io(error.to_string()),
        MihomoError::Http(error) => PortError::Network(error.to_string()),
        MihomoError::WebSocket(error) => PortError::Network(error.to_string()),
        MihomoError::NotFound(message) => PortError::NotFound(message),
        MihomoError::Config(message) | MihomoError::YamlEmit(message) => PortError::Failed(message),
        other => PortError::Failed(other.to_string()),
    }
}

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
        if self
            .read_running_pid()
            .await
            .map_err(map_port_error)?
            .is_some()
        {
            return Err(PortError::Failed("Service is already running".to_string()));
        }
        Ok(CoreProcessStartAdmission::LaunchNewProcess)
    }

    async fn start(&self, context: CoreProcessStartContext) -> result::Result<(), PortError> {
        match self.prepare_start(&context).await? {
            CoreProcessStartAdmission::LaunchNewProcess => {}
            CoreProcessStartAdmission::ReuseExactOwnedProcess => return Ok(()),
        }

        let spawned = process::spawn_daemon(&self.binary_path, &self.config_path)
            .await
            .map_err(map_port_error)?;
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
            return Err(map_port_error(error));
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
            process::remove_pid_file(&self.pid_file)
                .await
                .map_err(map_port_error)?;
            *self
                .owned_pid
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = None;
            return Err(PortError::Failed("Service failed to start".to_string()));
        }

        Ok(())
    }

    async fn stop(&self) -> result::Result<(), PortError> {
        match self
            .pid_record_observation()
            .await
            .map_err(map_port_error)?
        {
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
        if self
            .read_running_pid()
            .await
            .map_err(map_port_error)?
            .is_some()
        {
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

pub struct KeyringCredentialStore;

impl Default for KeyringCredentialStore {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl SecureStore for KeyringCredentialStore {
    async fn get(&self, service: &str, key: &str) -> result::Result<Option<String>, PortError> {
        let service = service.to_string();
        let key = key.to_string();
        spawn_blocking(move || -> result::Result<Option<String>, PortError> {
            let entry = match keyring::Entry::new(&service, &key) {
                Ok(entry) => entry,
                Err(err) => {
                    log::warn!("keyring init failed: {err}");
                    return Err(PortError::Failed(format!("Keyring init failed: {err}")));
                }
            };
            match entry.get_password() {
                Ok(value) => Ok(Some(value)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(err) => {
                    log::warn!("keyring get failed: {err}");
                    Err(PortError::Failed(format!("Keyring get failed: {err}")))
                }
            }
        })
        .await
        .map_err(|e| PortError::Failed(format!("Keyring task failed: {e}")))?
    }

    async fn set(&self, service: &str, key: &str, value: &str) -> result::Result<(), PortError> {
        let service = service.to_string();
        let key = key.to_string();
        let value = value.to_string();
        spawn_blocking(move || -> result::Result<(), PortError> {
            let entry = keyring::Entry::new(&service, &key)
                .map_err(|err| PortError::Failed(format!("Keyring init failed: {err}")))?;
            entry
                .set_password(&value)
                .map_err(|err| PortError::Failed(format!("Keyring set failed: {err}")))?;
            Ok(())
        })
        .await
        .map_err(|e| PortError::Failed(format!("Keyring task failed: {e}")))?
    }

    async fn delete(&self, service: &str, key: &str) -> result::Result<(), PortError> {
        let service = service.to_string();
        let key = key.to_string();
        spawn_blocking(move || -> result::Result<(), PortError> {
            let entry = keyring::Entry::new(&service, &key)
                .map_err(|err| PortError::Failed(format!("Keyring init failed: {err}")))?;
            entry
                .delete_credential()
                .map_err(|err| PortError::Failed(format!("Keyring delete failed: {err}")))?;
            Ok(())
        })
        .await
        .map_err(|e| PortError::Failed(format!("Keyring task failed: {e}")))?
    }
}

pub struct DesktopDataDirProvider;

impl Default for DesktopDataDirProvider {
    fn default() -> Self {
        Self
    }
}

impl DataDirProvider for DesktopDataDirProvider {
    fn data_dir(&self) -> Option<PathBuf> {
        get_home_dir().ok()
    }
}

mod process {
    use mihomo_api::error::{MihomoError, Result};
    #[cfg(windows)]
    use std::ffi::c_void;
    use std::fs::{File, OpenOptions, canonicalize};
    use std::io;
    #[cfg(windows)]
    use std::mem::size_of;
    use std::path::{Path, PathBuf};
    #[cfg(target_os = "linux")]
    use std::process::id;
    use std::process::{Command, Stdio};
    #[cfg(windows)]
    use std::ptr::null;
    use sysinfo::{Pid, ProcessStatus, ProcessesToUpdate, System};
    use tokio::fs;

    #[cfg(target_os = "linux")]
    use std::os::unix::process::CommandExt;
    #[cfg(windows)]
    use std::os::windows::io::AsRawHandle;
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    #[cfg(windows)]
    use std::process::Child;
    #[cfg(windows)]
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    #[cfg(windows)]
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    #[cfg(windows)]
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    use crate::paths::get_home_dir;

    /// A successfully spawned core process.
    pub struct SpawnedCore {
        pub pid: u32,
        // Windows only: kill-on-close Job Object the child was adopted into.
        // Must be kept alive (or deliberately left open) for the protection to
        // hold; see `JobObjectHandle`.
        #[cfg(windows)]
        pub job: Option<JobObjectHandle>,
    }

    /// Builds the closure handed to `CommandExt::pre_exec` that arms the
    /// kernel-side parent-death watchdog (CORE-002).
    ///
    /// The closure runs inside the forked child before `exec`:
    /// 1. `prctl(PR_SET_PDEATHSIG, SIGTERM)` asks the kernel to SIGTERM this
    ///    process the moment its parent dies.
    /// 2. Because the parent could die between `fork()` and `prctl()` (the
    ///    signal is only armed after prctl), it re-checks that the current
    ///    parent pid still equals `parent_pid` recorded just before spawn; on
    ///    mismatch the child was already orphaned/re-parented and must exit
    ///    immediately via `libc::_exit` (async-signal-safe, no atexit/stdio
    ///    flush of duplicated buffers).
    ///
    /// Gate: `prctl`/`PR_SET_PDEATHSIG` exist on Linux only (desktop.rs is not
    /// compiled for Android), so this is Linux-specific despite the task's
    /// "unix" wording; macOS/BSD would fail to compile otherwise.
    #[cfg(target_os = "linux")]
    pub(crate) fn parent_death_signal(
        parent_pid: u32,
    ) -> impl FnMut() -> io::Result<()> + Send + Sync {
        move || {
            // SAFETY: raw libc calls only, executed in the forked child before
            // exec. Every call here (prctl, getppid, _exit) is
            // async-signal-safe, and no Rust allocator/std facilities are used.
            unsafe {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0) != 0 {
                    libc::_exit(1);
                }
                if libc::getppid() as u32 != parent_pid {
                    libc::_exit(1);
                }
            }
            // std's `pre_exec` contract: the closure must report `Ok` to let
            // exec proceed; every failure path above never returns.
            Ok(())
        }
    }

    /// Windows Job Object wrapper (CORE-002). The wrapped kernel handle is
    /// armed with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: the kernel terminates
    /// every process in the job when the last handle to the job is closed.
    ///
    /// Deliberately NO `Drop`: dropping this wrapper only drops the Rust-side
    /// pointer copy, the OS handle stays open. That is the point — the handle
    /// must remain open until the GUI process itself dies, at which moment the
    /// kernel closes all of its handles (including "leaked" ones), triggers
    /// kill-on-close, and reaps mihomo. Handles superseded by a restart are
    /// left open on purpose; an empty job is harmless and reclaimed at process
    /// exit.
    #[cfg(windows)]
    pub struct JobObjectHandle(HANDLE);

    // SAFETY: HANDLE is a shared kernel handle; job objects may be used from
    // any thread and the wrapper neither reads nor mutates Rust state, so
    // moving/sharing it across threads is sound.
    #[cfg(windows)]
    unsafe impl Send for JobObjectHandle {}
    #[cfg(windows)]
    unsafe impl Sync for JobObjectHandle {}

    #[cfg(windows)]
    impl JobObjectHandle {
        /// Creates a Job Object configured to kill its processes when the last
        /// handle to it closes. Returns `None` (with a warning) if setup fails;
        /// the caller then simply loses the extra safety net.
        fn create() -> Option<Self> {
            // SAFETY: plain kernel32 calls with valid/null arguments; `job` is
            // closed on the failure path to avoid leaking the raw handle.
            unsafe {
                let job = CreateJobObjectW(null(), null());
                if job.is_null() {
                    log::warn!("CreateJobObjectW failed: {}", io::Error::last_os_error());
                    return None;
                }

                let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const c_void,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) == 0
                {
                    log::warn!(
                        "SetInformationJobObject failed: {}",
                        io::Error::last_os_error()
                    );
                    CloseHandle(job);
                    return None;
                }

                Some(Self(job))
            }
        }

        /// Adopts `child` into this job. Failure is logged but non-fatal: the
        /// most likely cause is that the child already exited, which
        /// `spawn_daemon` reports separately.
        fn assign(&self, child: &Child) {
            // SAFETY: both handles are valid kernel handles owned by this
            // process; the child's raw handle stays valid until `child` is
            // dropped, which does not happen during this call.
            let assigned = unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle()) };
            if assigned == 0 {
                log::warn!(
                    "AssignProcessToJobObject failed: {}",
                    io::Error::last_os_error()
                );
            }
        }
    }

    pub async fn spawn_daemon(binary: &Path, config: &Path) -> Result<SpawnedCore> {
        if !binary.exists() {
            return Err(MihomoError::NotFound(format!(
                "Binary not found: {}",
                binary.display()
            )));
        }

        if !config.exists() {
            return Err(MihomoError::NotFound(format!(
                "Config not found: {}",
                config.display()
            )));
        }

        let log_path = prepare_log_file().await?;
        let stdout = open_log_file(&log_path)?;
        let stderr = open_log_file(&log_path)?;
        log::info!("mihomo log file: {}", log_path.display());

        let config_dir = config.parent().ok_or_else(|| {
            MihomoError::Config("Config file has no parent directory".to_string())
        })?;

        let mut command = Command::new(binary);
        command
            .arg("-d")
            .arg(config_dir)
            .arg("-f")
            .arg(config)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));

        // CORE-002 (Linux): arm PR_SET_PDEATHSIG in the child so the kernel
        // sends SIGTERM to mihomo if this process dies. Our pid is recorded
        // before spawn for the fork/prctl race guard inside the closure.
        #[cfg(target_os = "linux")]
        {
            let parent_pid = id();
            // SAFETY: the closure only runs async-signal-safe libc calls in
            // the forked child before exec; see `parent_death_signal`.
            unsafe {
                command.pre_exec(parent_death_signal(parent_pid));
            }
        }

        #[cfg(windows)]
        {
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command
            .spawn()
            .map_err(|e| MihomoError::Service(format!("Failed to spawn process: {}", e)))?;

        // CORE-002 (Windows): create the kill-on-close job and adopt the child
        // right after spawn, before any early-exit reporting, so even a
        // short-lived child is covered while it lives.
        #[cfg(windows)]
        let job = {
            let job = JobObjectHandle::create();
            if let Some(handle) = &job {
                handle.assign(&child);
            }
            job
        };

        if let Ok(Some(status)) = child.try_wait() {
            return Err(MihomoError::Service(format!(
                "Process exited immediately with status: {}",
                status
            )));
        }

        Ok(SpawnedCore {
            pid: child.id(),
            #[cfg(windows)]
            job,
        })
    }

    async fn prepare_log_file() -> Result<PathBuf> {
        let home = get_home_dir()?;
        let log_dir = home.join("logs");
        fs::create_dir_all(&log_dir).await?;
        Ok(log_dir.join("mihomo.log"))
    }

    fn open_log_file(path: &Path) -> Result<File> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| {
                MihomoError::Service(format!("Failed to open log file {}: {}", path.display(), e))
            })
    }

    pub fn kill_process(pid: u32) -> Result<()> {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);

        let pid = Pid::from_u32(pid);
        if let Some(process) = system.process(pid)
            && !process.kill()
        {
            return Err(MihomoError::Service(format!(
                "Failed to kill process {}",
                pid
            )));
        }

        Ok(())
    }

    pub fn is_process_alive(pid: u32) -> bool {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);
        system.process(Pid::from_u32(pid)).is_some_and(|process| {
            !matches!(
                process.status(),
                ProcessStatus::Zombie | ProcessStatus::Dead
            )
        })
    }

    /// Read the exact process identity; absence or mismatch always fails closed.
    #[cfg(target_os = "linux")]
    pub fn process_matches_binary(pid: u32, binary: &Path) -> bool {
        let Ok(expected) = canonicalize(binary) else {
            return false;
        };
        let Ok(actual) = canonicalize(format!("/proc/{pid}/exe")) else {
            return false;
        };
        actual == expected
    }

    #[cfg(not(target_os = "linux"))]
    pub fn process_matches_binary(pid: u32, binary: &Path) -> bool {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
        let Some(process) = system.process(Pid::from_u32(pid)) else {
            return false;
        };
        let Some(actual) = process.exe() else {
            return false;
        };
        let Ok(expected) = canonicalize(binary) else {
            return false;
        };
        let Ok(actual) = canonicalize(actual) else {
            return false;
        };
        actual == expected
    }

    pub async fn read_pid_file(path: &Path) -> Result<u32> {
        if !path.exists() {
            return Err(MihomoError::NotFound("PID file not found".to_string()));
        }

        let content = fs::read_to_string(path).await?;
        let pid = content
            .trim()
            .parse::<u32>()
            .map_err(|e| MihomoError::Service(format!("Invalid PID in file: {}", e)))?;

        Ok(pid)
    }

    pub async fn write_pid_file(path: &Path, pid: u32) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, pid.to_string()).await?;
        Ok(())
    }

    pub async fn remove_pid_file(path: &Path) -> Result<()> {
        if path.exists() {
            fs::remove_file(path).await?;
        }
        Ok(())
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "desktop_tests.rs"]
mod tests;
