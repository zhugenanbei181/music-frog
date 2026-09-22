//! DUAL-07-05 / DUAL-07-06: shared subscription refresh orchestration.
//!
//! Both surfaces used to drive `ProfileApplication::update_subscription_*`
//! directly, so retry/backoff and re-entry protection lived either in the
//! admin host or in a surface-local flag. This module owns both at the shared
//! application layer:
//!
//! * exponential backoff sleeps through the injected
//!   [`ApplicationRuntime::sleep`] seam, so the application never imports an
//!   executor;
//! * a process-wide single-flight registry keyed by `(config dir, profile)`
//!   rejects a second concurrent refresh of the same profile with a typed
//!   `InvalidState` failure instead of silently doubling the download.
//!
//! DUAL-07-09 adds the third shared decision: after an update commits new
//! content for the *active* profile, the persisted `auto_reload_core`
//! preference decides whether the host's [`CoreReloadPort`] brings it live.
//! Without that seam the report carries a typed
//! [`CoreReloadOutcome::Unsupported`] instead of a silent no-op.
//!
//! The profile store adapter and the outbound subscription source stay ports;
//! only the orchestration is shared.

use futures_util::stream::{self, StreamExt};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::subscription_import::{
    CoreReloadOutcome, SubscriptionBatchReport, SubscriptionUpdateOutcome, SubscriptionUpdateReport,
};
use infiltrator_domain::profiles::sanitize_profile_name;
use infiltrator_domain::subscription_scheduler_policy::RetryBackoffPolicy;
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::core_reload::CoreReloadPort;
use infiltrator_ports::subscription_notification::{
    SubscriptionNotification, SubscriptionNotificationKind, SubscriptionNotificationPort,
};
use infiltrator_ports::subscription_source::SubscriptionSource;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::profile_application::ProfileApplication;

/// Process-wide registry of subscription refreshes currently in flight.
///
/// Keyed by `(config dir, profile)` so every `ProfileApplication` built over
/// the same store shares one guard, regardless of which surface constructed
/// it. A long-lived `'static` map is intentional: the desktop service outlives
/// any single surface and the guard must survive surface reconstruction.
fn inflight_refreshes() -> &'static Mutex<HashSet<(PathBuf, String)>> {
    static INFLIGHT: OnceLock<Mutex<HashSet<(PathBuf, String)>>> = OnceLock::new();
    INFLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

/// RAII single-flight slot. Dropping it (including on a cancelled future or a
/// panicking task) releases the profile for the next refresh.
#[derive(Debug)]
pub struct SubscriptionRefreshGuard {
    key: (PathBuf, String),
}

impl SubscriptionRefreshGuard {
    /// The profile this guard currently holds.
    pub fn profile(&self) -> &str {
        &self.key.1
    }
}

impl Drop for SubscriptionRefreshGuard {
    fn drop(&mut self) {
        if let Ok(mut inflight) = inflight_refreshes().lock() {
            inflight.remove(&self.key);
        }
    }
}

/// Shared subscription refresh: retry/backoff plus single-flight protection.
#[derive(Clone)]
pub struct SubscriptionRefreshApplication {
    profile: ProfileApplication,
    runtime: Arc<dyn ApplicationRuntime>,
    policy: RetryBackoffPolicy,
    notifier: Option<Arc<dyn SubscriptionNotificationPort>>,
    core_reload: Option<Arc<dyn CoreReloadPort>>,
}

impl SubscriptionRefreshApplication {
    pub fn new(
        profile: ProfileApplication,
        runtime: Arc<dyn ApplicationRuntime>,
        policy: RetryBackoffPolicy,
    ) -> Self {
        Self {
            profile,
            runtime,
            policy,
            notifier: None,
            core_reload: None,
        }
    }

    /// Production policy: the domain-defined 30s / 1m / 5m backoff.
    pub fn with_default_policy(
        profile: ProfileApplication,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        Self::new(profile, runtime, RetryBackoffPolicy::default())
    }

    /// DUAL-07-10: attach the host notification port. Every terminal refresh
    /// outcome then emits one locale-neutral notification; without a port the
    /// orchestration stays silent (a headless host).
    pub fn with_notifier(mut self, notifier: Arc<dyn SubscriptionNotificationPort>) -> Self {
        self.notifier = Some(notifier);
        self
    }

