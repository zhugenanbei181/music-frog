use infiltrator_contract::system_proxy::{SystemProxyDesiredState, SystemProxyObservation};
use infiltrator_platform::paths::get_home_dir;
use lurker_profile_store::instance_lock::{
    ThreadedInstanceLockGuard, acquire_threaded_instance_lock,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Write;
#[cfg(windows)]
use std::iter::once;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::{Mutex, MutexGuard, OnceLock};
#[cfg(windows)]
use std::thread::sleep;
#[cfg(windows)]
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, ProcessesToUpdate, System};

static SYSTEM_PROXY_RECOVERY_JOURNAL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const RECOVERY_JOURNAL_FILE: &str = ".musicfrog-system-proxy-recovery.json";
const RECOVERY_JOURNAL_LOCK_FILE: &str = ".musicfrog-system-proxy-recovery.lock";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct RecoveryJournal {
    pub(super) format_version: u8,
    pub(super) owner_pid: u32,
    pub(super) owner_start_time: u64,
    pub(super) revision: u64,
    pub(super) previous: SystemProxyObservation,
    pub(super) desired: SystemProxyDesiredState,
    pub(super) stage: RecoveryJournalStage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum RecoveryJournalStage {
    Applying { target: SystemProxyObservation },
    Claimed { observed: SystemProxyObservation },
    Releasing { claimed: SystemProxyObservation },
}

#[derive(Deserialize)]
struct VersionTwoRecoveryJournal {
    owner_pid: u32,
    #[serde(default)]
    owner_start_time: u64,
    revision: u64,
    previous: SystemProxyObservation,
    desired: SystemProxyDesiredState,
    stage: VersionTwoRecoveryJournalStage,
}

#[derive(Deserialize)]
enum VersionTwoRecoveryJournalStage {
    Applying,
    Claimed { observed: SystemProxyObservation },
    Releasing { claimed: SystemProxyObservation },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveryDecision {
    RestorePrevious,
    AlreadyRestored,
    SkipExternal,
    PreserveJournal,
}

#[derive(Deserialize)]
struct LegacyRecoveryJournal {
    owner_pid: u32,
    #[serde(default)]
    owner_start_time: u64,
    previous: SystemProxyObservation,
    desired: SystemProxyDesiredState,
}

pub(super) struct RecoveryJournalGuard {
    _cross_process: ThreadedInstanceLockGuard,
    _process: MutexGuard<'static, ()>,
}

pub(super) fn recovery_journal_lock() -> anyhow::Result<RecoveryJournalGuard> {
    let home = get_home_dir()?;
    fs::create_dir_all(&home)?;
    let process = SYSTEM_PROXY_RECOVERY_JOURNAL_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let cross_process = acquire_threaded_instance_lock(&home.join(RECOVERY_JOURNAL_LOCK_FILE))
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(RecoveryJournalGuard {
        _cross_process: cross_process,
        _process: process,
    })
}

pub(super) fn process_start_time(pid: u32) -> Option<u64> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
        .process(Pid::from_u32(pid))
        .map(|process| process.start_time())
}

pub(super) fn owner_process_is_live(journal: &RecoveryJournal) -> bool {
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
    journal.owner_start_time == 0 || process.start_time() == journal.owner_start_time
}

fn journal_path(home: &Path) -> PathBuf {
    home.join(RECOVERY_JOURNAL_FILE)
}

pub(super) fn read_journal() -> anyhow::Result<Option<RecoveryJournal>> {
    let home = get_home_dir()?;
    let path = journal_path(&home);
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    decode_journal(&content).map(Some)
}

fn decode_journal(content: &str) -> anyhow::Result<RecoveryJournal> {
    let value: serde_json::Value = serde_json::from_str(content)?;
    let format_version = match value.get("format_version") {
        None => None,
        Some(version) => Some(
            version
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("invalid system proxy journal format version"))?,
        ),
    };
    match format_version {
        None => Ok(migrate_legacy(serde_json::from_value(value)?)),
        Some(2) => Ok(migrate_version_two(serde_json::from_value(value)?)),
        Some(3) => Ok(serde_json::from_value(value)?),
        Some(version) => anyhow::bail!("unsupported system proxy journal version {version}"),
    }
}

