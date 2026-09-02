use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::time::{self, Instant};

use super::{
    ConfigReloadFn, FakeIpProbeFn, NodeRetestFn, PipelineStepReport, ProcessRespawnFn,
    SelfHealingPipeline, SelfHealingPipelineReport, SelfHealingTier, StepOutcome, ZombiePurgeFn,
};

impl Default for SelfHealingPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfHealingPipeline {
    /// Default per-step timeout (1500ms).
    pub const DEFAULT_STEP_TIMEOUT: Duration = Duration::from_millis(1500);
    /// Default threshold of consecutive failures before tripping safe mode (3).
    pub const DEFAULT_SAFE_MODE_THRESHOLD: u32 = 3;

    pub fn new() -> Self {
        Self {
            zombie_purge_fn: None,
            fake_ip_probe_fn: None,
            node_retest_fn: None,
            config_reload_fn: None,
            process_respawn_fn: None,
            step_timeout: Self::DEFAULT_STEP_TIMEOUT,
            safe_mode_threshold: Self::DEFAULT_SAFE_MODE_THRESHOLD,
            consecutive_failures: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn with_zombie_purge(mut self, purge_fn: ZombiePurgeFn) -> Self {
        self.zombie_purge_fn = Some(purge_fn);
        self
    }

    pub fn with_fake_ip_probe(mut self, probe_fn: FakeIpProbeFn) -> Self {
        self.fake_ip_probe_fn = Some(probe_fn);
        self
    }

    pub fn with_node_retest(mut self, retest_fn: NodeRetestFn) -> Self {
        self.node_retest_fn = Some(retest_fn);
        self
    }

    pub fn with_config_reload(mut self, reload_fn: ConfigReloadFn) -> Self {
        self.config_reload_fn = Some(reload_fn);
        self
    }

    pub fn with_process_respawn(mut self, respawn_fn: ProcessRespawnFn) -> Self {
        self.process_respawn_fn = Some(respawn_fn);
        self
    }

    pub fn with_step_timeout(mut self, timeout: Duration) -> Self {
        self.step_timeout = timeout;
        self
    }

    pub fn with_safe_mode_threshold(mut self, threshold: u32) -> Self {
        self.safe_mode_threshold = threshold;
        self
    }

    pub fn reset_failures(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
    }

    pub fn consecutive_failures(&self) -> u64 {
        self.consecutive_failures.load(Ordering::SeqCst)
    }

    /// Executes the 5-tier self-healing pipeline sequentially.
    pub async fn execute(&self, trigger_reason: &str) -> SelfHealingPipelineReport {
        let start_time = Instant::now();
        let timestamp_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut steps = Vec::new();
        let mut highest_tier = SelfHealingTier::Tier1RstZombiePurge;
        let mut overall_success = true;
        let mut needs_escalation_to_tier4 = false;

        // --- Tier 1: RST 僵尸连接清理 ---
        let t1_start = Instant::now();
        if let Some(ref purge_fn) = self.zombie_purge_fn {
            let res = time::timeout(self.step_timeout, purge_fn()).await;
            let outcome = match res {
                Ok(Ok(())) => StepOutcome::Success {
                    message: "RST zombie connections purged successfully".to_string(),
                },
                Ok(Err(err)) => StepOutcome::Failed {
                    error: format!("RST purge failed: {}", err),
                },
                Err(_) => StepOutcome::Failed {
                    error: "RST purge timed out".to_string(),
                },
            };
            steps.push(PipelineStepReport {
                tier: SelfHealingTier::Tier1RstZombiePurge,
                outcome,
                duration_ms: t1_start.elapsed().as_millis() as u64,
            });
        } else {
            steps.push(PipelineStepReport {
                tier: SelfHealingTier::Tier1RstZombiePurge,
                outcome: StepOutcome::Skipped {
                    reason: "No zombie purge hook configured".to_string(),
                },
                duration_ms: 0,
            });
        }

        // --- Tier 2: Fake-IP 探活 ---
        let t2_start = Instant::now();
        if let Some(ref probe_fn) = self.fake_ip_probe_fn {
            highest_tier = SelfHealingTier::Tier2FakeIpLiveness;
            let res = time::timeout(self.step_timeout, probe_fn()).await;
            match res {
                Ok(Ok(true)) => {
                    steps.push(PipelineStepReport {
                        tier: SelfHealingTier::Tier2FakeIpLiveness,
                        outcome: StepOutcome::Success {
                            message: "Fake-IP resolution probe succeeded".to_string(),
                        },
                        duration_ms: t2_start.elapsed().as_millis() as u64,
                    });
                }
                Ok(Ok(false)) => {
                    needs_escalation_to_tier4 = true;
                    steps.push(PipelineStepReport {
                        tier: SelfHealingTier::Tier2FakeIpLiveness,
                        outcome: StepOutcome::Escalated {
                            to_tier: SelfHealingTier::Tier4ConfigReload,
                            reason: "Fake-IP resolution probe returned false".to_string(),
                        },
                        duration_ms: t2_start.elapsed().as_millis() as u64,
                    });
                }
                Ok(Err(err)) => {
                    needs_escalation_to_tier4 = true;
                    steps.push(PipelineStepReport {
                        tier: SelfHealingTier::Tier2FakeIpLiveness,
                        outcome: StepOutcome::Escalated {
                            to_tier: SelfHealingTier::Tier4ConfigReload,
                            reason: format!("Fake-IP probe error: {}", err),
                        },
                        duration_ms: t2_start.elapsed().as_millis() as u64,
                    });
                }
                Err(_) => {
                    needs_escalation_to_tier4 = true;
                    steps.push(PipelineStepReport {
                        tier: SelfHealingTier::Tier2FakeIpLiveness,
                        outcome: StepOutcome::Escalated {
                            to_tier: SelfHealingTier::Tier4ConfigReload,
                            reason: "Fake-IP probe timed out".to_string(),
                        },
                        duration_ms: t2_start.elapsed().as_millis() as u64,
                    });
                }
            }
        } else {
            steps.push(PipelineStepReport {
                tier: SelfHealingTier::Tier2FakeIpLiveness,
                outcome: StepOutcome::Skipped {
                    reason: "No Fake-IP probe configured".to_string(),
                },
                duration_ms: 0,
            });
        }

        // --- Tier 3: 节点重测速 (Run if Tier 2 didn't trigger immediate escalation) ---
        if !needs_escalation_to_tier4 {
            let t3_start = Instant::now();
            highest_tier = SelfHealingTier::Tier3NodeDelayRetest;
            if let Some(ref retest_fn) = self.node_retest_fn {
                let res = time::timeout(self.step_timeout, retest_fn()).await;
                match res {
                    Ok(Ok(alive_count)) => {
                        if alive_count > 0 {
                            steps.push(PipelineStepReport {
                                tier: SelfHealingTier::Tier3NodeDelayRetest,
                                outcome: StepOutcome::Success {
                                    message: format!(
                                        "Node retest completed: {} alive nodes",
                                        alive_count
                                    ),
                                },
                                duration_ms: t3_start.elapsed().as_millis() as u64,
                            });
                        } else {
                            needs_escalation_to_tier4 = true;
                            steps.push(PipelineStepReport {
                                tier: SelfHealingTier::Tier3NodeDelayRetest,
                                outcome: StepOutcome::Escalated {
                                    to_tier: SelfHealingTier::Tier4ConfigReload,
                                    reason: "Node retest found 0 alive proxies".to_string(),
                                },
                                duration_ms: t3_start.elapsed().as_millis() as u64,
                            });
                        }
                    }
                    Ok(Err(err)) => {
                        needs_escalation_to_tier4 = true;
                        steps.push(PipelineStepReport {
                            tier: SelfHealingTier::Tier3NodeDelayRetest,
                            outcome: StepOutcome::Escalated {
                                to_tier: SelfHealingTier::Tier4ConfigReload,
                                reason: format!("Node retest error: {}", err),
                            },
                            duration_ms: t3_start.elapsed().as_millis() as u64,
                        });
                    }
                    Err(_) => {
                        needs_escalation_to_tier4 = true;
                        steps.push(PipelineStepReport {
                            tier: SelfHealingTier::Tier3NodeDelayRetest,
                            outcome: StepOutcome::Escalated {
                                to_tier: SelfHealingTier::Tier4ConfigReload,
                                reason: "Node retest timed out".to_string(),
                            },
                            duration_ms: t3_start.elapsed().as_millis() as u64,
                        });
                    }
                }
            } else {
                steps.push(PipelineStepReport {
                    tier: SelfHealingTier::Tier3NodeDelayRetest,
                    outcome: StepOutcome::Skipped {
                        reason: "No node retest hook configured".to_string(),
                    },
                    duration_ms: 0,
                });
            }
        }

        // --- Tier 4: 控制器与配置热重载 (Escalation from Tier 2 or Tier 3 failure) ---
        let mut needs_escalation_to_tier5 = false;
        if needs_escalation_to_tier4 {
            let t4_start = Instant::now();
            highest_tier = SelfHealingTier::Tier4ConfigReload;
            if let Some(ref reload_fn) = self.config_reload_fn {
                let res = time::timeout(self.step_timeout, reload_fn()).await;
                match res {
                    Ok(Ok(())) => {
                        // Re-verify Fake-IP after reload if available
                        let probe_ok = if let Some(ref probe_fn) = self.fake_ip_probe_fn {
                            time::timeout(self.step_timeout, probe_fn())
                                .await
                                .ok()
                                .and_then(|r| r.ok())
                                .unwrap_or(true)
                        } else {
                            true
                        };

                        if probe_ok {
                            steps.push(PipelineStepReport {
                                tier: SelfHealingTier::Tier4ConfigReload,
                                outcome: StepOutcome::Success {
                                    message: "Controller and config reloaded successfully"
                                        .to_string(),
                                },
                                duration_ms: t4_start.elapsed().as_millis() as u64,
                            });
                        } else {
                            needs_escalation_to_tier5 = true;
                            steps.push(PipelineStepReport {
                                tier: SelfHealingTier::Tier4ConfigReload,
                                outcome: StepOutcome::Escalated {
                                    to_tier: SelfHealingTier::Tier5ProcessRespawnAndSafeMode,
                                    reason: "Config reloaded but post-reload verification failed"
                                        .to_string(),
                                },
                                duration_ms: t4_start.elapsed().as_millis() as u64,
                            });
                        }
                    }
                    Ok(Err(err)) => {
                        needs_escalation_to_tier5 = true;
                        steps.push(PipelineStepReport {
                            tier: SelfHealingTier::Tier4ConfigReload,
                            outcome: StepOutcome::Escalated {
                                to_tier: SelfHealingTier::Tier5ProcessRespawnAndSafeMode,
                                reason: format!("Config reload error: {}", err),
                            },
                            duration_ms: t4_start.elapsed().as_millis() as u64,
                        });
                    }
                    Err(_) => {
                        needs_escalation_to_tier5 = true;
                        steps.push(PipelineStepReport {
                            tier: SelfHealingTier::Tier4ConfigReload,
                            outcome: StepOutcome::Escalated {
                                to_tier: SelfHealingTier::Tier5ProcessRespawnAndSafeMode,
                                reason: "Config reload timed out".to_string(),
                            },
                            duration_ms: t4_start.elapsed().as_millis() as u64,
                        });
                    }
                }
            } else {
                needs_escalation_to_tier5 = true;
                steps.push(PipelineStepReport {
                    tier: SelfHealingTier::Tier4ConfigReload,
                    outcome: StepOutcome::Escalated {
                        to_tier: SelfHealingTier::Tier5ProcessRespawnAndSafeMode,
                        reason: "No config reload hook configured".to_string(),
                    },
                    duration_ms: 0,
                });
            }
        }

        // --- Tier 5: 进程重启与安全模式直接降级 (Escalation from Tier 4 failure) ---
        let mut safe_mode_tripped = false;
        if needs_escalation_to_tier5 {
            let t5_start = Instant::now();
            highest_tier = SelfHealingTier::Tier5ProcessRespawnAndSafeMode;
            let failures = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;

            if failures >= self.safe_mode_threshold as u64 {
                safe_mode_tripped = true;
                overall_success = false;
            }

            if let Some(ref respawn_fn) = self.process_respawn_fn {
                let res = time::timeout(self.step_timeout, respawn_fn()).await;
                match res {
                    Ok(Ok(())) => {
                        steps.push(PipelineStepReport {
                            tier: SelfHealingTier::Tier5ProcessRespawnAndSafeMode,
                            outcome: if safe_mode_tripped {
                                StepOutcome::Failed {
                                    error: format!(
                                        "Respawn executed but safe mode tripped after {} consecutive failures",
                                        failures
                                    ),
                                }
                            } else {
                                StepOutcome::Success {
                                    message: format!("Process respawn succeeded (attempt {})", failures),
                                }
                            },
                            duration_ms: t5_start.elapsed().as_millis() as u64,
                        });
                    }
                    Ok(Err(err)) => {
                        overall_success = false;
                        steps.push(PipelineStepReport {
                            tier: SelfHealingTier::Tier5ProcessRespawnAndSafeMode,
                            outcome: StepOutcome::Failed {
                                error: format!("Process respawn failed: {}", err),
                            },
                            duration_ms: t5_start.elapsed().as_millis() as u64,
                        });
                    }
                    Err(_) => {
                        overall_success = false;
                        steps.push(PipelineStepReport {
                            tier: SelfHealingTier::Tier5ProcessRespawnAndSafeMode,
                            outcome: StepOutcome::Failed {
                                error: "Process respawn timed out".to_string(),
                            },
                            duration_ms: t5_start.elapsed().as_millis() as u64,
                        });
                    }
                }
            } else {
                overall_success = false;
                steps.push(PipelineStepReport {
                    tier: SelfHealingTier::Tier5ProcessRespawnAndSafeMode,
                    outcome: StepOutcome::Failed {
                        error: "No process respawn hook configured; safe mode direct fallback"
                            .to_string(),
                    },
                    duration_ms: 0,
                });
            }
        } else {
            // Pipeline succeeded without escalating to Tier 5; reset consecutive failure count
            self.reset_failures();
        }

        let total_duration_ms = start_time.elapsed().as_millis() as u64;

        SelfHealingPipelineReport {
            timestamp_secs,
            trigger_reason: trigger_reason.to_string(),
            steps,
            highest_tier_reached: highest_tier,
            success: overall_success,
            safe_mode_tripped,
            total_duration_ms,
        }
    }
}
