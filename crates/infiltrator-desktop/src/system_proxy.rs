//! Desktop system HTTP/SOCKS proxy port.

mod journal;

use crate::proxy::{
    DEFAULT_BYPASS, SystemProxyState, apply_system_proxy_with_bypass, read_system_proxy_state,
};
use async_trait::async_trait;
use infiltrator_contract::system_proxy::{
    SystemProxyClaim, SystemProxyClaimState, SystemProxyDesiredState, SystemProxyObservation,
    SystemProxyRecoveryReport, SystemProxyRecoverySnapshot, SystemProxyReleaseReport,
};
use infiltrator_ports::system_proxy::SystemProxyPort;
use journal::{
    RecoveryDecision, RecoveryJournal, RecoveryJournalStage, owner_process_is_live,
    process_start_time, read_journal, recovery_decision, recovery_journal_lock, remove_journal,
    write_journal,
};
use lurker_core_ports::error::PortError;
use std::process::id;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::task::spawn_blocking;

static SHARED_SYSTEM_PROXY_CLAIM: OnceLock<Arc<Mutex<SystemProxyClaimState>>> = OnceLock::new();
static SHARED_SYSTEM_PROXY_RECOVERY: OnceLock<Arc<Mutex<SystemProxyRecoverySnapshot>>> =
    OnceLock::new();
static NEXT_SYSTEM_PROXY_CLAIM_REVISION: AtomicU64 = AtomicU64::new(1);

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
    fn shared_claim(&self) -> Arc<Mutex<SystemProxyClaimState>> {
        SHARED_SYSTEM_PROXY_CLAIM
            .get_or_init(|| Arc::new(Mutex::new(SystemProxyClaimState::Unclaimed)))
            .clone()
    }

    fn shared_recovery(&self) -> Arc<Mutex<SystemProxyRecoverySnapshot>> {
        SHARED_SYSTEM_PROXY_RECOVERY
            .get_or_init(|| Arc::new(Mutex::new(SystemProxyRecoverySnapshot::default())))
            .clone()
    }

    async fn claim_and_apply(
        &self,
        desired: SystemProxyDesiredState,
    ) -> Result<(SystemProxyClaim, SystemProxyObservation), PortError> {
        let shared = self.shared_claim();
        spawn_blocking(move || claim_and_apply_sync(desired, &shared))
            .await
            .map_err(|error| PortError::Io(format!("system proxy claim worker failed: {error}")))?
    }

    async fn recover_orphaned(&self) -> Result<SystemProxyRecoveryReport, PortError> {
        let shared = self.shared_claim();
        spawn_blocking(move || recover_orphaned_sync(&shared))
            .await
            .map_err(|error| {
                PortError::Io(format!("system proxy recovery worker failed: {error}"))
            })?
    }

    async fn release_owned(
        &self,
        expected: &SystemProxyClaim,
    ) -> Result<SystemProxyReleaseReport, PortError> {
        let expected = expected.clone();
        let shared = self.shared_claim();
        spawn_blocking(move || release_owned_sync(&expected, &shared))
            .await
            .map_err(|error| {
                PortError::Io(format!("system proxy release worker failed: {error}"))
            })?
    }

    async fn snapshot(&self) -> Result<SystemProxyObservation, PortError> {
        spawn_blocking(snapshot_sync)
            .await
            .map_err(|error| PortError::Io(format!("system proxy probe worker failed: {error}")))?
    }
}

