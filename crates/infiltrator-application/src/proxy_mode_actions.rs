//! One peer-product mode transition; writes never manufacture lifecycle or read facts.
use crate::proxy_mode_application::ProxyModeApplication;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxy_mode::{ProxyModeSnapshot, ProxyModeStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingModeChange {
    pub token: u64,
    pub generation: u64,
    pub target: ProxyMode,
}

#[derive(Clone, Debug, Default)]
pub struct ProxyModeActions {
    pub observed: ProxyModeSnapshot,
    pub pending: Option<PendingModeChange>,
    pub requested: Option<ProxyMode>,
    pub failure: Option<Failure>,
    generation: u64,
    revision: u64,
    next_token: u64,
    initialized: bool,
    read_epoch: u64,
}

impl ProxyModeActions {
    pub fn observe(&mut self, generation: u64, revision: u64, observed: ProxyModeSnapshot) {
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

    pub fn begin(&mut self, target: ProxyMode) -> Result<PendingModeChange, Failure> {
        if self.pending.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "A mode change is already pending",
                false,
            ));
        }
        ProxyModeApplication::intent(&self.observed, target)?;
        self.next_token = self
            .next_token
            .checked_add(1)
            .expect("mode request identity exhausted");
        let pending = PendingModeChange {
            token: self.next_token,
            generation: self.generation,
            target,
        };
        self.requested = Some(target);
        self.pending = Some(pending);
        self.advance_read_epoch();
        self.failure = None;
        Ok(pending)
    }

    pub fn finish(
        &mut self,
        pending: PendingModeChange,
        result: Result<ProxyMode, Failure>,
    ) -> bool {
        if self.pending != Some(pending) || self.generation != pending.generation {
            return false;
        }
        self.pending = None;
        self.revision = self
            .revision
            .checked_add(1)
            .expect("mode observation revision exhausted");
        self.advance_read_epoch();
        match result {
            Ok(actual) if actual == pending.target => {
                self.observed.current = Some(actual);
                self.requested = None;
                self.failure = None;
            }
            Ok(actual) => {
                self.observed.current = Some(actual);
                self.failure = Some(Failure::new(
                    ErrorCode::InvalidState,
                    format!(
                        "Controller reported {} after requesting {}",
                        actual.to_wire(),
                        pending.target.to_wire()
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

    pub fn retry_target(&self) -> Option<ProxyMode> {
        self.failure.as_ref().filter(|failure| failure.retryable)?;
        self.requested.filter(|target| {
            self.observed.is_mode_selectable(*target) && self.observed.current != Some(*target)
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
        self.advance_read_epoch();
        self.pending = None;
        self.requested = None;
        self.failure = None;
        self.observed = ProxyModeSnapshot::default();
        self.initialized = false;
    }

    pub fn next_observation_revision(&self) -> u64 {
        self.revision
            .checked_add(1)
            .expect("mode observation revision exhausted")
    }

    pub fn read_epoch(&self) -> u64 {
        self.read_epoch
    }

    pub fn accepts_read(&self, epoch: u64) -> bool {
        self.pending.is_none() && epoch == self.read_epoch
    }

    fn advance_read_epoch(&mut self) {
        self.read_epoch = self
            .read_epoch
            .checked_add(1)
            .expect("mode read fence exhausted");
    }

    pub fn render_snapshot(&self) -> ProxyModeSnapshot {
        let mut snapshot = self.observed.clone();
        if self.pending.is_some() {
            snapshot.status = ProxyModeStatus::Pending;
        }
        snapshot
    }
}

#[cfg(test)]
#[path = "proxy_mode_actions_tests.rs"]
mod tests;
