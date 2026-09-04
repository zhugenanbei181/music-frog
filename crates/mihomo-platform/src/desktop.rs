use async_trait::async_trait;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_ports::data_dir::DataDirProvider;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use std::path::PathBuf;

#[cfg(windows)]
use std::sync::Mutex;

use mihomo_api::error::{MihomoError, Result};
use tokio::time::Duration;

use crate::paths::get_home_dir;
pub struct ProcessCoreController {
    binary_path: PathBuf,
    config_path: PathBuf,
    pid_file: PathBuf,
    // CORE-002: Windows-only guard. Holds the kill-on-close Job Object handle
    // for the currently spawned core. The handle is deliberately never closed
    // while this process lives (see `process::JobObjectHandle`), so when the
    // GUI process dies the kernel closes the last job handle and terminates
    // the core. On Linux/Unix the equivalent guarantee comes from
    // PR_SET_PDEATHSIG armed inside the child at spawn time.
    #[cfg(windows)]
    job: Mutex<Option<process::JobObjectHandle>>,
}

impl ProcessCoreController {
    pub fn new(binary_path: PathBuf, config_path: PathBuf) -> Self {
        let home = get_home_dir().unwrap_or_else(|_| PathBuf::from("."));
        let pid_file = home.join("mihomo.pid");

        Self {
            binary_path,
            config_path,
            pid_file,
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
            #[cfg(windows)]
            job: Mutex::new(None),
        }
    }

    pub fn with_pid_file(binary_path: PathBuf, config_path: PathBuf, pid_file: PathBuf) -> Self {
        Self {
            binary_path,
            config_path,
            pid_file,
            #[cfg(windows)]
            job: Mutex::new(None),
        }
    }

