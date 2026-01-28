use anyhow::anyhow;
use chrono::{Duration as ChronoDuration, Utc};
use log::{info, warn};
use mihomo_config::{ConfigManager, Profile};
use infiltrator_http::HttpClient;
use tokio::time::{sleep, Duration};
use tokio::task::JoinSet;

use crate::admin_api::AdminApiContext;
use infiltrator_core::{
    config as core_config,
    subscription::{fetch_subscription_text, mask_subscription_url, strip_utf8_bom},
};

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

pub(super) async fn run_subscription_tick<C: AdminApiContext>(
    ctx: &C,
    client: &HttpClient,
    raw_client: &HttpClient,
) -> anyhow::Result<bool> {
    let manager = ConfigManager::new()?;
    let profiles = manager.list_profiles().await?;
    let now = Utc::now();
    let mut rebuild_needed = false;
    for profile in profiles {
        if !profile.auto_update_enabled {
            continue;
        }
        let url = match profile.subscription_url.as_deref() {
            Some(url) if !url.trim().is_empty() => url.trim().to_string(),
            _ => continue,
        };
        let interval_hours = match profile.update_interval_hours {
            Some(hours) if hours > 0 => Some(hours),
            _ => continue,
        };
        let due = profile.next_update.map(|next| next <= now).unwrap_or(true);
        if !due {
            continue;
        }

        let result = update_profile_subscription_with_retry(
            ProfileUpdateParams {
                manager: &manager,
                profile: &profile,
                url: &url,
                interval_hours,
                auto_update_enabled: true,
                now,
                client,
                raw_client,
            },
            3,
        )
        .await;
        match result {
            Ok(needs_rebuild) => {
                if needs_rebuild {
                    rebuild_needed = true;
                }
                ctx.notify_subscription_update(profile.name.clone(), true, None)
                    .await;
            }
            Err(err) => {
                warn!(
                    "subscription update failed: profile={} url={} err={:#}",
                    profile.name,
                    mask_subscription_url(&url),
                    err
                );
                ctx.notify_subscription_update(
                    profile.name.clone(),
                    false,
                    Some(err.to_string()),
                )
                .await;
                if let Some(hours) = interval_hours {
                    let _ = schedule_next_attempt(&manager, &profile, hours, now).await;
                }
            }
        }
    }
    Ok(rebuild_needed)
}

pub async fn update_all_subscriptions<C: AdminApiContext>(
    ctx: &C,
    client: &HttpClient,
    raw_client: &HttpClient,
) -> anyhow::Result<SubscriptionUpdateSummary> {
    let manager = ConfigManager::new()?;
    let profiles = manager.list_profiles().await?;
    let now = Utc::now();
    let mut summary = SubscriptionUpdateSummary {
        total: profiles.len(),
        ..Default::default()
    };
    let mut rebuild_needed = false;

    // Collect profiles with subscription URLs
    let profiles_to_update: Vec<(String, Profile, Option<u32>, bool)> = profiles
        .into_iter()
        .filter_map(|profile| {
            if let Some(url) = profile.subscription_url.as_deref()
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty()) 
            {
                Some((
                    url,
                    profile.clone(),
                    profile.update_interval_hours,
                    profile.auto_update_enabled
                ))
            } else {
                summary.skipped += 1;
                None
            }
        })
        .collect();

    if profiles_to_update.is_empty() {
        return Ok(summary);
    }

    info!("starting parallel subscription update for {} profiles", profiles_to_update.len());

    // Use JoinSet for parallel updates with limited concurrency
    let max_concurrent = 5usize;
    let mut join_set: JoinSet<anyhow::Result<SubscriptionUpdateResult>> = JoinSet::new();

    for (url, profile, interval_hours, auto_update_enabled) in profiles_to_update {
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
                        warn!("subscription update failed: {err}");
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

        let client_clone = client.clone();
        let raw_client_clone = raw_client.clone();
        let profile_name = profile.name.clone();
        let profile_for_task = profile.clone();
        let interval_for_task = interval_hours;
        let auto_update_for_task = auto_update_enabled;

        join_set.spawn(async move {
            let result = update_profile_subscription_with_retry(
                ProfileUpdateParams {
                    manager: &ConfigManager::new()?,
                    profile: &profile_for_task,
                    url: &url,
                    interval_hours: interval_for_task,
                    auto_update_enabled: auto_update_for_task,
                    now,
                    client: &client_clone,
                    raw_client: &raw_client_clone,
                },
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
                        "subscription update failed: profile={} url={} err={:#}",
                        profile_name,
                        mask_subscription_url(&url),
                        err
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
                ctx.notify_subscription_update(
                    update_result.profile_name.clone(),
                    true,
                    None,
                )
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

    if rebuild_needed
        && let Err(err) = ctx.rebuild_runtime().await {
            warn!("subscription batch rebuild failed: {err:#}");
        }

    Ok(summary)
}

struct ProfileUpdateParams<'a> {
    manager: &'a ConfigManager,
    profile: &'a Profile,
    url: &'a str,
    interval_hours: Option<u32>,
    auto_update_enabled: bool,
    now: chrono::DateTime<Utc>,
    client: &'a HttpClient,
    raw_client: &'a HttpClient,
}

async fn update_profile_subscription(
    params: ProfileUpdateParams<'_>,
) -> anyhow::Result<bool> {
    info!(
        "subscription update: profile={} url={}",
        params.profile.name,
        mask_subscription_url(params.url)
    );
    let content = fetch_subscription_text(params.client, params.raw_client, params.url).await?;
    let content = strip_utf8_bom(&content);
    if core_config::validate_yaml(&content).is_err() {
        return Err(anyhow!("订阅内容不是有效的 YAML"));
    }
    params.manager
        .save(&params.profile.name, &content)
        .await
        .map_err(|err| anyhow!(err.to_string()))?;

    let next_update = if params.auto_update_enabled {
        params.interval_hours.map(|hours| params.now + ChronoDuration::hours(hours as i64))
    } else {
        None
    };
    let mut updated = params.profile.clone();
    updated.subscription_url = Some(params.url.to_string());
    updated.auto_update_enabled = params.auto_update_enabled;
    updated.update_interval_hours = params.interval_hours;
    updated.last_updated = Some(params.now);
    updated.next_update = next_update;
    params.manager.update_profile_metadata(&params.profile.name, &updated).await?;

    Ok(params.profile.active)
}

async fn update_profile_subscription_with_retry(
    params: ProfileUpdateParams<'_>,
    max_attempts: usize,
) -> anyhow::Result<bool> {
    let mut attempt = 0usize;
    let mut delay = Duration::from_secs(2);
    loop {
        attempt += 1;
        let retry_params = ProfileUpdateParams {
            manager: params.manager,
            profile: params.profile,
            url: params.url,
            interval_hours: params.interval_hours,
            auto_update_enabled: params.auto_update_enabled,
            now: params.now,
            client: params.client,
            raw_client: params.raw_client,
        };
        match update_profile_subscription(retry_params).await
        {
            Ok(needs_rebuild) => return Ok(needs_rebuild),
            Err(err) => {
                if attempt >= max_attempts {
                    return Err(err);
                }
                warn!(
                    "subscription update retry: profile={} attempt={} err={:#}",
                    params.profile.name, attempt, err
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
    manager: &ConfigManager,
    profile: &Profile,
    interval_hours: u32,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<()> {
    let next_update = now + ChronoDuration::hours(interval_hours as i64);
    let mut updated = profile.clone();
    updated.next_update = Some(next_update);
    manager.update_profile_metadata(&profile.name, &updated).await?;
    Ok(())
}
