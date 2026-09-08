//! Desktop system HTTP/SOCKS proxy port.

use async_trait::async_trait;
use infiltrator_contract::system_proxy::{
    SystemProxyDesiredState, SystemProxyObservation, SystemProxyRecoveryReport,
    SystemProxyRecoverySnapshot,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::system_proxy::SystemProxyPort;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io::Write};
use sysinfo::{Pid, ProcessesToUpdate, System};

static SHARED_SYSTEM_PROXY_TARGET: OnceLock<Arc<Mutex<Option<SystemProxyDesiredState>>>> =
    OnceLock::new();
static SHARED_SYSTEM_PROXY_RECOVERY: OnceLock<Arc<Mutex<SystemProxyRecoverySnapshot>>> =
    OnceLock::new();
static SYSTEM_PROXY_RECOVERY_JOURNAL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const RECOVERY_JOURNAL_FILE: &str = ".musicfrog-system-proxy-recovery.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecoveryJournal {
    owner_pid: u32,
    #[serde(default)]
    owner_start_time: u64,
    previous: SystemProxyObservation,
    desired: SystemProxyDesiredState,
}

/// Stateless desktop adapter over the OS-specific proxy implementations.
/// Windows registry, Linux GSettings/KDE/environment and macOS
/// `networksetup` remain behind `crate::proxy`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DesktopSystemProxy;

impl DesktopSystemProxy {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SystemProxyPort for DesktopSystemProxy {
    fn shared_target(&self) -> Arc<Mutex<Option<SystemProxyDesiredState>>> {
        SHARED_SYSTEM_PROXY_TARGET
            .get_or_init(|| Arc::new(Mutex::new(None)))
            .clone()
    }

    fn shared_recovery(&self) -> Arc<Mutex<SystemProxyRecoverySnapshot>> {
        SHARED_SYSTEM_PROXY_RECOVERY
            .get_or_init(|| Arc::new(Mutex::new(SystemProxyRecoverySnapshot::default())))
            .clone()
    }

    async fn arm_recovery(
        &self,
        previous: SystemProxyObservation,
        desired: SystemProxyDesiredState,
    ) -> Result<(), PortError> {
        tokio::task::spawn_blocking(move || arm_recovery_sync(previous, desired))
            .await
            .map_err(|error| {
                PortError::Io(format!("system proxy journal worker failed: {error}"))
            })?
    }

    async fn recover_orphaned(&self) -> Result<SystemProxyRecoveryReport, PortError> {
        tokio::task::spawn_blocking(recover_orphaned_sync)
            .await
            .map_err(|error| {
                PortError::Io(format!("system proxy recovery worker failed: {error}"))
            })?
    }

    async fn clear_recovery(&self) -> Result<(), PortError> {
        tokio::task::spawn_blocking(clear_recovery_sync)
            .await
            .map_err(|error| {
                PortError::Io(format!("system proxy journal worker failed: {error}"))
            })?
    }

    async fn snapshot(&self) -> Result<SystemProxyObservation, PortError> {
        tokio::task::spawn_blocking(crate::proxy::read_system_proxy_state)
            .await
            .map_err(|error| PortError::Io(format!("system proxy probe worker failed: {error}")))?
            .map(to_observation)
            .map_err(|error| PortError::Io(error.to_string()))
    }

    async fn apply(
        &self,
        endpoint: Option<String>,
        bypass: Option<String>,
    ) -> Result<(), PortError> {
        tokio::task::spawn_blocking(move || {
            if bypass.is_some() {
                crate::proxy::apply_system_proxy_with_bypass(endpoint.as_deref(), bypass.as_deref())
            } else {
                crate::proxy::apply_system_proxy(endpoint.as_deref())
            }
        })
        .await
        .map_err(|error| PortError::Io(format!("system proxy apply worker failed: {error}")))?
        .map_err(|error| PortError::Io(error.to_string()))
    }
}

impl DesktopSystemProxy {
    /// Clean-exit hook: restore the pre-ownership state if the OS still has
    /// our target, otherwise leave a third-party change untouched.
    pub fn restore_and_clear_sync() -> anyhow::Result<()> {
        let _lock = recovery_journal_lock();
        let Some(journal) = read_journal()? else {
            return Ok(());
        };
        if journal.owner_pid != std::process::id() && owner_process_is_live(&journal) {
            return Ok(());
        }
        let current = to_observation(crate::proxy::read_system_proxy_state()?);
        if desired_matches(&journal.desired, &current) {
            apply_observation(&journal.previous)?;
        }
        remove_journal()
    }
}

fn journal_path(home: &Path) -> PathBuf {
    home.join(RECOVERY_JOURNAL_FILE)
}

fn read_journal() -> anyhow::Result<Option<RecoveryJournal>> {
    let home = mihomo_platform::paths::get_home_dir()?;
    let path = journal_path(&home);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&std::fs::read_to_string(path)?)?))
}

