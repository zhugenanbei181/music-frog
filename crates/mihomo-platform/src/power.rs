#[path = "self_healing_pipeline.rs"]
mod self_healing_pipeline;

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch, Mutex};
use tokio::time::{self, Instant};

/// Power event representing system power changes, sleep, wake, battery state,
/// screen state, hibernation, and AC power transitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerEvent {
    Suspend,
    Resume,
    BatteryLow,
    AcPowerChanged,
    Sleep,
    Wake,
    TimerGapDetected { gap_ms: u64 },
    Hibernate,
    ScreenLocked,
    ScreenUnlocked,
}

impl PowerEvent {
    pub fn is_resume(&self) -> bool {
        matches!(
            self,
            PowerEvent::Resume | PowerEvent::Wake | PowerEvent::TimerGapDetected { .. }
        )
    }

    pub fn is_suspend(&self) -> bool {
        matches!(
            self,
            PowerEvent::Suspend | PowerEvent::Sleep | PowerEvent::Hibernate
        )
    }

    /// Determines whether the event requires resetting stale TCP connections and flushing DNS caches.
    pub fn requires_connection_reset(&self) -> bool {
        self.is_resume() || self.is_suspend()
    }
}

/// Watches for system power events using heuristic timer gap detection
/// and explicit event broadcasting.
pub struct PowerEventWatcher {
    sender: broadcast::Sender<PowerEvent>,
    stop_tx: watch::Sender<bool>,
    stop_rx: watch::Receiver<bool>,
    check_interval: Duration,
    gap_threshold: Duration,
}

impl Default for PowerEventWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerEventWatcher {
    /// Creates a new PowerEventWatcher with default 1000ms tick interval and 3000ms gap threshold.
    pub fn new() -> Self {
        Self::with_config(Duration::from_millis(1000), Duration::from_millis(3000))
    }

    /// Creates a new PowerEventWatcher with custom interval and gap threshold.
    pub fn with_config(check_interval: Duration, gap_threshold: Duration) -> Self {
        let (sender, _) = broadcast::channel(32);
        let (stop_tx, stop_rx) = watch::channel(false);
        Self {
            sender,
            stop_tx,
            stop_rx,
            check_interval,
            gap_threshold,
        }
    }

    /// Starts the watcher background task and returns a receiver for power events.
    pub fn start(&self) -> broadcast::Receiver<PowerEvent> {
        let rx = self.sender.subscribe();
        let sender = self.sender.clone();
        let mut stop_rx = self.stop_rx.clone();
        let interval_duration = self.check_interval;
        let gap_threshold_ms = self.gap_threshold.as_millis() as u64;

        tokio::spawn(async move {
            let mut interval = time::interval(interval_duration);
            interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            interval.tick().await;

            let mut last_tick = Instant::now();

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let now = Instant::now();
                        let gap = now.duration_since(last_tick).as_millis() as u64;

                        if gap > gap_threshold_ms {
                            let _ = sender.send(PowerEvent::Suspend);
                            let _ = sender.send(PowerEvent::Sleep);
                            let _ = sender.send(PowerEvent::TimerGapDetected { gap_ms: gap });
                            let _ = sender.send(PowerEvent::Resume);
                            let _ = sender.send(PowerEvent::Wake);
                        }

                        last_tick = now;
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        rx
    }

    /// Emits a power event manually into the broadcast channel.
    pub fn emit(
        &self,
        event: PowerEvent,
    ) -> Result<usize, broadcast::error::SendError<PowerEvent>> {
        self.sender.send(event)
    }

    /// Subscribes to power events.
    pub fn subscribe(&self) -> broadcast::Receiver<PowerEvent> {
        self.sender.subscribe()
    }

    /// Returns a clone of the underlying sender.
    pub fn sender(&self) -> broadcast::Sender<PowerEvent> {
        self.sender.clone()
    }

    /// Stops the watcher background task.
    pub fn stop(&self) {
        let _ = self.stop_tx.send(true);
    }
}

pub type ProbeFn = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;
pub type RecoveryFn = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;
pub type ResetConnectionsFn = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub type ZombiePurgeFn = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync>;
pub type FakeIpProbeFn = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<bool, String>> + Send>> + Send + Sync>;
pub type NodeRetestFn = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<u32, String>> + Send>> + Send + Sync>;
pub type ConfigReloadFn = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync>;
pub type ProcessRespawnFn = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync>;

/// Event emitted when the controller API is found to be dead/unresponsive after a resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfHealingTrigger {
    pub timestamp_secs: u64,
    pub reason: String,
    pub attempts: u32,
}

impl SelfHealingTrigger {
    pub fn new(reason: impl Into<String>) -> Self {
        let timestamp_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            timestamp_secs,
            reason: reason.into(),
            attempts: 1,
        }
    }

