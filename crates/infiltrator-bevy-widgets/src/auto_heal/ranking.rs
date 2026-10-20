//! Heuristic ranking of proxy nodes by latency, loss and recorded probe history.

use super::{
    HeuristicNodeEvaluator, NodeCandidate, NodeEvaluation, NodeObservation, NodeProbeHistory,
    UnscorableReason,
};

impl NodeProbeHistory {
    pub fn record_success(&mut self) {
        self.successes += 1;
    }

    pub fn record_failure(&mut self) {
        self.failures += 1;
    }

    pub fn total(&self) -> u32 {
        self.successes + self.failures
    }

    /// Observed success ratio, or `None` before any probe was recorded. A
    /// missing history is never fabricated into a healthy `1.0`.
    pub fn success_ratio(&self) -> Option<f32> {
        let total = self.total();
        (total > 0).then(|| self.successes as f32 / total as f32)
    }
}

impl NodeCandidate {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            observation: NodeObservation::default(),
            history: NodeProbeHistory::default(),
        }
    }
}

impl Default for HeuristicNodeEvaluator {
    fn default() -> Self {
        Self {
            latency_weight: 1.0,
            loss_weight: 200.0,
            history_weight: 50.0,
        }
    }
}

impl HeuristicNodeEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Score one candidate. Both latency and loss must be observed; history is
    /// optional context and contributes only when it actually exists.
    pub fn evaluate(&self, candidate: &NodeCandidate) -> NodeEvaluation {
        match (
            candidate.observation.latency_ms,
            candidate.observation.loss_ratio,
        ) {
            (Some(latency_ms), Some(loss_ratio)) => {
                let history_ratio = candidate.history.success_ratio();
                let history_penalty =
                    history_ratio.map_or(0.0, |ratio| (1.0 - ratio) * self.history_weight);
                let score = latency_ms * self.latency_weight
                    + loss_ratio * self.loss_weight
                    + history_penalty;
                NodeEvaluation::Scored {
                    score,
                    history_ratio,
                }
            }
            (None, None) => NodeEvaluation::Unscorable {
                reason: UnscorableReason::LatencyAndLossUnobserved,
            },
            (None, Some(_)) => NodeEvaluation::Unscorable {
                reason: UnscorableReason::LatencyUnobserved,
            },
            (Some(_), None) => NodeEvaluation::Unscorable {
                reason: UnscorableReason::LossUnobserved,
            },
        }
    }

    /// Best scored candidate, or `None` when no candidate had the facts needed
    /// to score. Unscorable candidates are never selected by fabrication.
    pub fn best<'a>(&self, candidates: &'a [NodeCandidate]) -> Option<&'a NodeCandidate> {
        candidates
            .iter()
            .filter_map(|candidate| match self.evaluate(candidate) {
                NodeEvaluation::Scored { score, .. } => Some((candidate, score)),
                NodeEvaluation::Unscorable { .. } => None,
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(candidate, _)| candidate)
    }
}
