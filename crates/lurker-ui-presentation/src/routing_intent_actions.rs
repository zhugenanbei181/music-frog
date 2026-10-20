//! Typed presentation transitions for user-requested routing intent changes.
//!
//! The reducer owns one explicit state machine. Every observation is fenced by
//! a `(generation, revision)` pair, and a pending change carries the token and
//! the exact observation it was based on, so a late receipt of a superseded
//! request can never publish a wrong state.
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::routing_intent::{
    RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
};

/// One in-flight routing intent change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRoutingIntentChange {
    /// The monotonic identity of this change.
    pub token: u64,
    /// The generation the change was based on.
    pub generation: u64,
    /// The observation revision the change was based on.
    pub revision: u64,
    /// The routing intent the user asked for.
    pub target: RoutingIntent,
}

/// One observed routing intent readback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingIntentObservation {
    /// The generation the observation belongs to.
    pub generation: u64,
    /// The monotonic revision of the observation.
    pub revision: u64,
    /// The observed snapshot.
    pub snapshot: RoutingIntentSnapshot,
}

/// The exhaustive phase of the routing intent presentation reducer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RoutingIntentActionPhase {
    /// Nothing has been observed yet.
    #[default]
    Unobserved,
    /// A readback is available.
    Observed(RoutingIntentObservation),
    /// A change is in flight.
    Pending {
        /// The observation the change was based on.
        observation: RoutingIntentObservation,
        /// The change that is in flight.
        change: PendingRoutingIntentChange,
    },
    /// The last change failed.
    Failed {
        /// The observation the failed change was based on.
        observation: RoutingIntentObservation,
        /// The routing intent the user asked for.
        requested: RoutingIntent,
        /// The typed failure of the change.
        failure: Failure,
    },
}

/// The outcome of folding one observation into the reducer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingIntentObserveOutcome {
    /// The reducer had no observation and now has one.
    Initialized,
    /// The observation replaced an older one of the same generation.
    Updated,
    /// The observation belongs to a new generation.
    GenerationChanged,
    /// A newer same-generation readback superseded the in-flight change.
    PendingSuperseded,
    /// The observation was not newer than the current one and was ignored.
    IgnoredStale,
}

/// The outcome of finishing one in-flight change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingIntentFinishOutcome {
    /// The receipt was applied to the reducer.
    Applied,
    /// The receipt did not match the in-flight change and was ignored.
    IgnoredStale,
}

/// The outcome of recording a failure for a routing intent change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingIntentRecordFailureOutcome {
    /// The failure was recorded.
    Recorded,
    /// The in-flight change was preserved; the failure was not recorded.
    PendingPreserved,
}

/// The outcome of dismissing the recorded failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingIntentDismissOutcome {
    /// The recorded failure was dismissed.
    Dismissed,
    /// There was no recorded failure to dismiss.
    NoFailure,
}

/// The presentation reducer of the shared routing intent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoutingIntentActions {
    phase: RoutingIntentActionPhase,
    next_token: u64,
}

impl RoutingIntentActions {
    /// Returns the current phase of the reducer.
    pub fn phase(&self) -> &RoutingIntentActionPhase {
        &self.phase
    }

    /// Folds one routing intent observation into the reducer.
    ///
    /// An observation that is not newer than the current one is ignored; a
    /// newer observation of another generation resets the phase to
    /// [`RoutingIntentActionPhase::Observed`].
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

    /// Starts one routing intent change.
    ///
    /// Fails with an `InvalidState` failure when a change is already pending or
    /// when the requested intent is already active, and with a `NotReady`
    /// failure when the requested intent is not selectable.
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

    /// Records the failure of a routing intent change.
    ///
    /// An in-flight change is preserved: its receipt may still arrive.
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

    /// Folds the receipt of one in-flight change into the reducer.
    ///
    /// A receipt that does not match the in-flight change exactly is ignored.
    /// A receipt that reports another intent than the requested one is
    /// recorded as a retryable failure.
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

    /// Returns the in-flight change, when one is pending.
    pub fn pending_change(&self) -> Option<&PendingRoutingIntentChange> {
        match &self.phase {
            RoutingIntentActionPhase::Pending { change, .. } => Some(change),
            RoutingIntentActionPhase::Unobserved
            | RoutingIntentActionPhase::Observed(_)
            | RoutingIntentActionPhase::Failed { .. } => None,
        }
    }

    /// Returns the recorded failure, when the phase carries one.
    pub fn failure(&self) -> Option<&Failure> {
        match &self.phase {
            RoutingIntentActionPhase::Failed { failure, .. } => Some(failure),
            RoutingIntentActionPhase::Unobserved
            | RoutingIntentActionPhase::Observed(_)
            | RoutingIntentActionPhase::Pending { .. } => None,
        }
    }

    /// Returns the routing intent the user asked for, when one is known.
    pub fn requested_target(&self) -> Option<&RoutingIntent> {
        match &self.phase {
            RoutingIntentActionPhase::Pending { change, .. } => Some(&change.target),
            RoutingIntentActionPhase::Failed { requested, .. } => Some(requested),
            RoutingIntentActionPhase::Unobserved | RoutingIntentActionPhase::Observed(_) => None,
        }
    }

    /// Returns the intent a retry should ask for, when the failure allows it.
    ///
    /// Only a retryable failure on a still-selectable, not-yet-active intent
    /// can be retried.
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

    /// Whether the failure asks the user for controller settings.
    pub fn needs_controller_settings(&self) -> bool {
        matches!(
            self.failure().map(|failure| &failure.code),
            Some(ErrorCode::Authentication | ErrorCode::Permission)
        )
    }

    /// Dismisses the recorded failure and returns to the observed phase.
    pub fn dismiss_failure(&mut self) -> RoutingIntentDismissOutcome {
        let RoutingIntentActionPhase::Failed { observation, .. } = &self.phase else {
            return RoutingIntentDismissOutcome::NoFailure;
        };
        self.phase = RoutingIntentActionPhase::Observed(observation.clone());
        RoutingIntentDismissOutcome::Dismissed
    }

    /// Drops every observation and returns to the unobserved phase.
    pub fn invalidate(&mut self) {
        self.phase = RoutingIntentActionPhase::Unobserved;
    }

    /// Returns the revision the next observation should carry.
    pub fn next_observation_revision(&self) -> u64 {
        self.phase
            .observation()
            .map(|observation| observation.revision)
            .unwrap_or_default()
            .checked_add(1)
            .expect("routing intent observation revision exhausted")
    }

    /// Returns the snapshot a surface should render.
    ///
    /// A pending change is rendered as a pending snapshot of the observation
    /// it was based on.
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
