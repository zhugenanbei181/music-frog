//! Intelligent heuristic network diagnostics and auto-repair interactive wizard state machine.

use bevy::ecs::resource::Resource;

/// Categorization of detected network / controller failure anomalies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticAnomaly {
    ControllerPortConflict(u16),
    TunInterfaceMissing,
    DnsLeakDetected,
    SubscriptionExpired,
    HighPacketLoss,
    ZombieProcessDetected,
}

/// A recommended automatic action to resolve a detected anomaly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoFixAction {
    pub anomaly: DiagnosticAnomaly,
    pub title: String,
    pub description: String,
    pub is_destructive: bool,
}

/// State machine for interactive auto-repair wizard.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct AutoHealWizardState {
    pub active_anomalies: Vec<DiagnosticAnomaly>,
    pub pending_actions: Vec<AutoFixAction>,
    pub is_repairing: bool,
    pub repair_progress_fraction: f32,
    pub last_repair_success: Option<bool>,
}

impl AutoHealWizardState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_anomaly(&mut self, anomaly: DiagnosticAnomaly) {
        if !self.active_anomalies.contains(&anomaly) {
            self.active_anomalies.push(anomaly);
            let action = match anomaly {
                DiagnosticAnomaly::ControllerPortConflict(port) => AutoFixAction {
                    anomaly,
                    title: format!("轮换控制器端口 (占用: {})", port),
                    description: "自动寻找未占用的高位空闲端口并重启核心接口".to_string(),
                    is_destructive: false,
                },
                DiagnosticAnomaly::TunInterfaceMissing => AutoFixAction {
                    anomaly,
                    title: "重新注册 TUN 虚拟网卡驱动".to_string(),
                    description: "调用系统网络特权服务重新分配虚拟网络接口".to_string(),
                    is_destructive: false,
                },
                DiagnosticAnomaly::DnsLeakDetected => AutoFixAction {
                    anomaly,
                    title: "强制启用 Strict Route 阻断直连 DNS".to_string(),
                    description: "重写防火墙规则，强制所有 UDP 53 流量重定向至核心 Fake-IP 池"
                        .to_string(),
                    is_destructive: false,
                },
                DiagnosticAnomaly::SubscriptionExpired => AutoFixAction {
                    anomaly,
                    title: "切换备用配置订阅".to_string(),
                    description: "当前订阅已过期，一键激活最近可用的备用配置档案".to_string(),
                    is_destructive: false,
                },
                DiagnosticAnomaly::HighPacketLoss => AutoFixAction {
                    anomaly,
                    title: "自动故障转移到低延迟节点".to_string(),
                    description: "执行并发测速并将当前策略组切至健康节点".to_string(),
                    is_destructive: false,
                },
                DiagnosticAnomaly::ZombieProcessDetected => AutoFixAction {
                    anomaly,
                    title: "清理僵尸核心进程".to_string(),
                    description: "向残留的孤儿进程发送终止信号并清理临时 PID 锁文件".to_string(),
                    is_destructive: true,
                },
            };
            self.pending_actions.push(action);
        }
    }

    pub fn clear(&mut self) {
        self.active_anomalies.clear();
        self.pending_actions.clear();
        self.is_repairing = false;
        self.repair_progress_fraction = 0.0;
        self.last_repair_success = None;
    }
}

use std::time::Duration;

/// Three-phase state of an individual proxy node circuit breaker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CircuitState {
    /// Normal operation: requests pass through, tracking consecutive failures.
    #[default]
    Closed,
    /// Tripped: consecutive failures exceeded threshold, failing fast to protect latency.
    Open,
    /// Canary testing: cooldown elapsed, allowing probe attempts to verify recovery.
    HalfOpen,
}

/// Adaptive circuit breaker protecting upstream routing from repeatedly failing proxy nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeCircuitBreaker {
    pub node_name: String,
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub cooldown_duration: Duration,
    pub tripped_at: Duration,
}

impl NodeCircuitBreaker {
    pub fn new(node_name: impl Into<String>) -> Self {
        Self {
            node_name: node_name.into(),
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            failure_threshold: 3,
            success_threshold: 2,
            cooldown_duration: Duration::from_secs(30),
            tripped_at: Duration::ZERO,
        }
    }

