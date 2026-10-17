//! Typed presentation transitions for user-requested routing intent changes.
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRoutingIntentChange {
    pub token: u64,
    pub generation: u64,
    pub revision: u64,
    pub target: RoutingIntent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingIntentObservation {
    pub generation: u64,
    pub revision: u64,
    pub snapshot: RoutingIntentSnapshot,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RoutingIntentActionPhase {
    #[default]
    Unobserved,
    Observed(RoutingIntentObservation),
    Pending {
        observation: RoutingIntentObservation,
        change: PendingRoutingIntentChange,
    },
    Failed {
        observation: RoutingIntentObservation,
        requested: RoutingIntent,
        failure: Failure,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingIntentObserveOutcome {
    Initialized,
    Updated,
    GenerationChanged,
    PendingSuperseded,
    IgnoredStale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingIntentFinishOutcome {
    Applied,
    IgnoredStale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingIntentRecordFailureOutcome {
    Recorded,
    PendingPreserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingIntentDismissOutcome {
    Dismissed,
    NoFailure,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoutingIntentActions {
    phase: RoutingIntentActionPhase,
    next_token: u64,
}

impl RoutingIntentActions {
    pub fn phase(&self) -> &RoutingIntentActionPhase {
        &self.phase
    }

    pub fn observe(
        &mut self,
        generation: u64,
        revision: u64,
        snapshot: RoutingIntentSnapshot,
    ) -> RoutingIntentObserveOutcome {
        let incoming = RoutingIntentObservation {
            generation,
            revision,
            snapshot,
        };
        match &self.phase {
            RoutingIntentActionPhase::Unobserved => {
                self.phase = RoutingIntentActionPhase::Observed(incoming);
                RoutingIntentObserveOutcome::Initialized
            }
            RoutingIntentActionPhase::Observed(current) => {
                if !is_newer(&incoming, current) {
                    return RoutingIntentObserveOutcome::IgnoredStale;
                }
                let outcome = generation_outcome(&incoming, current);
                self.phase = RoutingIntentActionPhase::Observed(incoming);
                outcome
            }
            RoutingIntentActionPhase::Pending { observation, .. } => {
                if !is_newer(&incoming, observation) {
                    return RoutingIntentObserveOutcome::IgnoredStale;
                }
                if incoming.generation != observation.generation {
                    self.phase = RoutingIntentActionPhase::Observed(incoming);
                    RoutingIntentObserveOutcome::GenerationChanged
                } else {
                    // A newer same-generation readback supersedes the in-flight
                    // intent. Its eventual receipt is stale and cannot publish.
                    self.phase = RoutingIntentActionPhase::Observed(incoming);
                    RoutingIntentObserveOutcome::PendingSuperseded
                }
            }
            RoutingIntentActionPhase::Failed {
                observation,
                requested,
                failure,
            } => {
                if !is_newer(&incoming, observation) {
                    return RoutingIntentObserveOutcome::IgnoredStale;
                }
                if incoming.generation != observation.generation {
                    self.phase = RoutingIntentActionPhase::Observed(incoming);
                    RoutingIntentObserveOutcome::GenerationChanged
                } else {
                    self.phase = RoutingIntentActionPhase::Failed {
                        observation: incoming,
                        requested: requested.clone(),
                        failure: failure.clone(),
                    };
                    RoutingIntentObserveOutcome::Updated
                }
            }
        }
    }

    pub fn begin(&mut self, target: RoutingIntent) -> Result<PendingRoutingIntentChange, Failure> {
        let observation = match &self.phase {
            RoutingIntentActionPhase::Unobserved => RoutingIntentObservation {
                generation: 0,
                revision: 0,
                snapshot: RoutingIntentSnapshot::default(),
            },
            RoutingIntentActionPhase::Observed(observation)
            | RoutingIntentActionPhase::Failed { observation, .. } => observation.clone(),
            RoutingIntentActionPhase::Pending { .. } => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "A routing intent change is already pending",
                    false,
                ));
            }
        };

        if !observation.snapshot.is_intent_selectable(&target) {
            return Err(observation.snapshot.failure.clone().unwrap_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "The requested routing intent is unavailable, unobserved, unsupported or pending",
                    true,
                )
            }));
        }
        if observation.snapshot.current.as_ref() == Some(&target) {
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
        let change = PendingRoutingIntentChange {
            token: self.next_token,
            generation: observation.generation,
            revision: observation.revision,
            target,
        };
        self.phase = RoutingIntentActionPhase::Pending {
            observation,
            change: change.clone(),
        };
        Ok(change)
    }

    pub fn record_failure(
        &mut self,
        requested: RoutingIntent,
        failure: Failure,
    ) -> RoutingIntentRecordFailureOutcome {
        let observation = match &self.phase {
            RoutingIntentActionPhase::Pending { .. } => {
                return RoutingIntentRecordFailureOutcome::PendingPreserved;
            }
            RoutingIntentActionPhase::Unobserved => RoutingIntentObservation {
                generation: 0,
                revision: 0,
                snapshot: RoutingIntentSnapshot::default(),
            },
            RoutingIntentActionPhase::Observed(observation)
            | RoutingIntentActionPhase::Failed { observation, .. } => observation.clone(),
        };
        self.phase = RoutingIntentActionPhase::Failed {
            observation,
            requested,
            failure,
        };
        RoutingIntentRecordFailureOutcome::Recorded
    }

    pub fn finish(
        &mut self,
        change: PendingRoutingIntentChange,
        result: Result<RoutingIntent, Failure>,
    ) -> RoutingIntentFinishOutcome {
        let observation = match &self.phase {
            RoutingIntentActionPhase::Pending {
                observation,
                change: current,
            } if current == &change
                && observation.generation == change.generation
                && observation.revision == change.revision =>
            {
                observation.clone()
            }
            RoutingIntentActionPhase::Unobserved
            | RoutingIntentActionPhase::Observed(_)
            | RoutingIntentActionPhase::Pending { .. }
            | RoutingIntentActionPhase::Failed { .. } => {
                return RoutingIntentFinishOutcome::IgnoredStale;
            }
        };
        let mut terminal = RoutingIntentObservation {
            revision: observation
                .revision
                .checked_add(1)
                .expect("routing intent observation revision exhausted"),
            ..observation
        };
        match result {
            Ok(actual) if actual == change.target => {
                terminal.snapshot.current = Some(actual);
                terminal.snapshot.status = RoutingIntentStatus::Ready;
                terminal.snapshot.failure = None;
                self.phase = RoutingIntentActionPhase::Observed(terminal);
            }
            Ok(actual) => {
                terminal.snapshot.current = Some(actual.clone());
                terminal.snapshot.status = RoutingIntentStatus::Ready;
                terminal.snapshot.failure = None;
                self.phase = RoutingIntentActionPhase::Failed {
                    observation: terminal,
                    requested: change.target.clone(),
                    failure: Failure::new(
                        ErrorCode::InvalidState,
                        format!(
                            "Core reported routing intent {actual:?} after requesting {:?}",
                            change.target
                        ),
                        true,
                    ),
                };
            }
            Err(failure) => {
                self.phase = RoutingIntentActionPhase::Failed {
                    observation: terminal,
                    requested: change.target,
                    failure,
                };
            }
        }
        RoutingIntentFinishOutcome::Applied
    }

    pub fn pending_change(&self) -> Option<&PendingRoutingIntentChange> {
        match &self.phase {
            RoutingIntentActionPhase::Pending { change, .. } => Some(change),
            RoutingIntentActionPhase::Unobserved
            | RoutingIntentActionPhase::Observed(_)
            | RoutingIntentActionPhase::Failed { .. } => None,
        }
    }

    pub fn failure(&self) -> Option<&Failure> {
        match &self.phase {
            RoutingIntentActionPhase::Failed { failure, .. } => Some(failure),
            RoutingIntentActionPhase::Unobserved
            | RoutingIntentActionPhase::Observed(_)
            | RoutingIntentActionPhase::Pending { .. } => None,
        }
    }

    pub fn requested_target(&self) -> Option<&RoutingIntent> {
        match &self.phase {
            RoutingIntentActionPhase::Pending { change, .. } => Some(&change.target),
            RoutingIntentActionPhase::Failed { requested, .. } => Some(requested),
            RoutingIntentActionPhase::Unobserved | RoutingIntentActionPhase::Observed(_) => None,
        }
    }

    pub fn retry_target(&self) -> Option<RoutingIntent> {
        let RoutingIntentActionPhase::Failed {
            observation,
            requested,
            failure,
        } = &self.phase
        else {
            return None;
        };
        if !failure.retryable {
            return None;
        }
        observation
            .snapshot
            .is_intent_selectable(requested)
            .then_some(requested.clone())
            .filter(|target| observation.snapshot.current.as_ref() != Some(target))
    }

    pub fn needs_controller_settings(&self) -> bool {
        matches!(
            self.failure().map(|failure| &failure.code),
            Some(ErrorCode::Authentication | ErrorCode::Permission)
        )
    }

    pub fn dismiss_failure(&mut self) -> RoutingIntentDismissOutcome {
        let RoutingIntentActionPhase::Failed { observation, .. } = &self.phase else {
            return RoutingIntentDismissOutcome::NoFailure;
        };
        self.phase = RoutingIntentActionPhase::Observed(observation.clone());
        RoutingIntentDismissOutcome::Dismissed
    }

    pub fn invalidate(&mut self) {
        self.phase = RoutingIntentActionPhase::Unobserved;
    }

    pub fn next_observation_revision(&self) -> u64 {
        self.phase
            .observation()
            .map(|observation| observation.revision)
            .unwrap_or_default()
            .checked_add(1)
            .expect("routing intent observation revision exhausted")
    }

    pub fn render_snapshot(&self) -> RoutingIntentSnapshot {
        match &self.phase {
            RoutingIntentActionPhase::Unobserved => RoutingIntentSnapshot::default(),
            RoutingIntentActionPhase::Observed(observation)
            | RoutingIntentActionPhase::Failed { observation, .. } => observation.snapshot.clone(),
            RoutingIntentActionPhase::Pending { observation, .. } => {
                let mut snapshot = observation.snapshot.clone();
                snapshot.status = RoutingIntentStatus::Pending;
                snapshot
            }
        }
    }
}

impl RoutingIntentActionPhase {
    fn observation(&self) -> Option<&RoutingIntentObservation> {
        match self {
            Self::Unobserved => None,
            Self::Observed(observation)
            | Self::Pending { observation, .. }
            | Self::Failed { observation, .. } => Some(observation),
        }
    }
}

fn is_newer(incoming: &RoutingIntentObservation, current: &RoutingIntentObservation) -> bool {
    (incoming.generation, incoming.revision) > (current.generation, current.revision)
}

fn generation_outcome(
    incoming: &RoutingIntentObservation,
    current: &RoutingIntentObservation,
) -> RoutingIntentObserveOutcome {
    if incoming.generation == current.generation {
        RoutingIntentObserveOutcome::Updated
    } else {
        RoutingIntentObserveOutcome::GenerationChanged
    }
}

#[cfg(test)]
#[path = "routing_intent_actions_tests.rs"]
mod tests;