    pub fn with_attempts(reason: impl Into<String>, attempts: u32) -> Self {
        let timestamp_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            timestamp_secs,
            reason: reason.into(),
            attempts,
        }
    }
}

/// Five-tier escalation levels during self-healing upon wake/resume from sleep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SelfHealingTier {
    /// Tier 1: RST 僵尸连接清理 (RST Zombie Connection Purge)
    Tier1RstZombiePurge,
    /// Tier 2: Fake-IP 探活与解析池健康检测 (Fake-IP Liveness Probe)
    Tier2FakeIpLiveness,
    /// Tier 3: 节点重测速与出站路由探活 (Proxy Node Latency Retest)
    Tier3NodeDelayRetest,
    /// Tier 4: 控制器重载与配置热加载 (Controller Hot-Reload / API Reconnect)
    Tier4ConfigReload,
    /// Tier 5: 进程重启与安全模式直接降级 (Process Respawn & Safe-Mode Direct Fallback)
    Tier5ProcessRespawnAndSafeMode,
}

impl SelfHealingTier {
    pub fn tier_number(&self) -> u8 {
        match self {
            Self::Tier1RstZombiePurge => 1,
            Self::Tier2FakeIpLiveness => 2,
            Self::Tier3NodeDelayRetest => 3,
            Self::Tier4ConfigReload => 4,
            Self::Tier5ProcessRespawnAndSafeMode => 5,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Tier1RstZombiePurge => "Tier 1: RST Zombie Connection Purge",
            Self::Tier2FakeIpLiveness => "Tier 2: Fake-IP Liveness Probe",
            Self::Tier3NodeDelayRetest => "Tier 3: Node Latency Retest",
            Self::Tier4ConfigReload => "Tier 4: Controller Hot-Reload",
            Self::Tier5ProcessRespawnAndSafeMode => "Tier 5: Process Respawn & Safe-Mode Fallback",
        }
    }
}

/// Execution outcome of an individual self-healing pipeline step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepOutcome {
    Success { message: String },
    Skipped { reason: String },
    Failed { error: String },
    Escalated { to_tier: SelfHealingTier, reason: String },
}

/// Execution report for a single tier step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineStepReport {
    pub tier: SelfHealingTier,
    pub outcome: StepOutcome,
    pub duration_ms: u64,
}

/// Full execution report of the 5-tier self-healing pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfHealingPipelineReport {
    pub timestamp_secs: u64,
    pub trigger_reason: String,
    pub steps: Vec<PipelineStepReport>,
    pub highest_tier_reached: SelfHealingTier,
    pub success: bool,
    pub safe_mode_tripped: bool,
    pub total_duration_ms: u64,
}

/// Five-tier self-healing pipeline executing progressive recovery actions on wake/resume:
/// Tier 1: RST 僵尸连接清理
/// Tier 2: Fake-IP 探活
/// Tier 3: 节点重测速
/// Tier 4: 控制器/配置热重载
/// Tier 5: 进程重启与安全模式降级
#[derive(Clone)]
pub struct SelfHealingPipeline {
    zombie_purge_fn: Option<ZombiePurgeFn>,
    fake_ip_probe_fn: Option<FakeIpProbeFn>,
    node_retest_fn: Option<NodeRetestFn>,
    config_reload_fn: Option<ConfigReloadFn>,
    process_respawn_fn: Option<ProcessRespawnFn>,
    step_timeout: Duration,
    safe_mode_threshold: u32,
    consecutive_failures: Arc<AtomicU64>,
}

/// Tracks runtime self-healing metrics and recovery actions.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SelfHealingStats {
    pub resumes_detected: u64,
    pub zombie_purges_executed: u64,
    pub fake_ip_probes_attempted: u64,
    pub fake_ip_probes_succeeded: u64,
    pub node_retests_executed: u64,
    pub config_reloads_executed: u64,
    pub process_respawns_executed: u64,
    pub probes_attempted: u64,
    pub probes_succeeded: u64,
    pub resets_triggered: u64,
    pub recoveries_executed: u64,
    pub safe_mode_trips: u64,
    pub last_healing_timestamp_secs: u64,
    pub last_pipeline_report: Option<SelfHealingPipelineReport>,
}