fn write_journal(journal: &RecoveryJournal) -> anyhow::Result<()> {
    let home = mihomo_platform::paths::get_home_dir()?;
    fs::create_dir_all(&home)?;
    let path = journal_path(&home);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temporary = home.join(format!(
        "{RECOVERY_JOURNAL_FILE}.tmp-{}-{stamp}",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    #[cfg(unix)]
    fs::set_permissions(
        &temporary,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )?;
    file.write_all(&serde_json::to_vec_pretty(journal)?)?;
    file.sync_all()?;
    drop(file);
    if let Err(error) = atomic_replace(&temporary, &path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    #[cfg(unix)]
    fs::File::open(&home)?.sync_all()?;
    Ok(())
}

fn remove_journal() -> anyhow::Result<()> {
    let home = mihomo_platform::paths::get_home_dir()?;
    let path = journal_path(&home);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn atomic_replace(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(temporary, destination)
}

#[cfg(windows)]
fn atomic_replace(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let temporary: Vec<u16> = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;
    // SAFETY: both paths are NUL-terminated UTF-16 buffers owned for the
    // duration of this call, and the Windows API does not retain them.
    let replaced = unsafe { MoveFileExW(temporary.as_ptr(), destination.as_ptr(), flags) };
    if replaced == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn recovery_journal_lock() -> std::sync::MutexGuard<'static, ()> {
    SYSTEM_PROXY_RECOVERY_JOURNAL_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn process_start_time(pid: u32) -> Option<u64> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
        .process(Pid::from_u32(pid))
        .map(|process| process.start_time())
}

fn owner_process_is_live(journal: &RecoveryJournal) -> bool {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let Some(process) = system.process(Pid::from_u32(journal.owner_pid)) else {
        return false;
    };
    if matches!(
        process.status(),
        sysinfo::ProcessStatus::Zombie | sysinfo::ProcessStatus::Dead
    ) {
        return false;
    }
    // Old journals have no start-time field. Conservatively keep treating an
    // alive PID as owned rather than touching another process's proxy state.
    journal.owner_start_time == 0 || process.start_time() == journal.owner_start_time
}

fn arm_recovery_sync(
    previous: SystemProxyObservation,
    desired: SystemProxyDesiredState,
) -> Result<(), PortError> {
    let _lock = recovery_journal_lock();
    let existing = read_journal().map_err(|error| PortError::Io(error.to_string()))?;
    let previous = if let Some(journal) = existing {
        if journal.owner_pid != std::process::id() && owner_process_is_live(&journal) {
            return Err(PortError::Failed(
                "another live process owns the system proxy recovery journal".to_owned(),
            ));
        }
        journal.previous
    } else {
        previous
    };
    write_journal(&RecoveryJournal {
        owner_pid: std::process::id(),
        owner_start_time: process_start_time(std::process::id()).unwrap_or_default(),
        previous,
        desired,
    })
    .map_err(|error| PortError::Io(error.to_string()))
}

fn recover_orphaned_sync() -> Result<SystemProxyRecoveryReport, PortError> {
    let _lock = recovery_journal_lock();
    let Some(journal) = read_journal().map_err(|error| PortError::Io(error.to_string()))? else {
        return Ok(SystemProxyRecoveryReport::NotNeeded);
    };
    if journal.owner_pid != std::process::id() && owner_process_is_live(&journal) {
        return Ok(SystemProxyRecoveryReport::SkippedLiveOwner {
            owner_pid: journal.owner_pid,
        });
    }
    let observed = to_observation(
        crate::proxy::read_system_proxy_state()
            .map_err(|error| PortError::Io(error.to_string()))?,
    );
    if !desired_matches(&journal.desired, &observed) {
        remove_journal().map_err(|error| PortError::Io(error.to_string()))?;
        return Ok(SystemProxyRecoveryReport::SkippedExternal {
            expected: journal.desired,
            observed,
        });
    }
    apply_observation(&journal.previous).map_err(|error| PortError::Io(error.to_string()))?;
    let restored = to_observation(
        crate::proxy::read_system_proxy_state()
            .map_err(|error| PortError::Io(error.to_string()))?,
    );
    if !observation_matches(&journal.previous, &restored) {
        return Err(PortError::Failed(
            "system proxy recovery readback did not match the previous state".to_owned(),
        ));
    }
    remove_journal().map_err(|error| PortError::Io(error.to_string()))?;
    Ok(SystemProxyRecoveryReport::Restored {
        previous: journal.previous,
        restored,
    })
}

fn clear_recovery_sync() -> Result<(), PortError> {
    let _lock = recovery_journal_lock();
    if let Some(journal) = read_journal().map_err(|error| PortError::Io(error.to_string()))?
        && journal.owner_pid != std::process::id()
        && owner_process_is_live(&journal)
    {
        return Err(PortError::Failed(
            "another live process owns the system proxy recovery journal".to_owned(),
        ));
    }
    remove_journal().map_err(|error| PortError::Io(error.to_string()))
}

fn apply_observation(observation: &SystemProxyObservation) -> anyhow::Result<()> {
    if observation.enabled {
        let endpoint = observation
            .endpoint
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("enabled proxy state has no endpoint"))?;
        crate::proxy::apply_system_proxy_with_bypass(Some(endpoint), observation.bypass.as_deref())
    } else {
        crate::proxy::apply_system_proxy(None)
    }
}

fn desired_matches(desired: &SystemProxyDesiredState, observed: &SystemProxyObservation) -> bool {
    if desired.enabled != observed.enabled {
        return false;
    }
    !desired.enabled
        || (desired.endpoint == observed.endpoint
            && desired
                .bypass
                .as_ref()
                .is_none_or(|bypass| observed.bypass.as_ref() == Some(bypass)))
}

fn observation_matches(
    expected: &SystemProxyObservation,
    observed: &SystemProxyObservation,
) -> bool {
    desired_matches(
        &SystemProxyDesiredState {
            enabled: expected.enabled,
            endpoint: expected.endpoint.clone(),
            bypass: expected.bypass.clone(),
        },
        observed,
    )
}

fn to_observation(state: crate::proxy::SystemProxyState) -> SystemProxyObservation {
    SystemProxyObservation {
        enabled: state.enabled,
        endpoint: state.endpoint,
        bypass: state.bypass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_proxy_adapter_preserves_os_observation_shape() {
        let observation = to_observation(crate::proxy::SystemProxyState {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: Some("localhost".to_owned()),
        });
        assert!(observation.enabled);
        assert_eq!(observation.endpoint.as_deref(), Some("127.0.0.1:7890"));
        assert_eq!(observation.bypass.as_deref(), Some("localhost"));
    }

    #[test]
    fn recovery_target_matching_rejects_a_third_party_endpoint() {
        let desired = SystemProxyDesiredState {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: None,
        };
        assert!(desired_matches(
            &desired,
            &SystemProxyObservation {
                enabled: true,
                endpoint: Some("127.0.0.1:7890".to_owned()),
                bypass: None,
            }
        ));
        assert!(!desired_matches(
            &desired,
            &SystemProxyObservation {
                enabled: true,
                endpoint: Some("127.0.0.1:9999".to_owned()),
                bypass: None,
            }
        ));
    }

    #[tokio::test]
    async fn recovery_journal_round_trips_and_keeps_original_state() {
        let _guard = mihomo_platform::TEST_LOCK.lock().await;
        let home = tempfile::tempdir().expect("temporary home");
        assert!(mihomo_platform::paths::set_home_dir_override(
            home.path().to_path_buf()
        ));
        let previous = SystemProxyObservation {
            enabled: false,
            endpoint: None,
            bypass: None,
        };
        let desired = SystemProxyDesiredState {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: None,
        };
        arm_recovery_sync(previous.clone(), desired.clone()).expect("arm recovery");
        let journal = read_journal()
            .expect("read journal")
            .expect("journal exists");
        assert_eq!(journal.previous, previous);
        assert_eq!(journal.desired, desired);
        remove_journal().expect("remove journal");
        mihomo_platform::paths::clear_home_dir_override();
    }
}