impl DesktopSystemProxy {
    /// Clean-exit hook: restore the pre-ownership state if the OS still has
    /// our target, otherwise leave a third-party change untouched.
    pub fn restore_and_clear_sync() -> anyhow::Result<()> {
        let _lock = recovery_journal_lock()?;
        let Some(journal) = read_journal()? else {
            return Ok(());
        };
        if journal.owner_pid != id() && owner_process_is_live(&journal) {
            return Ok(());
        }
        let claim = journal_to_claim(&journal);
        let current = to_observation(read_system_proxy_state()?);
        match recovery_decision(&journal, &current) {
            RecoveryDecision::RestorePrevious => {
                apply_observation(&journal.previous)?;
                let restored = to_observation(read_system_proxy_state()?);
                if !observation_matches(&journal.previous, &restored) {
                    anyhow::bail!("system proxy clean-exit restore readback did not match");
                }
            }
            RecoveryDecision::AlreadyRestored | RecoveryDecision::SkipExternal => {}
            RecoveryDecision::PreserveJournal => {
                anyhow::bail!("system proxy recovery journal was preserved due to ambiguous state");
            }
        }
        remove_journal()?;
        let shared = SHARED_SYSTEM_PROXY_CLAIM
            .get_or_init(|| Arc::new(Mutex::new(SystemProxyClaimState::Unclaimed)));
        clear_shared_claim_if_matches(shared, &claim);
        Ok(())
    }
}

fn claim_and_apply_sync(
    desired: SystemProxyDesiredState,
    shared: &Mutex<SystemProxyClaimState>,
) -> Result<(SystemProxyClaim, SystemProxyObservation), PortError> {
    if !desired.enabled {
        return Err(PortError::Failed(
            "system proxy claims must request an enabled state".to_owned(),
        ));
    }
    desired
        .endpoint
        .as_deref()
        .filter(|endpoint| !endpoint.trim().is_empty())
        .ok_or_else(|| {
            PortError::Failed("enabled system proxy claim has no endpoint".to_owned())
        })?;
    let _lock = recovery_journal_lock().map_err(|error| PortError::Io(error.to_string()))?;
    let existing = read_journal().map_err(|error| PortError::Io(error.to_string()))?;
    let previous = to_observation(
        read_system_proxy_state().map_err(|error| PortError::Io(error.to_string()))?,
    );
    let (previous, revision) = if let Some(journal) = existing {
        if journal.owner_pid != id() && owner_process_is_live(&journal) {
            return Err(PortError::Failed(
                "another live process owns the system proxy recovery journal".to_owned(),
            ));
        }
        let original = match recovery_decision(&journal, &previous) {
            RecoveryDecision::RestorePrevious => journal.previous,
            RecoveryDecision::AlreadyRestored | RecoveryDecision::SkipExternal => previous,
            RecoveryDecision::PreserveJournal => {
                return Err(PortError::Failed(
                    "system proxy recovery is incomplete or externally changed; recovery journal was preserved"
                        .to_owned(),
                ));
            }
        };
        let next = journal
            .revision
            .checked_add(1)
            .ok_or_else(|| PortError::Failed("system proxy claim revision exhausted".to_owned()))?;
        (original, next)
    } else {
        require_unclaimed_without_journal(shared)?;
        let revision = NEXT_SYSTEM_PROXY_CLAIM_REVISION
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| PortError::Failed("system proxy claim revision exhausted".to_owned()))?;
        (previous, revision)
    };
    let target = applying_target(&desired);
    let journal = RecoveryJournal {
        format_version: 3,
        owner_pid: id(),
        owner_start_time: process_start_time(id()).unwrap_or_default(),
        revision,
        previous,
        desired: desired.clone(),
        stage: RecoveryJournalStage::Applying {
            target: target.clone(),
        },
    };
    write_journal(&journal).map_err(|error| PortError::Io(error.to_string()))?;
    apply_observation(&target).map_err(|error| PortError::Io(error.to_string()))?;
    let observed = to_observation(
        read_system_proxy_state().map_err(|error| PortError::Io(error.to_string()))?,
    );
    if !observation_matches(&target, &observed) {
        return Err(PortError::Failed(
            "system proxy claim readback did not match the requested state".to_owned(),
        ));
    }
    let journal = RecoveryJournal {
        stage: RecoveryJournalStage::Claimed {
            observed: observed.clone(),
        },
        ..journal
    };
    write_journal(&journal).map_err(|error| PortError::Io(error.to_string()))?;
    let claim = SystemProxyClaim {
        revision,
        owner_pid: journal.owner_pid,
        owner_start_time: journal.owner_start_time,
        previous: journal.previous,
        desired: journal.desired,
        observed: observed.clone(),
    };
    *shared.lock().expect("system proxy claim lock") =
        SystemProxyClaimState::Claimed(claim.clone());
    Ok((claim, observed))
}

