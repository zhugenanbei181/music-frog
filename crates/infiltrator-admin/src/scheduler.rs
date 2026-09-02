//! Admin-side scheduling built on the unified core [`JobScheduler`].
//!
//! Subscription auto-update is a set of per-profile periodic jobs: every
//! profile with auto-update enabled owns one job named
//! `subscription-update-{profile}` whose interval comes from the profile's
//! `update_interval_hours`. The jobs run on a process-wide
//! [`JobScheduler`] registry ([`subscription_jobs`]) so the admin API
//! handlers can keep them in lockstep with profile metadata through
//! [`sync_profile_job`]: enabling spawns (or replaces, which covers interval
//! changes) the job, disabling or deleting the profile cancels it. Each job
//! calls the pre-existing per-profile update logic in
//! [`subscription::run_profile_subscription_tick`]; single-flight execution
//! and failure counters (run_count / failure_count / last_error, surfaced by
//! [`JobScheduler::snapshot`] and logged per failure by the core scheduler)
//! come for free.
//!
//! The one-shot actions (manual `update-now`, bulk update, WebDAV sync-now)
//! stay outside the registry: they are explicit one-time requests, not
//! periodic jobs. The only thing still running on the legacy minute ticker
//! is the optional WebDAV periodic sync, whose enablement and interval come
//! from app settings rather than profile metadata.
//!
//! [`SubscriptionScheduler::start`] must be called from within a tokio
//! runtime (the admin embedders do this from their async setup): it seeds
//! one job per already-enabled profile and keeps the WebDAV ticker alive.

use std::sync::OnceLock;
use std::time::Duration;

use infiltrator_core::scheduler::JobScheduler;
use infiltrator_http::{HttpClient, build_http_client, build_raw_http_client};
use log::warn;
use tokio::sync::watch;
use tokio::time::{Instant, interval};

use self::sync::run_sync_tick;
use crate::admin_api::state::AdminApiContext;
use crate::support::app_config_manager;

pub mod subscription;
pub mod sync;

#[cfg(test)]
mod subscription_test;

/// Name prefix of every per-profile subscription auto-update job.
const SUBSCRIPTION_JOB_PREFIX: &str = "subscription-update-";

/// Process-wide registry for the per-profile subscription jobs.
pub(crate) fn subscription_jobs() -> &'static JobScheduler {
    static SUBSCRIPTION_JOBS: OnceLock<JobScheduler> = OnceLock::new();
    SUBSCRIPTION_JOBS.get_or_init(JobScheduler::new)
}

/// Job name for one profile's subscription auto-update.
pub(crate) fn subscription_job_name(profile_name: &str) -> String {
    format!("{SUBSCRIPTION_JOB_PREFIX}{profile_name}")
}

/// Shared HTTP clients for the subscription job closures, built once.
fn subscription_http_clients() -> &'static (HttpClient, HttpClient) {
    static CLIENTS: OnceLock<(HttpClient, HttpClient)> = OnceLock::new();
    CLIENTS.get_or_init(|| {
        let client = build_http_client();
        let raw_client = build_raw_http_client(&client);
        (client, raw_client)
    })
}

/// Register (or replace) the periodic update job of one profile.
///
/// `interval` is the tick cadence; production callers derive it from the
/// profile's `update_interval_hours` via [`sync_profile_job`]. Tests inject
/// a small interval to exercise the enable -> fire -> disable cycle. Same
/// names replace the previous job, so re-scheduling after an interval change
/// is simply a spawn.
pub(crate) fn schedule_profile_update_job<C: AdminApiContext>(
    ctx: &C,
    profile_name: &str,
    interval: Duration,
) {
    debug_assert!(interval > Duration::ZERO, "job interval must be non-zero");
    let name = subscription_job_name(profile_name);
    let (client, raw_client) = &subscription_http_clients();
    let ctx = ctx.clone();
    let profile = profile_name.to_string();
    let client = client.clone();
    let raw_client = raw_client.clone();
    subscription_jobs().spawn_job(&name, interval, move || {
        let ctx = ctx.clone();
        let profile = profile.clone();
        let client = client.clone();
        let raw_client = raw_client.clone();
        async move {
            subscription::run_profile_subscription_tick(&ctx, &profile, &client, &raw_client).await
        }
    });
    log::info!("scheduled subscription auto-update job `{name}` (interval {interval:?})");
}

/// Cancel the periodic update job of one profile. Returns whether a job was
/// registered.
pub(crate) fn cancel_profile_update_job(profile_name: &str) -> bool {
    let name = subscription_job_name(profile_name);
    let canceled = subscription_jobs().cancel(&name);
    if canceled {
        log::info!("canceled subscription auto-update job `{name}`");
    }
    canceled
}

