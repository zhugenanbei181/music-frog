//! Self-heal signal projection: typed anomalies, the context-aware detector and
//! the immutable board the UI reads.

use super::{
    AnomalySeverity, ContextAwareHealDetector, HeuristicNodeEvaluator, NodeCandidate,
    NodeEvaluation, SelfHealAnomaly, SelfHealSignalBoard,
};

impl SelfHealAnomaly {
    pub fn severity(&self) -> AnomalySeverity {
        match self {
            Self::NodeUnobserved { .. } => AnomalySeverity::Warning,
            Self::ExcessiveLatency { .. } => AnomalySeverity::Warning,
            Self::ExcessiveLoss { .. } => AnomalySeverity::Critical,
            Self::UnhealthyHistory { .. } => AnomalySeverity::Critical,
        }
    }

    pub fn node_name(&self) -> &str {
        match self {
            Self::NodeUnobserved { node, .. }
            | Self::ExcessiveLatency { node, .. }
            | Self::ExcessiveLoss { node, .. }
            | Self::UnhealthyHistory { node, .. } => node,
        }
    }
}

impl Default for ContextAwareHealDetector {
    fn default() -> Self {
        Self {
            evaluator: HeuristicNodeEvaluator::default(),
            latency_ceiling_ms: 800.0,
            loss_ceiling: 0.05,
            history_floor: 0.5,
            history_min_samples: 3,
        }
    }
}

impl ContextAwareHealDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// The best scored node under the current heuristic, if any candidate had
    /// the observed facts required to be scored.
    pub fn best<'a>(&self, candidates: &'a [NodeCandidate]) -> Option<&'a NodeCandidate> {
        self.evaluator.best(candidates)
    }

    /// Collect typed anomalies for the UI. Order follows candidate order; no
    /// synthetic anomaly is emitted for a node without observations.
    pub fn detect(&self, candidates: &[NodeCandidate]) -> Vec<SelfHealAnomaly> {
        let mut anomalies = Vec::new();
        for candidate in candidates {
            if let NodeEvaluation::Unscorable { reason } = self.evaluator.evaluate(candidate) {
                anomalies.push(SelfHealAnomaly::NodeUnobserved {
                    node: candidate.name.clone(),
                    reason,
                });
                continue;
            }
            if let Some(latency_ms) = candidate.observation.latency_ms
                && latency_ms > self.latency_ceiling_ms
            {
                anomalies.push(SelfHealAnomaly::ExcessiveLatency {
                    node: candidate.name.clone(),
                    latency_ms,
                });
            }
            if let Some(loss_ratio) = candidate.observation.loss_ratio
                && loss_ratio > self.loss_ceiling
            {
                anomalies.push(SelfHealAnomaly::ExcessiveLoss {
                    node: candidate.name.clone(),
                    loss_ratio,
                });
            }
            if candidate.history.total() >= self.history_min_samples
                && let Some(success_ratio) = candidate.history.success_ratio()
                && success_ratio < self.history_floor
            {
                anomalies.push(SelfHealAnomaly::UnhealthyHistory {
                    node: candidate.name.clone(),
                    success_ratio,
                });
            }
        }
        anomalies
    }
}

impl SelfHealSignalBoard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Refresh from candidate facts through the detector.
    pub fn refresh(&mut self, detector: &ContextAwareHealDetector, candidates: &[NodeCandidate]) {
        self.anomalies = detector.detect(candidates);
        self.best_node = detector
            .best(candidates)
            .map(|candidate| candidate.name.clone());
    }

    /// Highest anomaly severity currently surfaced, if any.
    pub fn highest_severity(&self) -> Option<AnomalySeverity> {
        self.anomalies.iter().map(SelfHealAnomaly::severity).max()
    }

    /// Count of anomalies at or above the given severity.
    pub fn count_at_least(&self, severity: AnomalySeverity) -> usize {
        self.anomalies
            .iter()
            .filter(|anomaly| anomaly.severity() >= severity)
            .count()
    }
}