    /// Check whether a request or connection attempt is allowed through this node.
    pub fn can_attempt(&mut self, now: Duration) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if now >= self.tripped_at + self.cooldown_duration {
                    // Cooldown elapsed -> transition to HalfOpen canary
                    self.state = CircuitState::HalfOpen;
                    self.success_count = 0;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful connection or ping.
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.success_threshold {
                    // Canary passed -> fully recover to Closed
                    self.state = CircuitState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed connection, timeout, or handshake error.
    pub fn record_failure(&mut self, now: Duration) {
        match self.state {
            CircuitState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.failure_threshold {
                    // Threshold exceeded -> trip breaker to Open
                    self.state = CircuitState::Open;
                    self.tripped_at = now;
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in HalfOpen immediately trips back to Open
                self.state = CircuitState::Open;
                self.tripped_at = now;
                self.success_count = 0;
            }
            CircuitState::Open => {
                self.tripped_at = now;
            }
        }
    }

    pub fn is_healthy(&self) -> bool {
        self.state == CircuitState::Closed
    }
}

/// Phases of an interactive doctor healing transaction with rollback capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HealingPhase {
    #[default]
    Idle,
    Scanning,
    ReviewingAction,
    Applying,
    Verifying,
    Completed,
    RolledBack,
}

use std::collections::HashMap;

/// Cluster-wide circuit breaker registry managing health states across all proxy nodes.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MultiNodeCircuitRegistry {
    pub breakers: HashMap<String, NodeCircuitBreaker>,
}

impl MultiNodeCircuitRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Access or lazily initialize circuit breaker for a named node.
    pub fn get_or_create(&mut self, node_name: &str) -> &mut NodeCircuitBreaker {
        self.breakers
            .entry(node_name.to_string())
            .or_insert_with(|| NodeCircuitBreaker::new(node_name))
    }

    /// Record connection success on a node.
    pub fn record_node_success(&mut self, node_name: &str) {
        self.get_or_create(node_name).record_success();
    }

    /// Record connection failure or timeout on a node.
    pub fn record_node_failure(&mut self, node_name: &str, now: Duration) {
        self.get_or_create(node_name).record_failure(now);
    }

    /// Select the first healthy candidate from a priority order list.
    pub fn select_first_healthy<'a>(
        &mut self,
        candidates: &[&'a str],
        now: Duration,
    ) -> Option<&'a str> {
        for &cand in candidates {
            let breaker = self.get_or_create(cand);
            if breaker.can_attempt(now) {
                return Some(cand);
            }
        }
        None
    }

    /// Total number of currently tripped/open circuit breakers.
    pub fn tripped_count(&self) -> usize {
        self.breakers
            .values()
            .filter(|b| b.state == CircuitState::Open)
            .count()
    }
}

/// Exponential backoff policy with bounded clamping for reconnects and diagnostics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExponentialBackoffPolicy {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f32,
    pub max_retries: usize,
}

impl Default for ExponentialBackoffPolicy {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(15),
            backoff_factor: 2.0,
            max_retries: 5,
        }
    }
}

impl ExponentialBackoffPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate delay for attempt number `attempt` (0-indexed).
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        if attempt == 0 {
            return self.initial_delay;
        }
        let mult = self.backoff_factor.powi(attempt as i32);
        let millis = (self.initial_delay.as_millis() as f32 * mult) as u64;
        Duration::from_millis(millis).min(self.max_delay)
    }

    /// Check whether a further retry is allowed.
    pub fn should_retry(&self, attempt: usize) -> bool {
        attempt < self.max_retries
    }
}

/// Transactional auto-repair session with timeout-triggered state rollback.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoRollbackTransaction<S: Clone> {
    pub original_state: S,
    pub current_state: S,
    pub is_committed: bool,
    pub is_rolled_back: bool,
    pub timeout: Duration,
    pub elapsed: Duration,
}

