use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeHealthState {
    Healthy,
    Degraded { consecutive_fails: u32 },
    Failed { failed_at_secs: u64 },
    Recovering,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeHealthStatus {
    pub node_name: String,
    pub state: NodeHealthState,
    pub latency_ms: Option<u32>,
}

pub struct FailoverArbiter {
    failure_threshold: u32,
    recovery_cooldown_secs: u64,
    nodes: HashMap<String, NodeHealthStatus>,
}

impl FailoverArbiter {
    pub fn new(failure_threshold: u32, recovery_cooldown_secs: u64) -> Self {
        Self {
            failure_threshold,
            recovery_cooldown_secs,
            nodes: HashMap::new(),
        }
    }

    pub fn report_success(&mut self, node_name: &str, latency_ms: u32) {
        let entry = self
            .nodes
            .entry(node_name.to_string())
            .or_insert_with(|| NodeHealthStatus {
                node_name: node_name.to_string(),
                state: NodeHealthState::Healthy,
                latency_ms: None,
            });

        entry.state = NodeHealthState::Healthy;
        entry.latency_ms = Some(latency_ms);
    }

    pub fn report_failure(&mut self, node_name: &str, now_secs: u64) {
        let entry = self
            .nodes
            .entry(node_name.to_string())
            .or_insert_with(|| NodeHealthStatus {
                node_name: node_name.to_string(),
                state: NodeHealthState::Healthy,
                latency_ms: None,
            });

        match entry.state {
            NodeHealthState::Healthy | NodeHealthState::Recovering => {
                if self.failure_threshold <= 1 {
                    entry.state = NodeHealthState::Failed {
                        failed_at_secs: now_secs,
                    };
                } else {
                    entry.state = NodeHealthState::Degraded {
                        consecutive_fails: 1,
                    };
                }
            }
            NodeHealthState::Degraded { consecutive_fails } => {
                let next_fails = consecutive_fails + 1;
                if next_fails >= self.failure_threshold {
                    entry.state = NodeHealthState::Failed {
                        failed_at_secs: now_secs,
                    };
                } else {
                    entry.state = NodeHealthState::Degraded {
                        consecutive_fails: next_fails,
                    };
                }
            }
            NodeHealthState::Failed { .. } => {
                entry.state = NodeHealthState::Failed {
                    failed_at_secs: now_secs,
                };
            }
        }
    }

    pub fn elect_active_node<'a>(
        &self,
        candidates: &'a [String],
        now_secs: u64,
    ) -> Option<&'a str> {
        let mut healthy_candidates: Vec<(&'a str, u32)> = Vec::new();
        let mut recovering_candidates: Vec<&'a str> = Vec::new();

        for candidate in candidates {
            if let Some(status) = self.nodes.get(candidate) {
                match status.state {
                    NodeHealthState::Healthy => {
                        healthy_candidates
                            .push((candidate.as_str(), status.latency_ms.unwrap_or(u32::MAX)));
                    }
                    NodeHealthState::Degraded { .. } => {
                        // Only prioritize explicitly Healthy over Degraded? Requirements didn't specify Degraded priority, but implied Healthy.
                        // We can just treat them as healthy with a penalty, or skip them if there are fully healthy nodes.
                        // Let's treat them as healthy for now but maybe later.
                        // Wait, it says "Prioritizes Healthy nodes with lowest latency"
                    }
                    NodeHealthState::Failed { failed_at_secs } => {
                        if now_secs >= failed_at_secs + self.recovery_cooldown_secs {
                            recovering_candidates.push(candidate.as_str());
                        }
                    }
                    NodeHealthState::Recovering => {
                        recovering_candidates.push(candidate.as_str());
                    }
                }
            } else {
                healthy_candidates.push((candidate.as_str(), u32::MAX));
            }
        }

        if !healthy_candidates.is_empty() {
            healthy_candidates.sort_by_key(|&(_, lat)| lat);
            Some(healthy_candidates[0].0)
        } else if !recovering_candidates.is_empty() {
            Some(recovering_candidates[0])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_transitions_to_healthy() {
        let mut arbiter = FailoverArbiter::new(3, 10);
        arbiter.report_success("node1", 50);

        let status = arbiter.nodes.get("node1").unwrap();
        assert_eq!(status.state, NodeHealthState::Healthy);
        assert_eq!(status.latency_ms, Some(50));
    }

    #[test]
    fn test_consecutive_failures_degrade_and_fail() {
        let mut arbiter = FailoverArbiter::new(3, 10);

        arbiter.report_failure("node1", 100);
        assert_eq!(
            arbiter.nodes.get("node1").unwrap().state,
            NodeHealthState::Degraded {
                consecutive_fails: 1
            }
        );

        arbiter.report_failure("node1", 101);
        assert_eq!(
            arbiter.nodes.get("node1").unwrap().state,
            NodeHealthState::Degraded {
                consecutive_fails: 2
            }
        );

        arbiter.report_failure("node1", 102);
        assert_eq!(
            arbiter.nodes.get("node1").unwrap().state,
            NodeHealthState::Failed {
                failed_at_secs: 102
            }
        );
    }

    #[test]
    fn test_cooldown_enables_recovering_election() {
        let mut arbiter = FailoverArbiter::new(1, 10);
        arbiter.report_failure("node1", 100);

        let candidates = vec!["node1".to_string()];

        // Before cooldown
        assert_eq!(arbiter.elect_active_node(&candidates, 105), None);

        // After cooldown
        assert_eq!(arbiter.elect_active_node(&candidates, 110), Some("node1"));
    }

    #[test]
    fn test_active_node_election_priority() {
        let mut arbiter = FailoverArbiter::new(3, 10);
        arbiter.report_success("node1", 100);
        arbiter.report_success("node2", 50); // Lowest latency
        arbiter.report_failure("node3", 100); // Failed
        arbiter.report_failure("node3", 100);
        arbiter.report_failure("node3", 100);

        let candidates = vec![
            "node1".to_string(),
            "node2".to_string(),
            "node3".to_string(),
        ];
        assert_eq!(arbiter.elect_active_node(&candidates, 105), Some("node2"));
    }
}
