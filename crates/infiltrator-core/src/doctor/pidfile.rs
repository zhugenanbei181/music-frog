//! Pid-file state inspection for doctor checks.
//!
//! The pid file written by `ProcessCoreController` records only a bare pid
//! (no process start time), so pid reuse cannot be ruled out by comparing
//! start timestamps. Instead, a recorded pid only counts as a live core when
//! the OS process table shows a process whose name or executable matches the
//! core process hint; a recycled pid owned by any other program is reported
//! as a foreign process and the record treated as stale.

use std::path::Path;

use sysinfo::{Pid, ProcessesToUpdate, System};

/// Substring matched (case-insensitively) against a process name or
/// executable path to decide whether a recorded pid still belongs to the
/// mihomo core.
pub(super) const CORE_PROCESS_HINT: &str = "mihomo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProcessState {
    /// A process with this pid exists and matches the core hint.
    AliveCore,
    /// A process with this pid exists but looks like a different program
    /// (the pid was recycled after the core exited).
    AliveForeign,
    /// No process with this pid exists.
    Gone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PidFileState {
    /// No pid file at the expected location (service stopped, clean state).
    Absent,
    /// The pid file exists but cannot be read.
    Unreadable(String),
    /// The pid file exists but its content is not a bare pid.
    Malformed(String),
    Recorded {
        pid: u32,
        process: ProcessState,
    },
}

pub(super) fn inspect_process(pid: u32, core_hint: &str) -> ProcessState {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let Some(process) = system.process(Pid::from_u32(pid)) else {
        return ProcessState::Gone;
    };
    if process_matches_hint(process, core_hint) {
        ProcessState::AliveCore
    } else {
        ProcessState::AliveForeign
    }
}

fn process_matches_hint(process: &sysinfo::Process, core_hint: &str) -> bool {
    let hint = core_hint.to_lowercase();
    if process
        .name()
        .to_string_lossy()
        .to_lowercase()
        .contains(&hint)
    {
        return true;
    }
    process.exe().is_some_and(|exe| {
        exe.file_name()
            .is_some_and(|name| name.to_string_lossy().to_lowercase().contains(&hint))
    })
}

pub(super) async fn read_pid_state(path: &Path) -> PidFileState {
    if !path.exists() {
        return PidFileState::Absent;
    }
    let content = match tokio::fs::read_to_string(path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return PidFileState::Absent,
        Err(err) => return PidFileState::Unreadable(err.to_string()),
    };
    let trimmed = content.trim();
    match trimmed.parse::<u32>() {
        Ok(pid) => PidFileState::Recorded {
            pid,
            process: inspect_process(pid, CORE_PROCESS_HINT),
        },
        Err(_) => PidFileState::Malformed(trimmed.to_string()),
    }
}

/// Whether a live core process is recorded behind `path`. Any other state
/// (no file, malformed, dead pid, foreign pid) counts as "not running".
pub(super) async fn service_running(path: &Path) -> bool {
    matches!(
        read_pid_state(path).await,
        PidFileState::Recorded {
            process: ProcessState::AliveCore,
            ..
        }
    )
}

/// Remove the pid file only when it is provably stale: malformed content, a
/// dead pid, or a pid recycled by a foreign process. An unreadable file or a
/// file backed by a running core is never touched.
pub(super) async fn remove_stale_pid_file(path: &Path) -> anyhow::Result<bool> {
    let stale = match read_pid_state(path).await {
        PidFileState::Absent | PidFileState::Unreadable(_) => false,
        PidFileState::Malformed(_) => true,
        PidFileState::Recorded {
            process: ProcessState::Gone | ProcessState::AliveForeign,
            ..
        } => true,
        PidFileState::Recorded {
            process: ProcessState::AliveCore,
            ..
        } => false,
    };
    if !stale {
        return Ok(false);
    }
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err.into()),
    }
}
