//! Per-OS process mechanics owned by the desktop engine driver.
//!
//! This module only knows how to spawn, identify, signal and reap an engine
//! binary and how to persist its PID record. Ownership policy (which PID this
//! controller instance owns, when a record is stale, and what counts as an
//! exact terminal readback) lives in [`crate::driver::ProcessCoreController`].

pub mod command;

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
use std::result;
use sysinfo::{Pid, ProcessStatus, ProcessesToUpdate, System};
use tokio::fs;
use tokio::time::{Duration, sleep};

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

use lurker_core_ports::error::PortError;

type Result<T> = result::Result<T, PortError>;

/// A successfully spawned core process.
pub(crate) struct SpawnedCore {
    pub pid: u32,
    // Windows only: kill-on-close Job Object the child was adopted into.
    // Must be kept alive (or deliberately left open) for the protection to
    // hold; see `JobObjectHandle`.
    #[cfg(windows)]
    pub job: Option<JobObjectHandle>,
}

/// How long a stop waits for kernel-level process termination.
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const EXIT_POLL_ATTEMPTS: usize = 20;

/// Poll the kernel until the exact PID is no longer a live process.
pub(crate) async fn wait_for_process_exit(pid: u32) -> bool {
    for _ in 0..EXIT_POLL_ATTEMPTS {
        if !is_process_alive(pid) {
            return true;
        }
        sleep(EXIT_POLL_INTERVAL).await;
    }
    !is_process_alive(pid)
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
/// Gate: `prctl`/`PR_SET_PDEATHSIG` exist on Linux only, so this is
/// Linux-specific; macOS/BSD would fail to compile otherwise.
#[cfg(target_os = "linux")]
pub(crate) fn parent_death_signal(parent_pid: u32) -> impl FnMut() -> io::Result<()> + Send + Sync {
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
/// kill-on-close, and reaps the engine. Handles superseded by a restart are
/// left open on purpose; an empty job is harmless and reclaimed at process
/// exit.
#[cfg(windows)]
pub(crate) struct JobObjectHandle(HANDLE);

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

/// Spawns the engine binary as a detached daemon with its console output
/// redirected into `log_dir/logs`.
pub(crate) async fn spawn_daemon(
    binary: &Path,
    config: &Path,
    log_dir: &Path,
) -> Result<SpawnedCore> {
    if !binary.exists() {
        return Err(PortError::NotFound(format!(
            "Binary not found: {}",
            binary.display()
        )));
    }

    if !config.exists() {
        return Err(PortError::NotFound(format!(
            "Config not found: {}",
            config.display()
        )));
    }

    let log_path = prepare_log_file(log_dir).await?;
    let stdout = open_log_file(&log_path)?;
    let stderr = open_log_file(&log_path)?;
    log::info!("mihomo log file: {}", log_path.display());

    let config_dir = config
        .parent()
        .ok_or_else(|| PortError::Failed("Config file has no parent directory".to_string()))?;

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
    // sends SIGTERM to the engine if this process dies. Our pid is recorded
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
        .map_err(|error| PortError::Io(format!("Failed to spawn process: {error}")))?;

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
        return Err(PortError::Failed(format!(
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

async fn prepare_log_file(log_dir: &Path) -> Result<PathBuf> {
    let log_dir = log_dir.join("logs");
    fs::create_dir_all(&log_dir).await?;
    Ok(log_dir.join("mihomo.log"))
}

fn open_log_file(path: &Path) -> Result<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| PortError::Io(format!("Failed to open log file {}: {e}", path.display())))
}

pub(crate) fn kill_process(pid: u32) -> Result<()> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let pid = Pid::from_u32(pid);
    if let Some(process) = system.process(pid)
        && !process.kill()
    {
        return Err(PortError::Failed(format!("Failed to kill process {}", pid)));
    }

    Ok(())
}

pub(crate) fn is_process_alive(pid: u32) -> bool {
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
pub(crate) fn process_matches_binary(pid: u32, binary: &Path) -> bool {
    let Ok(expected) = canonicalize(binary) else {
        return false;
    };
    let Ok(actual) = canonicalize(format!("/proc/{pid}/exe")) else {
        return false;
    };
    actual == expected
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn process_matches_binary(pid: u32, binary: &Path) -> bool {
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

pub(crate) async fn read_pid_file(path: &Path) -> Result<u32> {
    if !path.exists() {
        return Err(PortError::NotFound("PID file not found".to_string()));
    }

    let content = fs::read_to_string(path).await?;
    let pid = content
        .trim()
        .parse::<u32>()
        .map_err(|error| PortError::Failed(format!("Invalid PID in file: {error}")))?;

    Ok(pid)
}

pub(crate) async fn write_pid_file(path: &Path, pid: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(path, pid.to_string()).await?;
    Ok(())
}

pub(crate) async fn remove_pid_file(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).await?;
    }
    Ok(())
}