impl<S: Clone> AutoRollbackTransaction<S> {
    pub fn new(initial_state: S, timeout: Duration) -> Self {
        Self {
            original_state: initial_state.clone(),
            current_state: initial_state,
            is_committed: false,
            is_rolled_back: false,
            timeout,
            elapsed: Duration::ZERO,
        }
    }

    /// Apply a tentative configuration change.
    pub fn apply_mutation(&mut self, mutated: S) {
        self.current_state = mutated;
    }

    /// Commit the change after diagnostics pass.
    pub fn commit(&mut self) -> Result<(), &'static str> {
        if self.is_rolled_back {
            return Err("Cannot commit already rolled back transaction");
        }
        self.is_committed = true;
        Ok(())
    }

    /// Rollback the change restoring original state.
    pub fn rollback(&mut self) -> S {
        self.current_state = self.original_state.clone();
        self.is_rolled_back = true;
        self.original_state.clone()
    }

    /// Advance verification timer. Returns true if timeout triggered auto-rollback.
    pub fn tick(&mut self, dt: Duration) -> bool {
        if self.is_committed || self.is_rolled_back {
            return false;
        }
        self.elapsed += dt;
        if self.elapsed >= self.timeout {
            self.rollback();
            true
        } else {
            false
        }
    }
}

/// Directed failover routing graph tracking upstream fallback chains and circular dependencies.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FailoverRoutingGraph {
    pub fallback_links: HashMap<String, String>,
}

impl FailoverRoutingGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Link a primary node to its backup failover destination.
    pub fn link_fallback(&mut self, primary: impl Into<String>, fallback: impl Into<String>) {
        self.fallback_links.insert(primary.into(), fallback.into());
    }

    /// Detect if the fallback chain starting from `start` contains a circular reference.
    pub fn has_cycle_from(&self, start: &str) -> bool {
        let mut visited = std::collections::HashSet::new();
        let mut curr = start;

        while let Some(next) = self.fallback_links.get(curr) {
            if !visited.insert(curr) {
                return true;
            }
            curr = next.as_str();
        }
        false
    }

    /// Resolve the active non-tripped outbound target along the fallback chain.
    /// Safely falls back to "DIRECT" if all nodes in chain are tripped or on cycle.
    pub fn resolve_active_outbound(
        &self,
        start_node: &str,
        is_node_tripped: impl Fn(&str) -> bool,
    ) -> String {
        let mut visited = std::collections::HashSet::new();
        let mut curr = start_node;

        while visited.insert(curr) {
            if !is_node_tripped(curr) {
                return curr.to_string();
            }
            if let Some(next) = self.fallback_links.get(curr) {
                curr = next.as_str();
            } else {
                break;
            }
        }

        "DIRECT".to_string()
    }
}

/// Heuristic inference engine evaluating causality priority across multiple co-occurring anomalies.
pub struct RootCauseInferenceEngine;

impl RootCauseInferenceEngine {
    /// Return the priority rank of an anomaly (lower number = more foundational root cause).
    pub fn causality_rank(anomaly: &DiagnosticAnomaly) -> u32 {
        match anomaly {
            DiagnosticAnomaly::ZombieProcessDetected => 1,
            DiagnosticAnomaly::ControllerPortConflict(_) => 2,
            DiagnosticAnomaly::TunInterfaceMissing => 3,
            DiagnosticAnomaly::SubscriptionExpired => 4,
            DiagnosticAnomaly::DnsLeakDetected => 5,
            DiagnosticAnomaly::HighPacketLoss => 6,
        }
    }

    /// Identify the single primary root cause from a collection of active anomalies.
    pub fn identify_primary(anomalies: &[DiagnosticAnomaly]) -> Option<DiagnosticAnomaly> {
        anomalies.iter().copied().min_by_key(Self::causality_rank)
    }
}

/// Watchdog timer monitoring auto-repair operations and forcibly aborting hung actions.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct HealingWatchdog {
    pub is_repairing: bool,
    pub active_action_name: Option<String>,
    pub elapsed_time: Duration,
    pub deadlock_limit: Duration,
    pub has_tripped: bool,
}

impl Default for HealingWatchdog {
    fn default() -> Self {
        Self {
            is_repairing: false,
            active_action_name: None,
            elapsed_time: Duration::ZERO,
            deadlock_limit: Duration::from_secs(10),
            has_tripped: false,
        }
    }
}

