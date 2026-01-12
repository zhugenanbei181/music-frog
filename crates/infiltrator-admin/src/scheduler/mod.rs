use std::sync::OnceLock;

use log::warn;
use tokio::sync::{Mutex, watch};
use tokio::time::{interval, Duration, Instant};

use infiltrator_http::{build_http_client, build_raw_http_client};

use crate::admin_api::AdminApiContext;
use self::subscription::run_subscription_tick;
use self::sync::run_sync_tick;

pub mod subscription;
pub mod sync;

#[derive(Clone)]
pub struct SubscriptionScheduler {
    stop_tx: watch::Sender<bool>,
}

impl SubscriptionScheduler {
    pub fn start<C: AdminApiContext>(ctx: C) -> Self {
        let (stop_tx, mut stop_rx) = watch::channel(false);
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            let client = build_http_client();
            let raw_client = build_raw_http_client(&client);
            
            // 提高检查频率至 1 分钟，以便处理不同频率的定时任务
            let mut ticker = interval(Duration::from_secs(60));
            // Avoid Instant underflow on early boot; force the first tick instead.
            let initial_backfill = Duration::from_secs(3600);
            let now = Instant::now();
            let initial_instant = now.checked_sub(initial_backfill);
            let (mut last_sub_update, mut force_sub_update) = match initial_instant {
                Some(instant) => (instant, false),
                None => (now, true),
            };
            let (mut last_sync_update, mut force_sync_update) = match initial_instant {
                Some(instant) => (instant, false),
                None => (now, true),
            };
            
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let settings = ctx_clone.get_app_settings().await;

                        // 锁保护，防止多个调度任务重叠
                        let _guard = match update_lock().try_lock() {
                            Ok(guard) => guard,
                            Err(_) => continue,
                        };

                        // 1. 订阅更新 (每小时一次)
                        if force_sub_update || last_sub_update.elapsed() >= Duration::from_secs(3600) {
                            match run_subscription_tick(&ctx_clone, &client, &raw_client).await {
                                Ok(rebuild_needed) => {
                                    if rebuild_needed {
                                        let _ = ctx_clone.rebuild_runtime().await;
                                    }
                                    last_sub_update = Instant::now();
                                    force_sub_update = false;
                                }
                                Err(err) => warn!("subscription scheduler failed: {err:#}"),
                            }
                        }

                        // 2. WebDAV 同步
                        if settings.webdav.enabled {
                            let interval = Duration::from_secs(settings.webdav.sync_interval_mins as u64 * 60);
                            if force_sync_update || last_sync_update.elapsed() >= interval {
                                match run_sync_tick(&ctx_clone, &settings.webdav).await {
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

    pub fn shutdown(&self) {
        let _ = self.stop_tx.send(true);
    }
}

pub(crate) fn update_lock() -> &'static Mutex<()> {
    static UPDATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    UPDATE_LOCK.get_or_init(|| Mutex::new(()))
}