    /// DUAL-07-09: attach the host reload seam. A successful update of the
    /// active profile is then applied to the running core when the profile's
    /// persisted `auto_reload_core` preference allows it.
    pub fn with_core_reload<Port: CoreReloadPort + 'static>(mut self, port: Port) -> Self {
        self.core_reload = Some(Arc::new(port));
        self
    }

    /// Whether a host reload seam is installed.
    pub fn has_core_reload(&self) -> bool {
        self.core_reload.is_some()
    }

    /// The retry policy this application applies.
    pub fn policy(&self) -> &RetryBackoffPolicy {
        &self.policy
    }

    /// Claim the single-flight slot for `profile`.
    ///
    /// Returns a typed `InvalidState` failure when a refresh of the same
    /// profile is already running. The caller must hold the returned guard for
    /// the whole refresh; dropping it releases the slot.
    pub fn begin_refresh(&self, name: &str) -> Result<SubscriptionRefreshGuard, Failure> {
        let name = sanitize_profile_name(name)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let key = (self.profile.config_dir(), name);
        let mut inflight = inflight_refreshes().lock().map_err(|_| {
            Failure::new(
                ErrorCode::Internal,
                "subscription refresh registry is poisoned",
                true,
            )
        })?;
        if !inflight.insert(key.clone()) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                format!("subscription update for `{}` is already in progress", key.1),
                true,
            ));
        }
        Ok(SubscriptionRefreshGuard { key })
    }

    /// Refresh one profile through the conditional path with backoff, then
    /// apply the DUAL-07-09 core-reload decision to the report.
    ///
    /// The single-flight guard is held across every retry so the app can never
    /// fan out into two competing downloads for the same profile.
    pub async fn refresh_profile<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        name: &str,
    ) -> Result<SubscriptionUpdateReport, Failure> {
        let result = self.refresh_profile_once(source, name).await;
        if let Some(notifier) = &self.notifier {
            notifier.notify(notification_for_refresh(name, &result));
        }
        result
    }

    /// One full single-profile refresh: guarded download plus the reload
    /// decision. The batch path uses [`Self::refresh_profile_inner`] so a
    /// multi-profile run reloads the core at most once.
    async fn refresh_profile_once<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        name: &str,
    ) -> Result<SubscriptionUpdateReport, Failure> {
        let mut report = self.refresh_profile_inner(source, name).await?;
        let changed = matches!(report.outcome, SubscriptionUpdateOutcome::Updated { .. });
        report.core_reload = self.core_reload_outcome(name, changed).await?;
        Ok(report)
    }

    /// The guarded refresh body without the reload decision, so the batch path
    /// can aggregate first and reload once for the active profile.
    async fn refresh_profile_inner<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        name: &str,
    ) -> Result<SubscriptionUpdateReport, Failure> {
        let _guard = self.begin_refresh(name)?;
        self.refresh_with_retry(source, name).await
    }

    /// DUAL-07-09: decide — and perform — the post-update core reload.
    ///
    /// * no new content, a profile that opted out, or a profile that is not
    ///   active: nothing is applied, and the report says exactly why;
    /// * no host reload seam: a typed [`CoreReloadOutcome::Unsupported`], never
    ///   a silent no-op;
    /// * the seam fails: a typed [`CoreReloadOutcome::Failed`] carrying the
    ///   adapter error, so the stored-but-not-live state is visible.
    async fn core_reload_outcome(
        &self,
        name: &str,
        changed: bool,
    ) -> Result<CoreReloadOutcome, Failure> {
        if !changed {
            return Ok(CoreReloadOutcome::NotAttempted);
        }
        let metadata = self.profile.load_metadata(name).await?;
        if !metadata.auto_reload_core {
            return Ok(CoreReloadOutcome::Disabled);
        }
        if self.profile.current_profile().await? != name {
            return Ok(CoreReloadOutcome::NotActive);
        }
        let Some(port) = &self.core_reload else {
            return Ok(CoreReloadOutcome::Unsupported);
        };
        Ok(match port.reload_active_profile().await {
            Ok(()) => CoreReloadOutcome::Reloaded,
            Err(error) => CoreReloadOutcome::Failed {
                error: error.to_string(),
            },
        })
    }

    /// Bounded-concurrency refresh of every profile carrying a subscription
    /// URL, with per-profile retry and single-flight. Mirrors the aggregation
    /// of [`ProfileApplication::update_all_subscriptions`] so both surfaces
    /// keep rendering the same [`SubscriptionBatchReport`].
    ///
    /// DUAL-07-09: at most one core reload happens per batch — only the active
    /// profile's own updated report carries the reload outcome.
    pub async fn refresh_all<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        concurrency: usize,
    ) -> Result<SubscriptionBatchReport, Failure> {
        let profiles = self.profile.list_profiles().await?;
        let total = profiles.len();
        let targets: Vec<String> = profiles
            .iter()
            .filter(|profile| {
                profile
                    .subscription_url
                    .as_deref()
                    .is_some_and(|url| !url.trim().is_empty())
            })
            .map(|profile| profile.name.clone())
            .collect();
        let mut report = SubscriptionBatchReport {
            total,
            skipped: total - targets.len(),
            ..Default::default()
        };

        let outcomes = stream::iter(targets.into_iter().map(|name| async move {
            let result = self.refresh_profile_inner(source, &name).await;
            (name, result)
        }))
        .buffer_unordered(concurrency.max(1))
        .collect::<Vec<_>>()
        .await;

        for (name, result) in outcomes {
            match result {
                Ok(outcome_report) => {
                    match &outcome_report.outcome {
                        SubscriptionUpdateOutcome::Updated { .. } => report.updated += 1,
                        SubscriptionUpdateOutcome::NotModified { .. } => report.not_modified += 1,
                        SubscriptionUpdateOutcome::Failed { .. } => report.failed += 1,
                    }
                    report.outcomes.push(outcome_report);
                }
                Err(failure) => {
                    report.failed += 1;
                    report.outcomes.push(SubscriptionUpdateReport {
                        profile_name: name,
                        outcome: SubscriptionUpdateOutcome::Failed {
                            error: failure.message,
                            attempts: 1,
                        },
                        etag: None,
                        last_modified: None,
                        quota: None,
                        usage_warning: false,
                        expiry_warning: false,
                        core_reload: CoreReloadOutcome::NotAttempted,
                        backed_up: false,
                    });
                }
            }
        }

        self.apply_batch_core_reload(&mut report).await?;

        if let Some(notifier) = &self.notifier {
            notifier.notify(notification_for_batch(&report));
        }

        Ok(report)
    }

    /// Stamp the single batch-wide reload decision onto the active profile's
    /// updated report (when the batch really updated it).
    async fn apply_batch_core_reload(
        &self,
        report: &mut SubscriptionBatchReport,
    ) -> Result<(), Failure> {
        let active = self.profile.current_profile().await?;
        let Some(outcome) = report.outcomes.iter_mut().find(|outcome| {
            outcome.profile_name == active
                && matches!(outcome.outcome, SubscriptionUpdateOutcome::Updated { .. })
        }) else {
            return Ok(());
        };
        outcome.core_reload = self.core_reload_outcome(&active, true).await?;
        Ok(())
    }

    async fn refresh_with_retry<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        name: &str,
    ) -> Result<SubscriptionUpdateReport, Failure> {
        let mut attempt = 0usize;
        loop {
            attempt += 1;
            match self
                .profile
                .update_subscription_conditional(source, name)
                .await
            {
                Ok(report) => return Ok(report),
                Err(failure) => match self.policy.delay_for_attempt(attempt) {
                    Some(delay) if !delay.is_zero() => self.runtime.sleep(delay).await,
                    Some(_) => {}
                    None => return Err(failure),
                },
            }
        }
    }
}