impl HealingWatchdog {
    pub fn new(limit: Duration) -> Self {
        Self {
            deadlock_limit: limit,
            ..Default::default()
        }
    }

    /// Arm watchdog for a new repair action.
    pub fn arm(&mut self, action_name: impl Into<String>) {
        self.is_repairing = true;
        self.active_action_name = Some(action_name.into());
        self.elapsed_time = Duration::ZERO;
        self.has_tripped = false;
    }

    /// Disarm watchdog upon clean action completion.
    pub fn disarm(&mut self) {
        self.is_repairing = false;
        self.active_action_name = None;
        self.elapsed_time = Duration::ZERO;
        self.has_tripped = false;
    }

    /// Step watchdog timer. Returns true if deadlock limit reached, forcibly aborting repair.
    pub fn tick(&mut self, dt: Duration) -> bool {
        if !self.is_repairing || self.has_tripped {
            return false;
        }
        self.elapsed_time += dt;
        if self.elapsed_time >= self.deadlock_limit {
            self.has_tripped = true;
            self.is_repairing = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_trips_to_open_on_threshold_failures() {
        let mut breaker = NodeCircuitBreaker::new("HK-01");
        assert!(breaker.is_healthy());

        breaker.record_failure(Duration::from_secs(1));
        assert_eq!(breaker.state, CircuitState::Closed);
        assert_eq!(breaker.failure_count, 1);

        breaker.record_failure(Duration::from_secs(2));
        assert_eq!(breaker.failure_count, 2);

        // Third failure trips to Open
        breaker.record_failure(Duration::from_secs(3));
        assert_eq!(breaker.state, CircuitState::Open);
        assert!(!breaker.is_healthy());

        // Fast fail inside cooldown window
        assert!(!breaker.can_attempt(Duration::from_secs(10)));
    }

    #[test]
    fn test_circuit_breaker_cooldown_to_half_open_and_recovery() {
        let mut breaker = NodeCircuitBreaker::new("US-01");
        for i in 1..=3 {
            breaker.record_failure(Duration::from_secs(i));
        }
        assert_eq!(breaker.state, CircuitState::Open);

        // After cooldown (30s from tripped_at=3 -> 33s)
        assert!(breaker.can_attempt(Duration::from_secs(35)));
        assert_eq!(breaker.state, CircuitState::HalfOpen);

        // First canary success
        breaker.record_success();
        assert_eq!(breaker.state, CircuitState::HalfOpen);

        // Second canary success -> fully recovered
        breaker.record_success();
        assert_eq!(breaker.state, CircuitState::Closed);
        assert!(breaker.is_healthy());
    }

    #[test]
    fn test_circuit_breaker_half_open_failure_reopens() {
        let mut breaker = NodeCircuitBreaker::new("JP-01");
        for i in 1..=3 {
            breaker.record_failure(Duration::from_secs(i));
        }

        // Advance past cooldown to HalfOpen
        assert!(breaker.can_attempt(Duration::from_secs(40)));
        assert_eq!(breaker.state, CircuitState::HalfOpen);

        // Failure during canary immediately re-opens breaker
        breaker.record_failure(Duration::from_secs(41));
        assert_eq!(breaker.state, CircuitState::Open);
        assert_eq!(breaker.tripped_at, Duration::from_secs(41));
    }
    #[test]
    fn test_multi_node_circuit_registry_failover() {
        let mut reg = MultiNodeCircuitRegistry::new();
        let candidates = ["Node-A", "Node-B", "Node-C"];

        // Initially Node-A is selected
        let first = reg.select_first_healthy(&candidates, Duration::ZERO);
        assert_eq!(first, Some("Node-A"));

        // Trip Node-A with 3 failures
        for i in 1..=3 {
            reg.record_node_failure("Node-A", Duration::from_secs(i));
        }
        assert_eq!(reg.tripped_count(), 1);

        // Failover automatically picks Node-B
        let failover = reg.select_first_healthy(&candidates, Duration::from_secs(5));
        assert_eq!(failover, Some("Node-B"));
    }
    #[test]
    fn test_exponential_backoff_delays_and_limit() {
        let policy = ExponentialBackoffPolicy::default();
        assert!(policy.should_retry(0));
        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(500));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(1000));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(2000));