fn migrate_legacy(legacy: LegacyRecoveryJournal) -> RecoveryJournal {
    let target = super::applying_target(&legacy.desired);
    RecoveryJournal {
        format_version: 3,
        owner_pid: legacy.owner_pid,
        owner_start_time: legacy.owner_start_time,
        revision: 0,
        previous: legacy.previous,
        desired: legacy.desired,
        stage: RecoveryJournalStage::Applying { target },
    }
}

fn migrate_version_two(legacy: VersionTwoRecoveryJournal) -> RecoveryJournal {
    let stage = match legacy.stage {
        VersionTwoRecoveryJournalStage::Applying => RecoveryJournalStage::Applying {
            target: super::applying_target(&legacy.desired),
        },
        VersionTwoRecoveryJournalStage::Claimed { observed } => {
            RecoveryJournalStage::Claimed { observed }
        }
        VersionTwoRecoveryJournalStage::Releasing { claimed } => {
            RecoveryJournalStage::Releasing { claimed }
        }
    };
    RecoveryJournal {
        format_version: 3,
        owner_pid: legacy.owner_pid,
        owner_start_time: legacy.owner_start_time,
        revision: legacy.revision,
        previous: legacy.previous,
        desired: legacy.desired,
        stage,
    }
}

pub(super) fn write_journal(journal: &RecoveryJournal) -> anyhow::Result<()> {
    let home = get_home_dir()?;
    fs::create_dir_all(&home)?;
    let path = journal_path(&home);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temporary = home.join(format!("{RECOVERY_JOURNAL_FILE}.tmp-{}-{stamp}", id()));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    #[cfg(unix)]
    fs::set_permissions(&temporary, PermissionsExt::from_mode(0o600))?;
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

pub(super) fn remove_journal() -> anyhow::Result<()> {
    let home = get_home_dir()?;
    let path = journal_path(&home);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(not(windows))]
fn atomic_replace(temporary: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temporary, destination)
}