/// Bring the periodic update job of one profile in sync with its stored
/// subscription metadata. Called from the admin API handlers after every
/// metadata mutation (enable / disable / interval change / import / clear).
pub(crate) fn sync_profile_job<C: AdminApiContext>(
    ctx: &C,
    profile_name: &str,
    auto_update_enabled: bool,
    subscription_url: Option<&str>,
    update_interval_hours: Option<u32>,
) {
    let enabled = auto_update_enabled
        && subscription_url.is_some_and(|url| !url.trim().is_empty())
        && update_interval_hours.is_some_and(|hours| hours > 0);
    if !enabled {
        cancel_profile_update_job(profile_name);
        return;
    }
    let hours = u64::from(update_interval_hours.unwrap_or_default());
    schedule_profile_update_job(ctx, profile_name, Duration::from_secs(hours * 3600));
}

/// Cancel every per-profile subscription job (used when all profiles are
/// reset to defaults).
pub(crate) fn cancel_all_profile_jobs() {
    let jobs = subscription_jobs();
    for snapshot in jobs.snapshot() {
        if snapshot.name.starts_with(SUBSCRIPTION_JOB_PREFIX) {
            jobs.cancel(&snapshot.name);
        }
    }
}

/// Register a job for every profile that currently has auto-update enabled.
/// Runs at admin server startup so profiles configured before this boot keep
/// updating; later metadata changes flow through [`sync_profile_job`].
pub(crate) async fn seed_subscription_jobs<C: AdminApiContext>(ctx: &C) {
    let manager = match app_config_manager().await {
        Ok(manager) => manager,
        Err(err) => {
            warn!("subscription job seed failed to open config manager: {err}");
            return;
        }
    };
    let profiles = match manager.list_profiles().await {
        Ok(profiles) => profiles,
        Err(err) => {
            warn!("subscription job seed failed to list profiles: {err:#}");
            return;
        }
    };
    for profile in profiles {
        sync_profile_job(
            ctx,
            &profile.name,
            profile.auto_update_enabled,
            profile.subscription_url.as_deref(),
            profile.update_interval_hours,
        );
    }
    // Observability: dump the fresh registry once after seeding; ongoing
    // failures are logged per run by the core scheduler.
    for snapshot in subscription_jobs().snapshot() {
        match &snapshot.last_error {
            Some(err) => warn!(
                "subscription job `{}` active with failures={} last_error={err}",
                snapshot.name, snapshot.failure_count
            ),
            None => log::info!(
                "subscription job `{}` active (runs={})",
                snapshot.name,
                snapshot.run_count
            ),
        }
    }
}

#[derive(Clone)]
pub struct SubscriptionScheduler {
    stop_tx: watch::Sender<bool>,
}

impl SubscriptionScheduler {
    /// Seed the per-profile subscription jobs and start the WebDAV periodic
    /// sync ticker. Must be called from within a tokio runtime.
    pub fn start<C: AdminApiContext>(ctx: C) -> Self {
        let seed_ctx = ctx.clone();
        tokio::spawn(async move {
            seed_subscription_jobs(&seed_ctx).await;
        });

        let (stop_tx, mut stop_rx) = watch::channel(false);
        let ctx_clone = ctx;
        tokio::spawn(async move {
            // 提高检查频率至 1 分钟，以便处理不同频率的定时任务
            let mut ticker = interval(Duration::from_secs(60));
            // Avoid Instant underflow on early boot; force the first tick instead.
            let initial_backfill = Duration::from_secs(3600);
            let now = Instant::now();
            let (mut last_sync_update, mut force_sync_update) =
                match now.checked_sub(initial_backfill) {
                    Some(instant) => (instant, false),
                    None => (now, true),
                };

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let settings = ctx_clone.get_app_settings().await;
                        if settings.webdav.enabled {
                            let interval = Duration::from_secs(settings.webdav.sync_interval_mins as u64 * 60);
                            if force_sync_update || last_sync_update.elapsed() >= interval {
                                match run_sync_tick(
                                    &ctx_clone,
                                    &settings.webdav,
                                    settings.configs_dir.as_deref(),
                                )
                                .await
                                {
                                    Ok(summary) => {
                                        if summary.total_actions > 0 {
                                            log::info!("webdav sync: {} success, {} failed", summary.success_count, summary.failed_count);
                                        }
                                    }
                                    Err(err) => warn!("webdav sync scheduler failed: {err:#}"),
                                }
                                last_sync_update = Instant::now();
                                force_sync_update = false;
                            }
                        }
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });
        Self { stop_tx }
    }

    /// Stop the WebDAV ticker and cancel every per-profile subscription job.
    pub fn shutdown(&self) {
        let _ = self.stop_tx.send(true);
        subscription_jobs().cancel_all();
    }
}