        // Clamped at max_delay (15s)
        assert_eq!(policy.delay_for_attempt(10), Duration::from_secs(15));

        // Terminate at max_retries (5)
        assert!(!policy.should_retry(5));
    }
    #[test]
    fn test_auto_rollback_transaction_lifecycle() {
        let mut tx =
            AutoRollbackTransaction::new("proxy_mode=rule".to_string(), Duration::from_secs(5));
        assert_eq!(tx.current_state, "proxy_mode=rule");

        tx.apply_mutation("proxy_mode=global".to_string());
        assert_eq!(tx.current_state, "proxy_mode=global");

        // Tick 3s -> not timed out
        assert!(!tx.tick(Duration::from_secs(3)));
        assert_eq!(tx.current_state, "proxy_mode=global");

        // Tick 3s more (total 6s >= 5s) -> auto rollback triggered
        assert!(tx.tick(Duration::from_secs(3)));
        assert!(tx.is_rolled_back);
        assert_eq!(tx.current_state, "proxy_mode=rule");
    }
    #[test]
    fn test_failover_routing_graph_resolution_and_cycle() {
        let mut graph = FailoverRoutingGraph::new();
        graph.link_fallback("HK-01", "HK-02");
        graph.link_fallback("HK-02", "JP-01");

        assert!(!graph.has_cycle_from("HK-01"));

        // If HK-01 is healthy -> HK-01
        assert_eq!(graph.resolve_active_outbound("HK-01", |_| false), "HK-01");

        // If HK-01 is tripped, HK-02 healthy -> HK-02
        assert_eq!(
            graph.resolve_active_outbound("HK-01", |node| node == "HK-01"),
            "HK-02"
        );

        // If HK-01 and HK-02 tripped -> JP-01
        assert_eq!(
            graph.resolve_active_outbound("HK-01", |node| node.starts_with("HK")),
            "JP-01"
        );

        // Circular link detection
        graph.link_fallback("JP-01", "HK-01");
        assert!(graph.has_cycle_from("HK-01"));
        // Safely degrades to DIRECT on circular loop with all tripped
        assert_eq!(graph.resolve_active_outbound("HK-01", |_| true), "DIRECT");
    }
    #[test]
    fn test_root_cause_inference_engine_priority() {
        // HighPacketLoss + TunInterfaceMissing + DnsLeakDetected -> TunInterfaceMissing is primary
        let anomalies = vec![
            DiagnosticAnomaly::HighPacketLoss,
            DiagnosticAnomaly::DnsLeakDetected,
            DiagnosticAnomaly::TunInterfaceMissing,
        ];
        let root = RootCauseInferenceEngine::identify_primary(&anomalies);
        assert_eq!(root, Some(DiagnosticAnomaly::TunInterfaceMissing));

        // If ZombieProcessDetected is also present, it takes top priority
        let with_zombie = vec![
            DiagnosticAnomaly::HighPacketLoss,
            DiagnosticAnomaly::ZombieProcessDetected,
            DiagnosticAnomaly::TunInterfaceMissing,
        ];
        assert_eq!(
            RootCauseInferenceEngine::identify_primary(&with_zombie),
            Some(DiagnosticAnomaly::ZombieProcessDetected)
        );
    }
    #[test]
    fn test_healing_watchdog_timeout_trip() {
        let mut watchdog = HealingWatchdog::new(Duration::from_secs(4));
        assert!(!watchdog.is_repairing);

        watchdog.arm("rebind_tun");
        assert!(watchdog.is_repairing);

        // Advance 2s -> does not trip
        assert!(!watchdog.tick(Duration::from_secs(2)));
        assert!(watchdog.is_repairing);

        // Advance 3s more (5s total >= 4s limit) -> trips watchdog
        assert!(watchdog.tick(Duration::from_secs(3)));
        assert!(watchdog.has_tripped);
        assert!(!watchdog.is_repairing);
    }
}
