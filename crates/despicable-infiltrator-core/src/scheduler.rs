use anyhow::anyhow;
use chrono::{Duration as ChronoDuration, Utc};
use log::{info, warn};
use mihomo_rs::config::{ConfigManager, Profile};
use reqwest::Client;
use std::sync::OnceLock;
use tokio::sync::{watch, Mutex};
use tokio::time::{interval, sleep, Duration, Instant};

use crate::{
    admin_api::AdminApiContext,
    config as core_config,
    subscription::{fetch_subscription_text, mask_subscription_url, strip_utf8_bom},
};

#[derive(Clone)]
pub struct SubscriptionScheduler {
    stop_tx: watch::Sender<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct SubscriptionUpdateSummary {
    pub total: usize,
    pub updated: usize,
    pub failed: usize,
    pub skipped: usize,
}

impl SubscriptionScheduler {
    pub fn start<C: AdminApiContext>(ctx: C) -> Self {
        let (stop_tx, mut stop_rx) = watch::channel(false);
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            let client = build_http_client();
            let raw_client = build_raw_http_client(&client);
            let mut ticker = interval(Duration::from_secs(3600));
            let mut last_tick = Instant::now();
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let now = Instant::now();
                        let drift = now.saturating_duration_since(last_tick);
                        last_tick = now;
                        if drift > Duration::from_secs(5400) {
                            warn!("subscription scheduler drift detected: {}s", drift.as_secs());
                        }
                        if let Err(err) = run_scheduler_tick(&ctx_clone, &client, &raw_client).await {
                            warn!("subscription scheduler tick failed: {err:#}");
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

    pub fn shutdown(&self) {
        let _ = self.stop_tx.send(true);
    }
}

fn update_lock() -> &'static Mutex<()> {
    static UPDATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    UPDATE_LOCK.get_or_init(|| Mutex::new(()))
}

pub async fn update_all_subscriptions<C: AdminApiContext>(
    ctx: &C,
) -> anyhow::Result<SubscriptionUpdateSummary> {
    let _guard = match update_lock().try_lock() {
        Ok(guard) => guard,
        Err(_) => return Err(anyhow!("订阅更新正在进行中，请稍后再试")),
    };
    let manager = ConfigManager::new()?;
    let profiles = manager.list_profiles().await?;
    let client = build_http_client();
    let raw_client = build_raw_http_client(&client);
    let now = Utc::now();
    let mut summary = SubscriptionUpdateSummary::default();
    summary.total = profiles.len();
    let mut rebuild_needed = false;

    for profile in profiles {
        let url = match profile.subscription_url.as_deref() {
            Some(url) if !url.trim().is_empty() => url.trim().to_string(),
            _ => {
                summary.skipped += 1;
                continue;
            }
        };

        let result = update_profile_subscription_with_retry(
            &manager,
            &profile,
            &url,
            profile.update_interval_hours,
            profile.auto_update_enabled,
            now,
            &client,
            &raw_client,
            3,
        )
        .await;

        match result {
            Ok(needs_rebuild) => {
                if needs_rebuild {
                    rebuild_needed = true;
                }
                summary.updated += 1;
                ctx.notify_subscription_update(profile.name.clone(), true, None)
                    .await;
            }
            Err(err) => {
                summary.failed += 1;
                ctx.notify_subscription_update(
                    profile.name.clone(),
                    false,
                    Some(err.to_string()),
                )
                .await;
            }
        }
    }

    if rebuild_needed {
        if let Err(err) = ctx.rebuild_runtime().await {
            warn!("subscription batch rebuild failed: {err:#}");
        }
    }

    Ok(summary)
}

async fn run_scheduler_tick<C: AdminApiContext>(
    ctx: &C,
    client: &Client,
    raw_client: &Client,
) -> anyhow::Result<()> {
    let _guard = match update_lock().try_lock() {
        Ok(guard) => guard,
        Err(_) => {
            warn!("subscription update already running, skip scheduler tick");
            return Ok(());
        }
    };
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
            &manager,
            &profile,
            &url,
            interval_hours,
            true,
            now,
            client,
            raw_client,
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
    if rebuild_needed {
        if let Err(err) = ctx.rebuild_runtime().await {
            warn!("subscription scheduler rebuild failed: {err:#}");
        }
    }
    Ok(())
}

async fn update_profile_subscription(
    manager: &ConfigManager,
    profile: &Profile,
    url: &str,
    interval_hours: Option<u32>,
    auto_update_enabled: bool,
    now: chrono::DateTime<Utc>,
    client: &Client,
    raw_client: &Client,
) -> anyhow::Result<bool> {
    info!(
        "subscription update: profile={} url={}",
        profile.name,
        mask_subscription_url(url)
    );
    let content = fetch_subscription_text(client, raw_client, url).await?;
    let content = strip_utf8_bom(&content);
    if core_config::validate_yaml(&content).is_err() {
        return Err(anyhow!("订阅内容不是有效的 YAML"));
    }
    manager
        .save(&profile.name, &content)
        .await
        .map_err(|err| anyhow!(err.to_string()))?;

    let next_update = if auto_update_enabled {
        interval_hours.map(|hours| now + ChronoDuration::hours(hours as i64))
    } else {
        None
    };
    let mut updated = profile.clone();
    updated.subscription_url = Some(url.to_string());
    updated.auto_update_enabled = auto_update_enabled;
    updated.update_interval_hours = interval_hours;
    updated.last_updated = Some(now);
    updated.next_update = next_update;
    manager.update_profile_metadata(&profile.name, &updated).await?;

    Ok(profile.active)
}

async fn update_profile_subscription_with_retry(
    manager: &ConfigManager,
    profile: &Profile,
    url: &str,
    interval_hours: Option<u32>,
    auto_update_enabled: bool,
    now: chrono::DateTime<Utc>,
    client: &Client,
    raw_client: &Client,
    max_attempts: usize,
) -> anyhow::Result<bool> {
    let mut attempt = 0usize;
    let mut delay = Duration::from_secs(2);
    loop {
        attempt += 1;
        match update_profile_subscription(
            manager,
            profile,
            url,
            interval_hours,
            auto_update_enabled,
            now,
            client,
            raw_client,
        )
        .await
        {
            Ok(needs_rebuild) => return Ok(needs_rebuild),
            Err(err) => {
                if attempt >= max_attempts {
                    return Err(err);
                }
                warn!(
                    "subscription update retry: profile={} attempt={} err={:#}",
                    profile.name, attempt, err
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

async fn schedule_next_attempt(
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

fn build_http_client() -> Client {
    Client::builder()
        .user_agent("Mihomo-Despicable-Infiltrator")
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|err| {
            warn!("failed to build scheduler http client: {err}");
            Client::new()
        })
}

fn build_raw_http_client(default_client: &Client) -> Client {
    Client::builder()
        .user_agent("Mihomo-Despicable-Infiltrator")
        .timeout(Duration::from_secs(30))
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .build()
        .unwrap_or_else(|err| {
            warn!("failed to build scheduler raw http client: {err}");
            default_client.clone()
        })
}
