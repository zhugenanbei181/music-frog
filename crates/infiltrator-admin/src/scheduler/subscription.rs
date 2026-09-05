use anyhow::anyhow;
use chrono::Utc;
use infiltrator_application::profile_application::ProfileApplication;
use log::{info, warn};
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_ports::subscription_source::SubscriptionSource;
use std::sync::Arc;
use tokio::task::JoinSet;
use tokio::time::{Duration, sleep};

use crate::admin_api::state::AdminApiContext;
use infiltrator_domain::redact::redact_line;
use infiltrator_domain::subscription::mask_subscription_url;

#[derive(Clone, Debug, Default)]
pub struct SubscriptionUpdateSummary {
    pub total: usize,
    pub updated: usize,
    pub failed: usize,
    pub skipped: usize,
}

pub(crate) struct SubscriptionUpdateResult {
    profile_name: String,
    needs_rebuild: bool,
}

/// Periodic tick for one profile's subscription auto-update job on the
/// unified host [`JobScheduler`](super::job_scheduler::JobScheduler).
///
/// The update logic itself is untouched; this only replaces the trigger
/// shell of the former hourly sweep. Metadata is re-read on every run so a
/// run already in flight still respects the latest state, and the former
/// due-check (`next_update` reached) is kept: the immediate first run of a
/// freshly scheduled job is therefore a no-op until the configured interval
/// has elapsed. Errors are returned as readable strings so the scheduler
/// can count and record them (`failure_count` / `last_error`).
pub(super) async fn run_profile_subscription_tick<C: AdminApiContext>(
    ctx: &C,
    profile_name: &str,
) -> Result<(), String> {
    let application = ctx
        .profile_application()
        .await
        .map_err(|error| format!("打开 profile application 失败: {error}"))?;
    let profile = application
        .load_profile_info(profile_name)
        .await
        .map_err(|failure| format!("读取 profile `{profile_name}` 元数据失败: {}", failure.message))?;
    // The job is canceled as soon as auto-update is switched off; treat an
    // in-flight run that observes the new state as done.
    if !profile.auto_update_enabled {
        return Ok(());
    }
    match profile.subscription_url.as_deref() {
        Some(url) if !url.trim().is_empty() => {}
        _ => return Ok(()),
    }
    let interval_hours = match profile.update_interval_hours {
        Some(hours) if hours > 0 => hours,
        _ => return Ok(()),
    };
    let now = Utc::now();
    let due = profile.next_update.map(|next| next <= now).unwrap_or(true);
    if !due {
        return Ok(());
    }

    let source = ctx
        .subscription_source()
        .await
        .map_err(|error| format!("打开订阅 source 失败: {error}"))?;
    match update_profile_subscription_with_retry(&application, source.as_ref(), &profile.name, 3)
        .await
    {
        Ok(needs_rebuild) => {
            if needs_rebuild && let Err(err) = ctx.rebuild_runtime().await {
                warn!(
                    "subscription rebuild failed: profile={} err={}",
                    profile.name,
                    redact_line(&format!("{err:#}"), &[])
                );
            }
            ctx.notify_subscription_update(profile.name.clone(), true, None)
                .await;
            Ok(())
        }
        Err(err) => {
            ctx.notify_subscription_update(
                profile.name.clone(),
                false,
                Some(redact_line(&err.to_string(), &[])),
            )
            .await;
            let _ = schedule_next_attempt(&application, &profile.name, interval_hours, now).await;
            // Redacted: this string becomes the JobScheduler's last_error and
            // admin-facing status text; anyhow chains can embed the full
            // request URL including its subscription token.
            Err(redact_line(&format!("{err:#}"), &[]))
        }
    }
}