#[cfg(windows)]
fn atomic_replace(temporary: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let temporary: Vec<u16> = temporary.as_os_str().encode_wide().chain(once(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(once(0))
        .collect();
    let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;

    for attempt in 0..4 {
        // SAFETY: both paths are NUL-terminated UTF-16 buffers owned for the
        // duration of this call, and the Windows API does not retain them.
        let replaced = unsafe { MoveFileExW(temporary.as_ptr(), destination.as_ptr(), flags) };
        if replaced != 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if let Some(code) = err.raw_os_error() {
            if (code == 32 || code == 5) && attempt < 3 {
                sleep(Duration::from_millis(50 * (attempt + 1) as u64));
                continue;
            }
        }
        return Err(err);
    }
    Err(io::Error::last_os_error())
}

pub(super) fn recovery_decision(
    journal: &RecoveryJournal,
    observed: &SystemProxyObservation,
) -> RecoveryDecision {
    match &journal.stage {
        RecoveryJournalStage::Applying { target } => {
            if journal.previous == *observed {
                RecoveryDecision::AlreadyRestored
            } else if target == observed {
                RecoveryDecision::RestorePrevious
            } else {
                RecoveryDecision::PreserveJournal
            }
        }
        RecoveryJournalStage::Claimed { observed: claimed } => {
            if claimed == observed {
                RecoveryDecision::RestorePrevious
            } else {
                RecoveryDecision::SkipExternal
            }
        }
        RecoveryJournalStage::Releasing { claimed } => {
            if journal.previous == *observed {
                RecoveryDecision::AlreadyRestored
            } else if claimed == observed {
                RecoveryDecision::RestorePrevious
            } else {
                RecoveryDecision::PreserveJournal
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy::DEFAULT_BYPASS;

    fn desired_without_bypass() -> SystemProxyDesiredState {
        SystemProxyDesiredState {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: None,
        }
    }

    fn previous() -> SystemProxyObservation {
        SystemProxyObservation::default()
    }

    fn version_two(stage: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "format_version": 2,
            "owner_pid": 42,
            "owner_start_time": 7,
            "revision": 3,
            "previous": previous(),
            "desired": desired_without_bypass(),
            "stage": stage,
        })
    }

    #[test]
    fn legacy_journal_dispatch_migrates_to_an_exact_default_bypass_target() {
        let legacy = serde_json::json!({
            "owner_pid": 42,
            "previous": previous(),
            "desired": desired_without_bypass(),
        });
        let migrated = decode_journal(&legacy.to_string()).expect("dispatch legacy journal");

        assert_eq!(migrated.format_version, 3);
        assert_eq!(migrated.owner_start_time, 0);
        assert_eq!(migrated.revision, 0);
        let RecoveryJournalStage::Applying { target } = migrated.stage else {
            panic!("legacy journal must migrate to Applying")
        };
        assert!(target.enabled);
        assert_eq!(target.endpoint.as_deref(), Some("127.0.0.1:7890"));
        assert_eq!(target.bypass.as_deref(), Some(DEFAULT_BYPASS));
    }

    #[test]
    fn version_two_applying_dispatch_migrates_to_an_exact_default_bypass_target() {
        let journal = version_two(serde_json::json!("Applying"));
        let migrated = decode_journal(&journal.to_string()).expect("dispatch v2 Applying journal");

        assert_eq!(migrated.format_version, 3);
        assert_eq!(migrated.revision, 3);
        let RecoveryJournalStage::Applying { target } = migrated.stage else {
            panic!("v2 Applying journal must migrate to Applying")
        };
        assert!(target.enabled);
        assert_eq!(target.endpoint.as_deref(), Some("127.0.0.1:7890"));
        assert_eq!(target.bypass.as_deref(), Some(DEFAULT_BYPASS));
    }

    #[test]
    fn version_two_claimed_and_releasing_dispatch_preserves_exact_observations() {
        let claimed_observed = SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: Some("localhost,127.0.0.1".to_owned()),
        };
        let releasing_claimed = SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: None,
        };
        let claimed_journal = version_two(serde_json::json!({
            "Claimed": {"observed": claimed_observed.clone()}
        }));
        let releasing_journal = version_two(serde_json::json!({
            "Releasing": {"claimed": releasing_claimed.clone()}
        }));
        let claimed =
            decode_journal(&claimed_journal.to_string()).expect("dispatch v2 Claimed journal");
        let releasing =
            decode_journal(&releasing_journal.to_string()).expect("dispatch v2 Releasing journal");

        assert_eq!(claimed.revision, 3);
        assert_eq!(
            claimed.stage,
            RecoveryJournalStage::Claimed {
                observed: claimed_observed,
            }
        );
        assert_eq!(
            releasing.stage,
            RecoveryJournalStage::Releasing {
                claimed: releasing_claimed,
            }
        );
    }

    #[test]
    fn version_three_dispatch_reads_the_current_format_without_migration() {
        let target = SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: Some("localhost".to_owned()),
        };
        let current = RecoveryJournal {
            format_version: 3,
            owner_pid: 42,
            owner_start_time: 7,
            revision: 8,
            previous: previous(),
            desired: desired_without_bypass(),
            stage: RecoveryJournalStage::Claimed {
                observed: target.clone(),
            },
        };
        let decoded =
            decode_journal(&serde_json::to_string(&current).expect("serialize v3 journal"))
                .expect("dispatch v3 journal");

        assert_eq!(decoded.format_version, 3);
        assert_eq!(decoded.revision, 8);
        assert_eq!(decoded.stage, current.stage);
    }

    #[test]
    fn unsupported_journal_version_is_rejected() {
        let unsupported = serde_json::json!({
            "format_version": 4,
        });
        assert!(decode_journal(&unsupported.to_string()).is_err());
    }
}