    async fn read_running_pid(&self) -> Result<Option<u32>> {
        match process::read_pid_file(&self.pid_file).await {
            Ok(pid) => {
                if process::is_process_alive(pid) {
                    Ok(Some(pid))
                } else {
                    process::remove_pid_file(&self.pid_file).await?;
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
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
    async fn start(&self) -> std::result::Result<(), PortError> {
        if self
            .read_running_pid()
            .await
            .map_err(map_port_error)?
            .is_some()
        {
            return Err(PortError::Failed("Service is already running".to_string()));
        }

        let spawned = process::spawn_daemon(&self.binary_path, &self.config_path)
            .await
            .map_err(map_port_error)?;
        process::write_pid_file(&self.pid_file, spawned.pid)
            .await
            .map_err(map_port_error)?;

        // CORE-002 (Windows): keep the kill-on-close Job Object handle alive
        // for the controller's lifetime. Replacing a previous handle on restart
        // leaves the old (empty) job handle open; stale handles are reclaimed
        // when this process exits.
        #[cfg(windows)]
        if let Some(job) = spawned.job {
            let mut slot = self
                .job
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *slot = Some(job);
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        if !process::is_process_alive(spawned.pid) {
            process::remove_pid_file(&self.pid_file)
                .await
                .map_err(map_port_error)?;
            return Err(PortError::Failed("Service failed to start".to_string()));
        }

        Ok(())
    }

    async fn stop(&self) -> std::result::Result<(), PortError> {
        let pid = process::read_pid_file(&self.pid_file)
            .await
            .map_err(map_port_error)?;

        if !process::is_process_alive(pid) {
            process::remove_pid_file(&self.pid_file)
                .await
                .map_err(map_port_error)?;
            return Err(PortError::Failed("Service is not running".to_string()));
        }

        process::kill_process(pid).map_err(map_port_error)?;
        process::remove_pid_file(&self.pid_file)
            .await
            .map_err(map_port_error)?;

        Ok(())
    }

    async fn status(&self) -> std::result::Result<CoreLifecycle, PortError> {
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
    async fn get(
        &self,
        service: &str,
        key: &str,
    ) -> std::result::Result<Option<String>, PortError> {
        let service = service.to_string();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || -> std::result::Result<Option<String>, PortError> {
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

    async fn set(
        &self,
        service: &str,
        key: &str,
        value: &str,
    ) -> std::result::Result<(), PortError> {
        let service = service.to_string();
        let key = key.to_string();
        let value = value.to_string();
        tokio::task::spawn_blocking(move || -> std::result::Result<(), PortError> {
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

    async fn delete(&self, service: &str, key: &str) -> std::result::Result<(), PortError> {
        let service = service.to_string();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || -> std::result::Result<(), PortError> {
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
    use std::fs::OpenOptions;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use sysinfo::{Pid, ProcessesToUpdate, System};
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
    ) -> impl FnMut() -> std::io::Result<()> + Send + Sync {
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
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    log::warn!(
                        "CreateJobObjectW failed: {}",
                        std::io::Error::last_os_error()
                    );
                    return None;
                }

                let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) == 0
                {
                    log::warn!(
                        "SetInformationJobObject failed: {}",
                        std::io::Error::last_os_error()
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
                    std::io::Error::last_os_error()
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
            let parent_pid = std::process::id();
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

    fn open_log_file(path: &Path) -> Result<std::fs::File> {
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
        system.process(Pid::from_u32(pid)).is_some()
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
mod tests {
    use super::process;
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    /// Happy path: with a correct parent pid, the pre_exec closure arms
    /// PR_SET_PDEATHSIG successfully and lets exec proceed, so the spawned
    /// `sh` runs to completion with exit code 0.
    #[test]
    fn parent_death_signal_allows_normal_spawn() {
        let parent = std::process::id();
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("exit 0");
        // SAFETY: closure only runs async-signal-safe libc calls in the child.
        unsafe {
            cmd.pre_exec(process::parent_death_signal(parent));
        }
        let status = cmd.status().expect("failed to spawn sh");
        assert!(status.success(), "expected exit 0, got: {status}");
    }

    /// Race-guard path: with a parent pid that can never match (u32::MAX),
    /// the closure must `_exit(1)` inside the child before exec, so `sh` never
    /// runs and the child reports exit code 1.
    #[test]
    fn parent_death_signal_exits_when_parent_pid_mismatches() {
        let impossible_parent = u32::MAX;
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("exit 0");
        // SAFETY: closure only runs async-signal-safe libc calls in the child.
        unsafe {
            cmd.pre_exec(process::parent_death_signal(impossible_parent));
        }
        let status = cmd.status().expect("failed to spawn sh");
        assert_eq!(status.code(), Some(1), "expected child _exit(1): {status}");
    }

    /// End-to-end: `spawn_daemon` (the real spawn path with the pre_exec
    /// wiring) starts a long-running fake core, reports a live pid, and
    /// `kill_process` still cleans it up. Runs under the shared TEST_LOCK
    /// because `spawn_daemon` resolves the log dir via the global HOME
    /// override.
    #[tokio::test]
    async fn spawn_daemon_runs_fake_core_with_death_signal() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = crate::TEST_LOCK.lock().await;
        let dir = tempfile::tempdir().expect("tempdir");

        // Fake core: a long-running executable so spawn_daemon's liveness
        // checks see a running process.
        let script = dir.path().join("fake-mihomo");
        std::fs::write(&script, "#!/bin/sh\nwhile :; do sleep 60; done\n")
            .expect("write fake core");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake core");

        let config = dir.path().join("config.yaml");
        std::fs::write(&config, "mixed-port: 7890\n").expect("write config");

        crate::paths::set_home_dir_override(dir.path().to_path_buf());
        let spawned = process::spawn_daemon(&script, &config).await;
        crate::paths::clear_home_dir_override();

        let spawned = spawned.expect("spawn_daemon should succeed");
        assert!(
            process::is_process_alive(spawned.pid),
            "core should be alive"
        );

        process::kill_process(spawned.pid).expect("kill fake core");

        // spawn_daemon intentionally never waits on the child (detached
        // daemon), so the SIGKILLed core lingers as a zombie that sysinfo
        // still counts as alive until this test reaps it. Reaping also proves
        // the kill landed.
        let mut reaped = false;
        for _ in 0..20 {
            // SAFETY: the fake core is a direct child of this test process.
            let rc =
                unsafe { libc::waitpid(spawned.pid as i32, std::ptr::null_mut(), libc::WNOHANG) };
            if rc == spawned.pid as i32 || rc == -1 {
                reaped = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(reaped, "fake core was not reaped after kill");
        assert!(
            !process::is_process_alive(spawned.pid),
            "killed core should no longer be listed as alive"
        );
    }

    /// The pid file round-trip used by start/stop keeps working alongside the
    /// death-signal wiring.
    #[tokio::test]
    async fn pid_file_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pid_file = dir.path().join("mihomo.pid");

        process::write_pid_file(&pid_file, 4242)
            .await
            .expect("write");
        let pid = process::read_pid_file(&pid_file).await.expect("read");
        assert_eq!(pid, 4242);
        process::remove_pid_file(&pid_file).await.expect("remove");
        assert!(process::read_pid_file(&pid_file).await.is_err());
    }

    /// Compile-time guarantee that the pre_exec closure satisfies the
    /// `FnMut() + Send + Sync + 'static` bounds required by `CommandExt::pre_exec`.
    #[test]
    fn parent_death_signal_closure_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        let closure = process::parent_death_signal(std::process::id());
        assert_send_sync(&closure);
    }
}