fn require_unclaimed_without_journal(
    shared: &Mutex<SystemProxyClaimState>,
) -> Result<(), PortError> {
    match &*shared.lock().expect("system proxy claim lock") {
        SystemProxyClaimState::Unclaimed => Ok(()),
        SystemProxyClaimState::Claimed(claim) => Err(PortError::Failed(format!(
            "system proxy claim revision {} exists without a recovery journal",
            claim.revision
        ))),
    }
}

fn snapshot_sync() -> Result<SystemProxyObservation, PortError> {
    let _lock = recovery_journal_lock().map_err(|error| PortError::Io(error.to_string()))?;
    read_system_proxy_state()
        .map(to_observation)
        .map_err(|error| PortError::Io(error.to_string()))
}

fn recover_orphaned_sync(
    shared: &Arc<Mutex<SystemProxyClaimState>>,
) -> Result<SystemProxyRecoveryReport, PortError> {
    let _lock = recovery_journal_lock().map_err(|error| PortError::Io(error.to_string()))?;
    let Some(journal) = read_journal().map_err(|error| PortError::Io(error.to_string()))? else {
        require_unclaimed_without_journal(shared)?;
        return Ok(SystemProxyRecoveryReport::NotNeeded);
    };
    if owner_process_is_live(&journal) {
        publish_shared_claim_from_journal(shared, &journal_to_claim(&journal));
        return Ok(SystemProxyRecoveryReport::SkippedLiveOwner {
            owner_pid: journal.owner_pid,
        });
    }
    let observed = to_observation(
        read_system_proxy_state().map_err(|error| PortError::Io(error.to_string()))?,
    );
    let report = match recovery_decision(&journal, &observed) {
        RecoveryDecision::AlreadyRestored => SystemProxyRecoveryReport::Restored {
            previous: journal.previous.clone(),
            restored: observed,
        },
        RecoveryDecision::SkipExternal => SystemProxyRecoveryReport::SkippedExternal {
            expected: journal.desired.clone(),
            observed,
        },
        RecoveryDecision::PreserveJournal => {
            return Err(PortError::Failed(
                "system proxy recovery is incomplete or externally changed; recovery journal was preserved"
                    .to_owned(),
            ));
        }
        RecoveryDecision::RestorePrevious => {
            apply_observation(&journal.previous)
                .map_err(|error| PortError::Io(error.to_string()))?;
            let restored = to_observation(
                read_system_proxy_state().map_err(|error| PortError::Io(error.to_string()))?,
            );
            if !observation_matches(&journal.previous, &restored) {
                return Err(PortError::Failed(
                    "system proxy recovery readback did not match the previous state".to_owned(),
                ));
            }
            SystemProxyRecoveryReport::Restored {
                previous: journal.previous.clone(),
                restored,
            }
        }
    };
    remove_journal().map_err(|error| PortError::Io(error.to_string()))?;
    clear_shared_claim_if_matches(shared, &journal_to_claim(&journal));
    Ok(report)
}