pub async fn update_all_subscriptions<C: AdminApiContext>(
    ctx: &C,
) -> anyhow::Result<SubscriptionUpdateSummary> {
    let application = ctx.profile_application().await?;
    let profiles = application
        .list_profiles()
        .await
        .map_err(|failure| anyhow!(failure.message))?;
    let mut summary = SubscriptionUpdateSummary {
        total: profiles.len(),
        ..Default::default()
    };
    let mut rebuild_needed = false;

    // Collect profiles with subscription URLs
    let profiles_to_update: Vec<ProfileInfo> = profiles
        .into_iter()
        .filter_map(|profile| {
            if let Some(url) = profile
                .subscription_url
                .as_deref()
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty())
            {
                let _ = url;
                Some(profile)
            } else {
                summary.skipped += 1;
                None
            }
        })
        .collect();

    if profiles_to_update.is_empty() {
        return Ok(summary);
    }

    info!(
        "starting parallel subscription update for {} profiles",
        profiles_to_update.len()
    );

    // Use JoinSet for parallel updates with limited concurrency
    let max_concurrent = 5usize;
    let mut join_set: JoinSet<anyhow::Result<SubscriptionUpdateResult>> = JoinSet::new();

    let source = ctx.subscription_source().await?;
    for profile in profiles_to_update {
        // Wait for available slot if we've reached max concurrency
        while join_set.len() >= max_concurrent {
            if let Some(result) = join_set.join_next().await {
                match result {
                    Ok(Ok(update_result)) => {
                        if update_result.needs_rebuild {
                            rebuild_needed = true;
                        }
                        summary.updated += 1;
                        ctx.notify_subscription_update(
                            update_result.profile_name.clone(),
                            true,
                            None,
                        )
                        .await;
                    }
                    Ok(Err(err)) => {
                        // Task failed with an error (not a panic)
                        warn!(
                            "subscription update failed: {}",
                            redact_line(&format!("{err:#}"), &[])
                        );
                        summary.failed += 1;
                    }
                    Err(join_err) => {
                        // Task panicked
                        warn!("subscription update task panicked: {join_err}");
                        summary.failed += 1;
                    }
                }
            }
        }

        let profile_name = profile.name.clone();
        let application_for_task = application.clone();
        let source_for_task = Arc::clone(&source);

        join_set.spawn(async move {
            let result = update_profile_subscription_with_retry(
                &application_for_task,
                source_for_task.as_ref(),
                &profile_name,
                3,
            )
            .await;

            match result {
                Ok(needs_rebuild) => Ok(SubscriptionUpdateResult {
                    profile_name: profile_name.clone(),
                    needs_rebuild,
                }),
                Err(err) => {
                    warn!(
                        "subscription update failed: profile={} url={} err={}",
                        profile_name,
                        // mask shortens the path; redact strips query tokens
                        // and any credentials the error Display embedded.
                        redact_line(
                            &mask_subscription_url(
                                profile
                                    .subscription_url
                                    .as_deref()
                                    .unwrap_or_default(),
                            ),
                            &[],
                        ),
                        redact_line(&format!("{err:#}"), &[])
                    );
                    Err(err)
                }
            }
        });
    }

    // Wait for remaining tasks
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(update_result)) => {
                if update_result.needs_rebuild {
                    rebuild_needed = true;
                }
                summary.updated += 1;
                ctx.notify_subscription_update(update_result.profile_name.clone(), true, None)
                    .await;
            }
            Ok(Err(err)) => {
                warn!("subscription update task panicked: {err}");
                summary.failed += 1;
            }
            Err(join_err) => {
                warn!("subscription update task join error: {join_err}");
                summary.failed += 1;
            }
        }
    }

    if rebuild_needed && let Err(err) = ctx.rebuild_runtime().await {
        warn!(
            "subscription batch rebuild failed: {}",
            redact_line(&format!("{err:#}"), &[])
        );
    }

    Ok(summary)
}

async fn update_profile_subscription(
    application: &ProfileApplication,
    source: &dyn SubscriptionSource,
    profile_name: &str,
) -> anyhow::Result<bool> {
    let profile = application
        .load_profile_info(profile_name)
        .await
        .map_err(|failure| anyhow!(failure.message))?;
    let url = profile.subscription_url.as_deref().unwrap_or_default();
    info!(
        "subscription update: profile={} url={}",
        profile.name,
        mask_subscription_url(url)
    );
    let updated = application
        .update_subscription(source, profile_name)
        .await
        .map_err(|failure| anyhow!(failure.message))?;
    Ok(updated.active)
}

async fn update_profile_subscription_with_retry(
    application: &ProfileApplication,
    source: &dyn SubscriptionSource,
    profile_name: &str,
    max_attempts: usize,
) -> anyhow::Result<bool> {
    let mut attempt = 0usize;
    let mut delay = Duration::from_secs(2);
    loop {
        attempt += 1;
        match update_profile_subscription(application, source, profile_name).await {
            Ok(needs_rebuild) => return Ok(needs_rebuild),
            Err(err) => {
                if attempt >= max_attempts {
                    return Err(err);
                }
                warn!(
                    "subscription update retry: profile={} attempt={} err={:#}",
                    profile_name, attempt, err
                );
                sleep(delay).await;
                delay = delay
                    .checked_mul(2)
                    .unwrap_or(delay)
                    .min(Duration::from_secs(30));
            }
        }
    }
}

pub(crate) async fn schedule_next_attempt(
    application: &ProfileApplication,
    profile_name: &str,
    interval_hours: u32,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<()> {
    let next_update = now + chrono::Duration::hours(interval_hours as i64);
    let mut updated = application
        .load_metadata(profile_name)
        .await
        .map_err(|failure| anyhow!(failure.message))?;
    updated.next_update = Some(next_update);
    application
        .update_metadata(profile_name, &updated)
        .await
        .map_err(|failure| anyhow!(failure.message))?;
    Ok(())
}