/// Controls self-healing by listening to power events and triggering progressive recovery pipelines.
pub struct SelfHealingController {
    probe_fn: ProbeFn,
    recovery_fn: RecoveryFn,
    reset_conns_fn: Option<ResetConnectionsFn>,
    pipeline: Option<SelfHealingPipeline>,
    debounce_duration: Duration,
    probe_timeout: Duration,
    trigger_tx: broadcast::Sender<SelfHealingTrigger>,
    report_tx: broadcast::Sender<SelfHealingPipelineReport>,
    stats: Arc<Mutex<SelfHealingStats>>,
    consecutive_failures: Arc<AtomicU64>,
}

impl SelfHealingController {
    /// Default debounce duration after resume (2s).
    pub const DEFAULT_DEBOUNCE: Duration = Duration::from_secs(2);
    /// Default probe timeout (2s).
    pub const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

    /// Creates a new SelfHealingController with default 2s debounce and 2s probe timeout.
    pub fn new(probe_fn: ProbeFn, recovery_fn: RecoveryFn) -> Self {
        Self::new_with_config(
            probe_fn,
            recovery_fn,
            Self::DEFAULT_DEBOUNCE,
            Self::DEFAULT_PROBE_TIMEOUT,
        )
    }

    /// Creates a new SelfHealingController with a custom probe timeout and default 2s debounce.
    pub fn new_with_timeout(
        probe_fn: ProbeFn,
        recovery_fn: RecoveryFn,
        probe_timeout: Duration,
    ) -> Self {
        Self::new_with_config(probe_fn, recovery_fn, Self::DEFAULT_DEBOUNCE, probe_timeout)
    }