fn release_owned_sync(
    expected: &SystemProxyClaim,
    shared: &Arc<Mutex<SystemProxyClaimState>>,
) -> Result<SystemProxyReleaseReport, PortError> {
    let _lock = recovery_journal_lock().map_err(|error| PortError::Io(error.to_string()))?;
    let observed = to_observation(
        read_system_proxy_state().map_err(|error| PortError::Io(error.to_string()))?,
    );
    let Some(journal) = read_journal().map_err(|error| PortError::Io(error.to_string()))? else {
        return Ok(SystemProxyReleaseReport::NoClaim { observed });
    };
    if journal.owner_pid != id() && owner_process_is_live(&journal) {
        let current = journal_to_claim(&journal);
        publish_shared_claim_from_journal(shared, &current);
        return Ok(SystemProxyReleaseReport::SkippedLiveOwner { current, observed });
    }
    let current = journal_to_claim(&journal);
    if current != *expected {
        publish_claim_if_newer(shared, &current, expected);
        return Ok(SystemProxyReleaseReport::SkippedStaleClaim { current, observed });
    }
    match recovery_decision(&journal, &observed) {
        RecoveryDecision::AlreadyRestored => {
            remove_journal().map_err(|error| PortError::Io(error.to_string()))?;
            clear_shared_claim_if_matches(shared, &current);
            return Ok(SystemProxyReleaseReport::Restored {
                previous: journal.previous,
                restored: observed,
            });
        }
        RecoveryDecision::SkipExternal => {
            remove_journal().map_err(|error| PortError::Io(error.to_string()))?;
            clear_shared_claim_if_matches(shared, &current);
            return Ok(SystemProxyReleaseReport::SkippedExternal {
                expected: current,
                observed,
            });
        }
        RecoveryDecision::PreserveJournal => {
            return Err(PortError::Failed(
                "system proxy release is incomplete or externally changed; recovery journal was preserved"
                    .to_owned(),
            ));
        }
        RecoveryDecision::RestorePrevious => {}
    }
    if expected.observed != observed {
        publish_claim_if_newer(shared, &current, expected);
        return Ok(SystemProxyReleaseReport::SkippedStaleClaim { current, observed });
    }

    write_journal(&RecoveryJournal {
        stage: RecoveryJournalStage::Releasing {
            claimed: expected.observed.clone(),
        },
        ..journal.clone()
    })
    .map_err(|error| PortError::Io(error.to_string()))?;

    apply_observation(&journal.previous).map_err(|error| PortError::Io(error.to_string()))?;
    let restored = to_observation(
        read_system_proxy_state().map_err(|error| PortError::Io(error.to_string()))?,
    );
    if !observation_matches(&journal.previous, &restored) {
        return Err(PortError::Failed(
            "system proxy release readback did not match the pre-claim state".to_owned(),
        ));
    }
    remove_journal().map_err(|error| PortError::Io(error.to_string()))?;
    clear_shared_claim_if_matches(shared, expected);
    Ok(SystemProxyReleaseReport::Restored {
        previous: journal.previous,
        restored,
    })
}

fn applying_target(desired: &SystemProxyDesiredState) -> SystemProxyObservation {
    if desired.enabled {
        let bypass = match desired.bypass.as_deref() {
            Some(bypass) if bypass.trim().is_empty() => None,
            Some(bypass) => Some(bypass.to_owned()),
            None => Some(DEFAULT_BYPASS.to_owned()),
        };
        SystemProxyObservation {
            enabled: true,
            endpoint: desired.endpoint.clone(),
            bypass,
        }
    } else {
        SystemProxyObservation::default()
    }
}

fn apply_observation(observation: &SystemProxyObservation) -> anyhow::Result<()> {
    if observation.enabled {
        let endpoint = observation
            .endpoint
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("enabled proxy state has no endpoint"))?;
        apply_system_proxy_with_bypass(
            Some(endpoint),
            Some(observation.bypass.as_deref().unwrap_or_default()),
        )
    } else {
        apply_system_proxy_with_bypass(None, None)
    }
}