/// Map a single-profile refresh result to its locale-neutral notification.
fn notification_for_refresh(
    name: &str,
    result: &Result<SubscriptionUpdateReport, Failure>,
) -> SubscriptionNotification {
    match result {
        Ok(report) => {
            let kind = match &report.outcome {
                SubscriptionUpdateOutcome::Updated { .. } => SubscriptionNotificationKind::Updated,
                SubscriptionUpdateOutcome::NotModified { .. } => {
                    SubscriptionNotificationKind::NotModified
                }
                SubscriptionUpdateOutcome::Failed { .. } => SubscriptionNotificationKind::Failed,
            };
            if kind == SubscriptionNotificationKind::Failed {
                let error = match &report.outcome {
                    SubscriptionUpdateOutcome::Failed { error, .. } => error.clone(),
                    _ => String::new(),
                };
                SubscriptionNotification::failure(vec![name.to_string()], error)
            } else {
                SubscriptionNotification::for_profiles(kind, vec![name.to_string()])
            }
        }
        Err(failure) => {
            SubscriptionNotification::failure(vec![name.to_string()], failure.message.clone())
        }
    }
}

/// Map an aggregated batch to one locale-neutral notification.
fn notification_for_batch(report: &SubscriptionBatchReport) -> SubscriptionNotification {
    let profiles = report
        .outcomes
        .iter()
        .map(|outcome| outcome.profile_name.clone())
        .collect::<Vec<_>>();
    if report.failed > 0 {
        let error = report
            .outcomes
            .iter()
            .find_map(|outcome| match &outcome.outcome {
                SubscriptionUpdateOutcome::Failed { error, .. } => Some(error.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "subscription update failed".to_string());
        return SubscriptionNotification::failure(profiles, error);
    }
    let kind = if report.updated > 0 {
        SubscriptionNotificationKind::Updated
    } else {
        SubscriptionNotificationKind::NotModified
    };
    SubscriptionNotification::for_profiles(kind, profiles)
}

#[cfg(test)]
#[path = "subscription_refresh_application_test.rs"]
mod tests;
