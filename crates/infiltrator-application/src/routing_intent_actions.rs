//! Shared transition state for user-requested routing intent changes.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRoutingIntentChange {
    pub token: u64,
    pub generation: u64,
    pub target: RoutingIntent,
}

#[derive(Clone, Debug, Default)]
pub struct RoutingIntentActions {
    pub observed: RoutingIntentSnapshot,
    pub pending: Option<PendingRoutingIntentChange>,
    pub requested: Option<RoutingIntent>,
    pub failure: Option<Failure>,
    generation: u64,
    revision: u64,
    next_token: u64,
    initialized: bool,
}

impl RoutingIntentActions {
    pub fn observe(&mut self, generation: u64, revision: u64, observed: RoutingIntentSnapshot) {
        if self.initialized && (generation, revision) <= (self.generation, self.revision) {
            return;
        }
        if generation != self.generation {
            self.pending = None;
            self.requested = None;
            self.failure = None;
        }
        self.initialized = true;
        self.generation = generation;
        self.revision = revision;
        self.observed = observed;
    }

    pub fn begin(&mut self, target: RoutingIntent) -> Result<PendingRoutingIntentChange, Failure> {
        if self.pending.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "A routing intent change is already pending",
                false,
            ));
        }
        if !self.observed.is_intent_selectable(&target) {
            return Err(self.observed.failure.clone().unwrap_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "The requested routing intent is unavailable, unobserved, unsupported or pending",
                    true,
                )
            }));
        }
        if self.observed.current.as_ref() == Some(&target) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "The requested routing intent is already active",
                false,
            ));
        }

        self.next_token = self
            .next_token
            .checked_add(1)
            .expect("routing intent request identity exhausted");
        let pending = PendingRoutingIntentChange {
            token: self.next_token,
            generation: self.generation,
            target: target.clone(),
        };
        self.requested = Some(target);
        self.pending = Some(pending.clone());
        self.failure = None;
        Ok(pending)
    }

    pub fn finish(
        &mut self,
        pending: PendingRoutingIntentChange,
        result: Result<RoutingIntent, Failure>,
    ) -> bool {
        if self.pending.as_ref() != Some(&pending) || self.generation != pending.generation {
            return false;
        }
        self.pending = None;
        self.revision = self
            .revision
            .checked_add(1)
            .expect("routing intent observation revision exhausted");
        match result {
            Ok(actual) if actual == pending.target => {
                self.observed.current = Some(actual);
                self.observed.status = RoutingIntentStatus::Ready;
                self.observed.failure = None;
                self.requested = None;
                self.failure = None;
            }
            Ok(actual) => {
                self.observed.current = Some(actual.clone());
                self.observed.status = RoutingIntentStatus::Ready;
                self.failure = Some(Failure::new(
                    ErrorCode::InvalidState,
                    format!(
                        "Core reported routing intent {actual:?} after requesting {:?}",
                        pending.target
                    ),
                    true,
                ));
            }
            Err(failure) => {
                self.failure = Some(failure);
            }
        }
        true
    }

    pub fn retry_target(&self) -> Option<RoutingIntent> {
        self.failure.as_ref().filter(|failure| failure.retryable)?;
        self.requested.clone().filter(|target| {
            self.observed.is_intent_selectable(target)
                && self.observed.current.as_ref() != Some(target)
        })
    }

    pub fn needs_controller_settings(&self) -> bool {
        self.failure.as_ref().is_some_and(|failure| {
            matches!(
                failure.code,
                ErrorCode::Authentication | ErrorCode::Permission
            )
        })
    }

    pub fn dismiss_failure(&mut self) {
        if self.pending.is_none() {
            self.failure = None;
            self.requested = None;
        }
    }

    pub fn invalidate(&mut self) {
        self.pending = None;
        self.requested = None;
        self.failure = None;
        self.observed = RoutingIntentSnapshot::default();
        self.initialized = false;
    }

    pub fn next_observation_revision(&self) -> u64 {
        self.revision
            .checked_add(1)
            .expect("routing intent observation revision exhausted")
    }

    pub fn render_snapshot(&self) -> RoutingIntentSnapshot {
        let mut snapshot = self.observed.clone();
        if self.pending.is_some() {
            snapshot.status = RoutingIntentStatus::Pending;
        }
        snapshot
    }
}

#[cfg(test)]
#[path = "routing_intent_actions_tests.rs"]
mod tests;