fn journal_to_claim(journal: &RecoveryJournal) -> SystemProxyClaim {
    let observed = match &journal.stage {
        RecoveryJournalStage::Applying { target } => target.clone(),
        RecoveryJournalStage::Claimed { observed } => observed.clone(),
        RecoveryJournalStage::Releasing { claimed } => claimed.clone(),
    };
    SystemProxyClaim {
        revision: journal.revision,
        owner_pid: journal.owner_pid,
        owner_start_time: journal.owner_start_time,
        previous: journal.previous.clone(),
        desired: journal.desired.clone(),
        observed,
    }
}

fn clear_shared_claim_if_matches(
    shared: &Mutex<SystemProxyClaimState>,
    expected: &SystemProxyClaim,
) {
    let mut current = shared.lock().expect("system proxy claim lock");
    if matches!(&*current, SystemProxyClaimState::Claimed(claim) if claim == expected) {
        *current = SystemProxyClaimState::Unclaimed;
    }
}

fn publish_shared_claim_from_journal(
    shared: &Mutex<SystemProxyClaimState>,
    authoritative: &SystemProxyClaim,
) {
    let mut current = shared.lock().expect("system proxy claim lock");
    match &*current {
        SystemProxyClaimState::Unclaimed => {
            *current = SystemProxyClaimState::Claimed(authoritative.clone());
        }
        SystemProxyClaimState::Claimed(claim) if claim.revision < authoritative.revision => {
            *current = SystemProxyClaimState::Claimed(authoritative.clone());
        }
        SystemProxyClaimState::Claimed(claim) if claim == authoritative => {}
        SystemProxyClaimState::Claimed(_) => {}
    }
}

fn publish_claim_if_newer(
    shared: &Mutex<SystemProxyClaimState>,
    authoritative: &SystemProxyClaim,
    expected: &SystemProxyClaim,
) {
    if authoritative.revision < expected.revision {
        return;
    }
    let mut current = shared.lock().expect("system proxy claim lock");
    match &*current {
        SystemProxyClaimState::Unclaimed => {
            *current = SystemProxyClaimState::Claimed(authoritative.clone());
        }
        SystemProxyClaimState::Claimed(claim) if claim == expected => {
            *current = SystemProxyClaimState::Claimed(authoritative.clone());
        }
        SystemProxyClaimState::Claimed(claim) if claim == authoritative => {}
        SystemProxyClaimState::Claimed(claim) if claim.revision < authoritative.revision => {
            *current = SystemProxyClaimState::Claimed(authoritative.clone());
        }
        SystemProxyClaimState::Claimed(_) => {}
    }
}

fn observation_matches(
    expected: &SystemProxyObservation,
    observed: &SystemProxyObservation,
) -> bool {
    expected == observed
}

