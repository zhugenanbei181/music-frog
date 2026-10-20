//! Intelligent heuristic network diagnostics and auto-repair interactive wizard state machine.
//!
//! This file owns the typed data model of the auto-heal domain. The behaviour
//! of every business seam lives in the sibling modules next to it
//! (`breaker`, `ranking`, `recovery`, `signals`, `watchdog`, `wizard`), so no
//! single file outgrows the repository line budget while the public paths of
//! the crate stay exactly where they were.

mod breaker;
mod ranking;
mod recovery;
mod signals;
mod watchdog;
mod wizard;

use bevy::ecs::resource::Resource;
use std::collections::HashMap;
use std::time::Duration;

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

/// Cluster-wide circuit breaker registry managing health states across all proxy nodes.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MultiNodeCircuitRegistry {
    pub breakers: HashMap<String, NodeCircuitBreaker>,
}

/// Exponential backoff policy with bounded clamping for reconnects and diagnostics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExponentialBackoffPolicy {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f32,
    pub max_retries: usize,
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

/// Directed failover routing graph tracking upstream fallback chains and circular dependencies.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FailoverRoutingGraph {
    pub fallback_links: HashMap<String, String>,
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

/// The one stage a [`HealingWatchdog`] is in.
///
/// The stage is a single enum because "a repair runs" and "the watchdog
/// tripped" are mutually exclusive: the old flag pair could express both at
/// once, and only an implicit guard kept the impossible combination out.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum WatchdogStage {
    /// No repair is under the watchdog.
    #[default]
    Idle,
    /// A repair action runs; the deadlock timer counts up.
    Repairing {
        /// Name of the repair action under the watchdog.
        action_name: String,
    },
    /// The deadlock limit elapsed; the repair was forcibly aborted.
    Tripped {
        /// Name of the repair action that was aborted.
        action_name: String,
    },
}

/// Typed refusal of a watchdog move the current stage forbids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchdogTransitionError {
    /// The stage that was already running when the move was refused.
    pub from: WatchdogStage,
}

/// Watchdog timer monitoring auto-repair operations and forcibly aborting hung actions.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct HealingWatchdog {
    /// The single stage the watchdog is in.
    pub stage: WatchdogStage,
    /// Time the current stage has been running.
    pub elapsed_time: Duration,
    /// Deadlock budget one repair may spend before the watchdog aborts it.
    pub deadlock_limit: Duration,
}

/// A single measured observation for a candidate node.
///
/// Every metric is optional: `None` is the typed "unobserved" state. The
/// evaluator never substitutes a default, so an unobserved latency can never be
/// silently scored as if it were `0` (which would read as "perfect").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NodeObservation {
    pub latency_ms: Option<f32>,
    pub loss_ratio: Option<f32>,
}

/// Real probe outcomes recorded for a candidate node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeProbeHistory {
    pub successes: u32,
    pub failures: u32,
}

/// A named node with its latest observation and recorded probe history.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeCandidate {
    pub name: String,
    pub observation: NodeObservation,
    pub history: NodeProbeHistory,
}

/// Why the evaluator refused to score a candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnscorableReason {
    LatencyUnobserved,
    LossUnobserved,
    LatencyAndLossUnobserved,
}

/// Result of scoring one candidate. `Unscorable` is not a node failure; it is
/// the honest statement that the required facts were not observed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NodeEvaluation {
    Scored {
        score: f32,
        history_ratio: Option<f32>,
    },
    Unscorable {
        reason: UnscorableReason,
    },
}

/// Weighted heuristic ranking proxy nodes by latency, loss and probe history.
///
/// Lower score is better. Unobserved metrics are never fabricated: a candidate
/// missing latency or loss is `Unscorable` rather than scored as zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeuristicNodeEvaluator {
    pub latency_weight: f32,
    pub loss_weight: f32,
    pub history_weight: f32,
}

/// Severity of a self-heal anomaly surfaced to the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnomalySeverity {
    Info,
    Warning,
    Critical,
}

/// Typed self-heal anomaly the UI can render. Each variant carries only facts
/// that were actually observed; an unobserved metric surfaces as
/// [`SelfHealAnomaly::NodeUnobserved`] rather than an invented value.
#[derive(Clone, Debug, PartialEq)]
pub enum SelfHealAnomaly {
    NodeUnobserved {
        node: String,
        reason: UnscorableReason,
    },
    ExcessiveLatency {
        node: String,
        latency_ms: f32,
    },
    ExcessiveLoss {
        node: String,
        loss_ratio: f32,
    },
    UnhealthyHistory {
        node: String,
        success_ratio: f32,
    },
}

/// Context-aware self-heal detector. Thresholds are explicit configuration;
/// the detector only reports facts that were actually observed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextAwareHealDetector {
    pub evaluator: HeuristicNodeEvaluator,
    pub latency_ceiling_ms: f32,
    pub loss_ceiling: f32,
    pub history_floor: f32,
    pub history_min_samples: u32,
}

/// UI-facing board holding the latest self-heal signals and best node. The UI
/// reads this immutable projection; it never re-derives node health itself.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SelfHealSignalBoard {
    pub anomalies: Vec<SelfHealAnomaly>,
    pub best_node: Option<String>,
}

#[cfg(test)]
mod tests;
