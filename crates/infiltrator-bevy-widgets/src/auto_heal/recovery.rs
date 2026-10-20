//! Reconnect backoff, transactional rollback and failover routing behaviour.

use super::{AutoRollbackTransaction, ExponentialBackoffPolicy, FailoverRoutingGraph};
use std::collections::HashSet;
use std::time::Duration;

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
        let mut visited = HashSet::new();
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
        let mut visited = HashSet::new();
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
