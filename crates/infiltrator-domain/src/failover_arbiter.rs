//! Load balancing strategies, health-based failover, consistent hashing, and sticky session routing.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

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
                    NodeHealthState::Degraded { .. } => {}
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

/// Consistent Hash Ring for LoadBalance consistent-hashing proxy strategy.
pub struct ConsistentHashRing {
    vnodes_per_node: usize,
    ring: BTreeMap<u64, String>,
    nodes: Vec<String>,
}

impl ConsistentHashRing {
    pub fn new(vnodes_per_node: usize) -> Self {
        Self {
            vnodes_per_node: vnodes_per_node.max(10),
            ring: BTreeMap::new(),
            nodes: Vec::new(),
        }
    }

    fn hash_key(key: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    pub fn add_node(&mut self, node_name: &str) {
        if !self.nodes.iter().any(|n| n == node_name) {
            self.nodes.push(node_name.to_string());
            for v in 0..self.vnodes_per_node {
                let vnode_key = format!("{node_name}#vn{v}");
                let hash = Self::hash_key(&vnode_key);
                self.ring.insert(hash, node_name.to_string());
            }
        }
    }

    pub fn remove_node(&mut self, node_name: &str) {
        if let Some(pos) = self.nodes.iter().position(|n| n == node_name) {
            self.nodes.remove(pos);
            for v in 0..self.vnodes_per_node {
                let vnode_key = format!("{node_name}#vn{v}");
                let hash = Self::hash_key(&vnode_key);
                self.ring.remove(&hash);
            }
        }
    }

    pub fn get_node<'a>(&'a self, request_key: &str) -> Option<&'a str> {
        if self.ring.is_empty() {
            return None;
        }

        let hash = Self::hash_key(request_key);
        // Find the first vnode with hash >= request hash, or wrap around to the first element
        if let Some((_, node)) = self.ring.range(hash..).next() {
            Some(node.as_str())
        } else if let Some((_, node)) = self.ring.iter().next() {
            Some(node.as_str())
        } else {
            None
        }
    }

    pub fn nodes(&self) -> &[String] {
        &self.nodes
    }
}

/// Sticky Session manager mapping client identifiers (e.g. source IP) to selected nodes with TTL.
pub struct StickySessionManager {
    sessions: HashMap<String, (String, u64)>,
    ttl_secs: u64,
}

impl StickySessionManager {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            sessions: HashMap::new(),
            ttl_secs,
        }
    }

    pub fn get_or_assign(
        &mut self,
        client_key: &str,
        now_secs: u64,
        assign_fn: impl FnOnce() -> Option<String>,
    ) -> Option<String> {
        // Clean up or read existing session
        if let Some((node, last_seen)) = self.sessions.get(client_key)
            && now_secs <= last_seen.saturating_add(self.ttl_secs)
        {
            let node_clone = node.clone();
            self.sessions.insert(client_key.to_string(), (node_clone.clone(), now_secs));
            return Some(node_clone);
        }

        // Assign new session
        if let Some(new_node) = assign_fn() {
            self.sessions.insert(client_key.to_string(), (new_node.clone(), now_secs));
            Some(new_node)
        } else {
            None
        }
    }

    pub fn purge_expired(&mut self, now_secs: u64) {
        let ttl = self.ttl_secs;
        self.sessions.retain(|_, (_, last_seen)| now_secs <= last_seen.saturating_add(ttl));
    }

    pub fn active_session_count(&self) -> usize {
        self.sessions.len()
    }
}

/// Smooth Weighted Round-Robin Selector (Nginx algorithm) for heterogeneous nodes.
#[derive(Debug, Clone)]
pub struct WeightedNodeItem {
    pub name: String,
    pub weight: i32,
    pub current_weight: i32,
}

pub struct WeightedRoundRobinSelector {
    nodes: Vec<WeightedNodeItem>,
}

impl WeightedRoundRobinSelector {
    pub fn new(node_weights: Vec<(String, i32)>) -> Self {
        let nodes = node_weights
            .into_iter()
            .map(|(name, weight)| WeightedNodeItem {
                name,
                weight: weight.max(1),
                current_weight: 0,
            })
            .collect();
        Self { nodes }
    }

    pub fn next_node(&mut self) -> Option<String> {
        if self.nodes.is_empty() {
            return None;
        }

        let total_weight: i32 = self.nodes.iter().map(|n| n.weight).sum();
        if total_weight <= 0 {
            return Some(self.nodes[0].name.clone());
        }

        let mut best_idx = 0;
        let mut max_current = i32::MIN;

        for (i, node) in self.nodes.iter_mut().enumerate() {
            node.current_weight += node.weight;
            if node.current_weight > max_current {
                max_current = node.current_weight;
                best_idx = i;
            }
        }

        self.nodes[best_idx].current_weight -= total_weight;
        Some(self.nodes[best_idx].name.clone())
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

    #[test]
    fn test_consistent_hash_ring() {
        let mut ring = ConsistentHashRing::new(50);
        ring.add_node("HK-01");
        ring.add_node("JP-01");
        ring.add_node("US-01");

        let node1 = ring.get_node("192.168.1.100").unwrap();
        let node2 = ring.get_node("192.168.1.100").unwrap();
        assert_eq!(node1, node2); // Deterministic

        let node_other = ring.get_node("10.0.0.5").unwrap();
        assert!(["HK-01", "JP-01", "US-01"].contains(&node_other));

        ring.remove_node("US-01");
        assert_eq!(ring.nodes().len(), 2);
    }

    #[test]
    fn test_sticky_session_manager() {
        let mut sessions = StickySessionManager::new(300);
        let client_ip = "192.168.1.50";

        let assigned = sessions.get_or_assign(client_ip, 1000, || Some("Node-A".to_string())).unwrap();
        assert_eq!(assigned, "Node-A");

        // Subsequent request within TTL returns same node
        let repeat = sessions.get_or_assign(client_ip, 1200, || Some("Node-B".to_string())).unwrap();
        assert_eq!(repeat, "Node-A");

        // Request after TTL expires gets new assignment
        let expired = sessions.get_or_assign(client_ip, 1600, || Some("Node-C".to_string())).unwrap();
        assert_eq!(expired, "Node-C");

        sessions.purge_expired(2000);
        assert_eq!(sessions.active_session_count(), 0);
    }

    #[test]
    fn test_weighted_round_robin_selector() {
        let mut selector = WeightedRoundRobinSelector::new(vec![
            ("Node-A".to_string(), 4),
            ("Node-B".to_string(), 2),
            ("Node-C".to_string(), 1),
        ]);

        let mut counts = HashMap::new();
        for _ in 0..7 {
            let n = selector.next_node().unwrap();
            *counts.entry(n).or_insert(0) += 1;
        }

        assert_eq!(counts.get("Node-A"), Some(&4));
        assert_eq!(counts.get("Node-B"), Some(&2));
        assert_eq!(counts.get("Node-C"), Some(&1));
    }
}