fn to_observation(state: SystemProxyState) -> SystemProxyObservation {
    if state.enabled {
        SystemProxyObservation {
            enabled: true,
            endpoint: state.endpoint,
            bypass: state.bypass,
        }
    } else {
        SystemProxyObservation::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use infiltrator_platform::paths::clear_home_dir_override;
    #[cfg(test)]
    use infiltrator_platform::paths::set_home_dir_override;

    fn test_journal(stage: RecoveryJournalStage) -> RecoveryJournal {
        RecoveryJournal {
            format_version: 3,
            owner_pid: 1,
            owner_start_time: 1,
            revision: 1,
            previous: SystemProxyObservation::default(),
            desired: SystemProxyDesiredState {
                enabled: true,
                endpoint: Some("127.0.0.1:7890".to_owned()),
                bypass: Some("localhost".to_owned()),
            },
            stage,
        }
    }

    fn test_claim(revision: u64) -> SystemProxyClaim {
        let mut journal = test_journal(RecoveryJournalStage::Claimed {
            observed: SystemProxyObservation {
                enabled: true,
                endpoint: Some("127.0.0.1:7890".to_owned()),
                bypass: Some("localhost".to_owned()),
            },
        });
        journal.revision = revision;
        journal_to_claim(&journal)
    }

    #[test]
    fn missing_journal_preserves_an_existing_shared_claim_and_fails_closed() {
        let claim = test_claim(5);
        let shared = Mutex::new(SystemProxyClaimState::Claimed(claim.clone()));

        let error = require_unclaimed_without_journal(&shared)
            .expect_err("a claimed state without a journal is inconsistent");

        assert!(matches!(error, PortError::Failed(_)));
        assert_eq!(
            *shared.lock().expect("shared claim lock"),
            SystemProxyClaimState::Claimed(claim)
        );
    }

    #[test]
    fn shared_claim_publication_is_revision_fenced() {
        let expected = test_claim(5);
        let authoritative = test_claim(6);
        let shared = Mutex::new(SystemProxyClaimState::Claimed(expected.clone()));

        publish_claim_if_newer(&shared, &authoritative, &expected);
        assert_eq!(
            *shared.lock().expect("shared claim lock"),
            SystemProxyClaimState::Claimed(authoritative.clone())
        );

        let newer = test_claim(7);
        *shared.lock().expect("shared claim lock") = SystemProxyClaimState::Claimed(newer.clone());
        publish_claim_if_newer(&shared, &authoritative, &expected);
        assert_eq!(
            *shared.lock().expect("shared claim lock"),
            SystemProxyClaimState::Claimed(newer)
        );

        *shared.lock().expect("shared claim lock") = SystemProxyClaimState::Unclaimed;
        let stale = test_claim(4);
        publish_claim_if_newer(&shared, &stale, &expected);
        assert_eq!(
            *shared.lock().expect("shared claim lock"),
            SystemProxyClaimState::Unclaimed
        );
    }

    #[test]
    fn live_owner_journal_publication_is_revision_fenced() {
        let authoritative = test_claim(6);
        let shared = Mutex::new(SystemProxyClaimState::Unclaimed);

        publish_shared_claim_from_journal(&shared, &authoritative);
        assert_eq!(
            *shared.lock().expect("shared claim lock"),
            SystemProxyClaimState::Claimed(authoritative.clone())
        );

        let newer = test_claim(7);
        *shared.lock().expect("shared claim lock") = SystemProxyClaimState::Claimed(newer.clone());
        publish_shared_claim_from_journal(&shared, &authoritative);
        assert_eq!(
            *shared.lock().expect("shared claim lock"),
            SystemProxyClaimState::Claimed(newer)
        );
    }

    #[test]
    fn shared_claim_cleanup_only_clears_the_exact_claim() {
        let expected = test_claim(5);
        let replacement = test_claim(6);
        let shared = Mutex::new(SystemProxyClaimState::Claimed(replacement.clone()));

        clear_shared_claim_if_matches(&shared, &expected);
        assert_eq!(
            *shared.lock().expect("shared claim lock"),
            SystemProxyClaimState::Claimed(replacement)
        );

        *shared.lock().expect("shared claim lock") =
            SystemProxyClaimState::Claimed(expected.clone());
        clear_shared_claim_if_matches(&shared, &expected);
        assert_eq!(
            *shared.lock().expect("shared claim lock"),
            SystemProxyClaimState::Unclaimed
        );
    }

    #[test]
    fn recovery_decision_is_exhaustive_for_each_journal_stage() {
        let desired = SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: Some("localhost".to_owned()),
        };
        let external = SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:9999".to_owned()),
            bypass: None,
        };
        let previous = SystemProxyObservation::default();

        let applying = test_journal(RecoveryJournalStage::Applying {
            target: desired.clone(),
        });
        assert_eq!(
            recovery_decision(&applying, &desired),
            RecoveryDecision::RestorePrevious
        );
        assert_eq!(
            recovery_decision(&applying, &previous),
            RecoveryDecision::AlreadyRestored
        );
        assert_eq!(
            recovery_decision(&applying, &external),
            RecoveryDecision::PreserveJournal
        );
        let changed_bypass = SystemProxyObservation {
            enabled: true,
            endpoint: desired.endpoint.clone(),
            bypass: Some("third-party.example".to_owned()),
        };
        assert_eq!(
            recovery_decision(&applying, &changed_bypass),
            RecoveryDecision::PreserveJournal
        );

        let claimed = test_journal(RecoveryJournalStage::Claimed {
            observed: desired.clone(),
        });
        assert_eq!(
            recovery_decision(&claimed, &desired),
            RecoveryDecision::RestorePrevious
        );
        assert_eq!(
            recovery_decision(&claimed, &external),
            RecoveryDecision::SkipExternal
        );

        let releasing = test_journal(RecoveryJournalStage::Releasing {
            claimed: desired.clone(),
        });
        assert_eq!(
            recovery_decision(&releasing, &desired),
            RecoveryDecision::RestorePrevious
        );
        assert_eq!(
            recovery_decision(&releasing, &previous),
            RecoveryDecision::AlreadyRestored
        );
        assert_eq!(
            recovery_decision(&releasing, &external),
            RecoveryDecision::PreserveJournal
        );
    }

    #[test]
    fn applying_claim_with_unspecified_bypass_uses_an_exact_default_target() {
        let desired = SystemProxyDesiredState {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: None,
        };
        let target = applying_target(&desired);
        assert_eq!(target.bypass.as_deref(), Some(DEFAULT_BYPASS));
        let journal = RecoveryJournal {
            format_version: 3,
            owner_pid: 1,
            owner_start_time: 1,
            revision: 1,
            previous: SystemProxyObservation::default(),
            desired: desired.clone(),
            stage: RecoveryJournalStage::Applying { target },
        };
        let changed_bypass = SystemProxyObservation {
            enabled: true,
            endpoint: desired.endpoint.clone(),
            bypass: Some("third-party.example".to_owned()),
        };
        assert!(desired.matches(&changed_bypass));
        assert_eq!(
            recovery_decision(&journal, &changed_bypass),
            RecoveryDecision::PreserveJournal
        );
    }

    #[test]
    fn disabled_host_residue_is_not_exposed_as_effective_proxy_state() {
        assert_eq!(
            to_observation(SystemProxyState {
                enabled: false,
                endpoint: Some("127.0.0.1:7890".to_owned()),
                bypass: Some("localhost".to_owned()),
            }),
            SystemProxyObservation::default()
        );
    }

    #[test]
    fn desktop_proxy_adapter_preserves_os_observation_shape() {
        let observation = to_observation(SystemProxyState {
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
        assert!(desired.matches(&SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: None,
        }));
        assert!(!desired.matches(&SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:9999".to_owned()),
            bypass: None,
        }));
    }

    #[tokio::test]
    async fn recovery_journal_round_trips_and_keeps_original_state() {
        let _guard = infiltrator_platform::TEST_LOCK.lock().await;
        let home = tempfile::tempdir().expect("temporary home");
        assert!(set_home_dir_override(home.path().to_path_buf()));
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
        let journal = RecoveryJournal {
            format_version: 3,
            owner_pid: id(),
            owner_start_time: process_start_time(id()).unwrap_or_default(),
            revision: 1,
            previous: previous.clone(),
            desired: desired.clone(),
            stage: RecoveryJournalStage::Claimed {
                observed: SystemProxyObservation {
                    enabled: true,
                    endpoint: desired.endpoint.clone(),
                    bypass: desired.bypass.clone(),
                },
            },
        };
        write_journal(&journal).expect("write recovery journal");
        let journal = read_journal()
            .expect("read journal")
            .expect("journal exists");
        assert_eq!(journal.previous, previous);
        assert_eq!(journal.desired, desired);
        assert!(matches!(
            journal.stage,
            RecoveryJournalStage::Claimed { .. }
        ));
        remove_journal().expect("remove journal");
        clear_home_dir_override();
    }
}
