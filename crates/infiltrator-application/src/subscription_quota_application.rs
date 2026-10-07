//! Product-local, request-fenced quota observations; failed same-source reads retain actual facts.
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason};
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::subscription_quota::{
    QuotaProfileIdentity, SubscriptionQuotaSnapshot, SubscriptionQuotaSource,
    SubscriptionQuotaStatus,
};
use infiltrator_domain::profile_source::hash_document_bytes;
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_domain::subscription_quota::derive;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct SubscriptionQuotaApplication {
    state: Arc<Mutex<QuotaObservation>>,
}
struct QuotaObservation {
    owner: u64,
    issued: u64,
    accepted: u64,
    snapshot: SubscriptionQuotaSnapshot,
}
#[derive(Clone, Debug)]
pub struct QuotaRead {
    owner: u64,
    sequence: u64,
}
impl Default for SubscriptionQuotaApplication {
    fn default() -> Self {
        static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
        let owner = NEXT_OWNER
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("quota owner identity exhausted");
        Self {
            state: Arc::new(Mutex::new(QuotaObservation {
                owner,
                issued: 0,
                accepted: 0,
                snapshot: Default::default(),
            })),
        }
    }
}
impl SubscriptionQuotaApplication {
    pub fn begin(&self) -> QuotaRead {
        let mut state = self.state.lock().expect("quota observation");
        state.issued = state
            .issued
            .checked_add(1)
            .expect("quota read identity exhausted");
        QuotaRead {
            owner: state.owner,
            sequence: state.issued,
        }
    }
    pub fn project(
        &self,
        read: &QuotaRead,
        core: &CoreSnapshot,
        current: Option<&Result<QuotaProfileIdentity, Failure>>,
        profiles: Option<&Result<Vec<ProfileInfo>, Failure>>,
    ) -> SubscriptionQuotaSnapshot {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|time| i64::try_from(time.as_secs()).ok());
        self.project_at(read, core, current, profiles, now)
    }
    pub fn project_at(
        &self,
        read: &QuotaRead,
        core: &CoreSnapshot,
        current: Option<&Result<QuotaProfileIdentity, Failure>>,
        profiles: Option<&Result<Vec<ProfileInfo>, Failure>>,
        now_unix: Option<i64>,
    ) -> SubscriptionQuotaSnapshot {
        let mut state = self.state.lock().expect("quota observation");
        if read.owner != state.owner
            || read.sequence != state.issued
            || read.sequence <= state.accepted
        {
            return state.snapshot.clone();
        }
        state.accepted = read.sequence;
        let revision = read.sequence.max(core.revision);
        let selected = current.and_then(|result| result.as_ref().ok());
        let same_source = state.snapshot.source.as_ref().is_some_and(|source| {
            selected.is_some_and(|selected| {
                selected.profile == source.profile
                    && selected.provider_hash.is_some()
                    && selected.provider_hash == source.provider_hash
            }) && source.generation == core.generation
                && source.session_token == core.session_token
        });
        let same_source = same_source
            && profiles
                .and_then(|result| result.as_ref().ok())
                .and_then(|items| {
                    items.iter().find(|profile| {
                        profile.active
                            && selected.is_some_and(|selected| selected.profile == profile.name)
                    })
                })
                .is_none_or(|profile| {
                    state
                        .snapshot
                        .source
                        .as_ref()
                        .is_some_and(|source| source.path == profile.path)
                });
        if !same_source {
            state.snapshot = SubscriptionQuotaSnapshot {
                generation: core.generation,
                revision,
                ..Default::default()
            };
        }
        let result = project_input(core, revision, current, profiles, now_unix);
        match result {
            Ok(snapshot) => state.snapshot = snapshot,
            Err(failure) => {
                state.snapshot.generation = core.generation;
                state.snapshot.revision = revision;
                state.snapshot.retained = same_source && state.snapshot.source.is_some();
                state.snapshot.status = if failure.code == ErrorCode::Unsupported {
                    SubscriptionQuotaStatus::Unsupported
                } else {
                    SubscriptionQuotaStatus::Failed
                };
                state.snapshot.failure = Some(failure);
            }
        }
        state.snapshot.clone()
    }
}
fn project_input(
    core: &CoreSnapshot,
    revision: u64,
    current: Option<&Result<QuotaProfileIdentity, Failure>>,
    profiles: Option<&Result<Vec<ProfileInfo>, Failure>>,
    now_unix: Option<i64>,
) -> Result<SubscriptionQuotaSnapshot, Failure> {
    let current = current
        .ok_or_else(|| Failure::unsupported("Profile application is not composed"))?
        .as_ref()
        .map_err(Clone::clone)?;
    let profiles = profiles
        .ok_or_else(|| Failure::unsupported("Profile application is not composed"))?
        .as_ref()
        .map_err(Clone::clone)?;
    let active: Vec<_> = profiles.iter().filter(|profile| profile.active).collect();
    if active.is_empty() && current.profile.is_empty() {
        return Ok(SubscriptionQuotaSnapshot {
            generation: core.generation,
            revision,
            status: SubscriptionQuotaStatus::Empty,
            ..Default::default()
        });
    }
    let [profile] = active.as_slice() else {
        return Err(Failure::new(
            ErrorCode::InvalidState,
            "The active profile observation is inconsistent",
            true,
        )
        .with_reason(FailureReason::ActiveProfileInconsistent));
    };
    if profile.name != current.profile
        || profile
            .subscription_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .map(hash_document_bytes)
            != current.provider_hash
    {
        return Err(Failure::new(
            ErrorCode::NotReady,
            "The active profile changed during quota observation",
            true,
        )
        .with_reason(FailureReason::QuotaSourceChanged));
    }
    let now = now_unix.ok_or_else(|| {
        Failure::new(
            ErrorCode::InvalidState,
            "The current time is unavailable",
            true,
        )
        .with_reason(FailureReason::CurrentTimeUnavailable)
    })?;
    let mut snapshot = derive(core.generation, revision, profiles, now);
    if snapshot.status == SubscriptionQuotaStatus::Failed {
        return Err(snapshot
            .failure
            .expect("failed quota derivation has a cause"));
    }
    snapshot.source = Some(SubscriptionQuotaSource {
        provider_hash: current.provider_hash.clone(),
        profile: profile.name.clone(),
        path: profile.path.clone(),
        generation: core.generation,
        session_token: core.session_token,
    });
    Ok(snapshot)
}

#[cfg(test)]
#[path = "subscription_quota_application_tests.rs"]
mod tests;