    /// Creates a new SelfHealingController with custom debounce and probe timeout.
    pub fn new_with_config(
        probe_fn: ProbeFn,
        recovery_fn: RecoveryFn,
        debounce_duration: Duration,
        probe_timeout: Duration,
    ) -> Self {
        let (trigger_tx, _) = broadcast::channel(32);
        let (report_tx, _) = broadcast::channel(32);
        Self {
            probe_fn,
            recovery_fn,
            reset_conns_fn: None,
            pipeline: None,
            debounce_duration,
            probe_timeout,
            trigger_tx,
            report_tx,
            stats: Arc::new(Mutex::new(SelfHealingStats::default())),
            consecutive_failures: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Attaches an optional connection reset hook (e.g. sending RST / close_all on wake).
    pub fn with_connection_reset(mut self, reset_fn: ResetConnectionsFn) -> Self {
        self.reset_conns_fn = Some(reset_fn);
        self
    }

    /// Attaches a full 5-tier self-healing pipeline.
    pub fn with_pipeline(mut self, pipeline: SelfHealingPipeline) -> Self {
        self.pipeline = Some(pipeline);
        self
    }

    /// Subscribes to self-healing trigger events emitted when controller API is dead.
    pub fn subscribe_triggers(&self) -> broadcast::Receiver<SelfHealingTrigger> {
        self.trigger_tx.subscribe()
    }

    /// Subscribes to full 5-tier self-healing execution reports.
    pub fn subscribe_reports(&self) -> broadcast::Receiver<SelfHealingPipelineReport> {
        self.report_tx.subscribe()
    }

    /// Returns a clone of the trigger sender.
    pub fn trigger_sender(&self) -> broadcast::Sender<SelfHealingTrigger> {
        self.trigger_tx.clone()
    }

    /// Returns a snapshot of self-healing stats.
    pub async fn get_stats(&self) -> SelfHealingStats {
        self.stats.lock().await.clone()
    }

    /// Executes the 5-tier self-healing pipeline directly and records statistics.
    pub async fn execute_pipeline(&self, reason: &str) -> SelfHealingPipelineReport {
        let report = if let Some(ref pipeline) = self.pipeline {
            pipeline.execute(reason).await
        } else {
            // Construct a default pipeline using existing probe/recovery hooks
            let p_fn = self.probe_fn.clone();
            let r_fn = self.recovery_fn.clone();
            let mut pl = SelfHealingPipeline::new()
                .with_fake_ip_probe(Arc::new(move || {
                    let pf = p_fn();
                    Box::pin(async move {
                        let ok = pf.await;
                        Ok(ok)
                    })
                }))
                .with_config_reload(Arc::new(move || {
                    let rf = r_fn();
                    Box::pin(async move {
                        rf.await;
                        Ok(())
                    })
                }));

            if let Some(ref reset) = self.reset_conns_fn {
                let rst = reset.clone();
                pl = pl.with_zombie_purge(Arc::new(move || {
                    let rf = rst();
                    Box::pin(async move {
                        rf.await;
                        Ok(())
                    })
                }));
            }

            pl.execute(reason).await
        };

        {
            let mut stats_guard = self.stats.lock().await;
            stats_guard.last_healing_timestamp_secs = report.timestamp_secs;
            for step in &report.steps {
                match step.tier {
                    SelfHealingTier::Tier1RstZombiePurge => {
                        if matches!(step.outcome, StepOutcome::Success { .. }) {
                            stats_guard.zombie_purges_executed += 1;
                            stats_guard.resets_triggered += 1;
                        }
                    }
                    SelfHealingTier::Tier2FakeIpLiveness => {
                        stats_guard.fake_ip_probes_attempted += 1;
                        stats_guard.probes_attempted += 1;
                        if matches!(step.outcome, StepOutcome::Success { .. }) {
                            stats_guard.fake_ip_probes_succeeded += 1;
                            stats_guard.probes_succeeded += 1;
                        }
                    }
                    SelfHealingTier::Tier3NodeDelayRetest => {
                        if matches!(step.outcome, StepOutcome::Success { .. }) {
                            stats_guard.node_retests_executed += 1;
                        }
                    }
                    SelfHealingTier::Tier4ConfigReload => {
                        stats_guard.config_reloads_executed += 1;
                        stats_guard.recoveries_executed += 1;
                    }
                    SelfHealingTier::Tier5ProcessRespawnAndSafeMode => {
                        stats_guard.process_respawns_executed += 1;
                        if report.safe_mode_tripped {
                            stats_guard.safe_mode_trips += 1;
                        }
                    }
                }
            }
            stats_guard.last_pipeline_report = Some(report.clone());
        }

        let _ = self.report_tx.send(report.clone());
        if !report.success {
            let trigger = SelfHealingTrigger::with_attempts(reason, report.highest_tier_reached.tier_number() as u32);
            let _ = self.trigger_tx.send(trigger);
        }

        report
    }

    /// Executes a single probe and recovery cycle if the probe fails.
    /// Emits `SelfHealingTrigger` if the controller API is dead.
    /// Returns `true` if healthy, `false` if dead and recovery was triggered.
    pub async fn probe_and_heal(&self, reason: &str) -> bool {
        {
            let mut stats_guard = self.stats.lock().await;
            stats_guard.probes_attempted += 1;
        }

        let probe = (self.probe_fn)();
        let probe_success = time::timeout(self.probe_timeout, probe)
            .await
            .unwrap_or(false);

        if !probe_success {
            let failures = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
            {
                let mut stats_guard = self.stats.lock().await;
                stats_guard.recoveries_executed += 1;
                stats_guard.last_healing_timestamp_secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if failures >= 3 {
                    stats_guard.safe_mode_trips += 1;
                }
            }

            let trigger = SelfHealingTrigger::with_attempts(reason, failures as u32);
            let _ = self.trigger_tx.send(trigger);
            (self.recovery_fn)().await;
            false
        } else {
            self.consecutive_failures.store(0, Ordering::SeqCst);
            let mut stats_guard = self.stats.lock().await;
            stats_guard.probes_succeeded += 1;
            true
        }
    }

    /// Runs the controller background task, listening for power events.
    pub async fn run(
        &self,
        mut watcher_rx: broadcast::Receiver<PowerEvent>,
        mut stop_rx: watch::Receiver<bool>,
    ) {
        let mut last_wake = Instant::now()
            .checked_sub(Duration::from_secs(60))
            .unwrap_or_else(Instant::now);

        loop {
            tokio::select! {
                result = watcher_rx.recv() => {
                    match result {
                        Ok(event) if event.is_resume() => {
                            let now = Instant::now();
                            if now.duration_since(last_wake) < self.debounce_duration {
                                continue;
                            }
                            last_wake = now;

                            {
                                let mut stats = self.stats.lock().await;
                                stats.resumes_detected += 1;
                            }

                            if self.pipeline.is_some() {
                                self.execute_pipeline("Power resume / wake event detected").await;
                            } else {
                                // Trigger RST connection purge if configured
                                if let Some(ref reset_fn) = self.reset_conns_fn {
                                    reset_fn().await;
                                    let mut stats = self.stats.lock().await;
                                    stats.resets_triggered += 1;
                                }

                                self.probe_and_heal("Controller API unresponsive after power resume").await;
                            }
                        }
                        Ok(PowerEvent::Suspend | PowerEvent::Sleep | PowerEvent::Hibernate | PowerEvent::BatteryLow | PowerEvent::AcPowerChanged | PowerEvent::ScreenLocked | PowerEvent::ScreenUnlocked) => {}
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
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
    }
}

#[cfg(test)]
#[path = "power_test.rs"]
mod tests;
