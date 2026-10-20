//! Circuit breaker behaviour for a single proxy node and for the cluster-wide
//! registry that tracks the health of every known node.

use super::{CircuitState, MultiNodeCircuitRegistry, NodeCircuitBreaker};
use std::time::Duration;

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
