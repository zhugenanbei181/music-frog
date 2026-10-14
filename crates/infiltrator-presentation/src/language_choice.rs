//! One language transition owner; only a correlated durable success publishes a choice.
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::language::{LanguagePreference, LanguageSettingsSnapshot};
use infiltrator_domain::settings::AppSettings;

pub fn project_language(
    settings: Option<&Result<AppSettings, Failure>>,
) -> LanguageSettingsSnapshot {
    match settings {
        None => LanguageSettingsSnapshot::default(),
        Some(Err(failure)) => LanguageSettingsSnapshot {
            preference: None,
            can_persist: false,
            failure: Some(failure.clone()),
        },
        Some(Ok(settings)) => match LanguagePreference::parse(&settings.language) {
            Ok(preference) => LanguageSettingsSnapshot {
                preference: Some(preference),
                can_persist: true,
                failure: None,
            },
            Err(failure) => LanguageSettingsSnapshot {
                preference: None,
                can_persist: true,
                failure: Some(failure),
            },
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingLanguageChoice {
    pub token: u64,
    pub preference: LanguagePreference,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LanguageReadPhase {
    #[default]
    Unobserved,
    Observed {
        preference: Option<LanguagePreference>,
    },
    Failed {
        failure: Failure,
        last_known: Option<LanguagePreference>,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LanguagePersistence {
    #[default]
    Unavailable,
    Available,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LanguageChoicePhase {
    #[default]
    Ready,
    Pending(PendingLanguageChoice),
    Rejected(Failure),
    Failed {
        requested: LanguagePreference,
        failure: Failure,
    },
    AwaitingObservation {
        preference: LanguagePreference,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanguageChoiceFinish {
    Stale,
    Saved(LanguagePreference),
    Failed(Failure),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LanguageChoiceState {
    pub read: LanguageReadPhase,
    pub persistence: LanguagePersistence,
    pub phase: LanguageChoicePhase,
    next_token: u64,
}

impl LanguageChoiceState {
    pub fn observe(&mut self, snapshot: &LanguageSettingsSnapshot) {
        self.persistence = match snapshot.can_persist {
            true => LanguagePersistence::Available,
            false => LanguagePersistence::Unavailable,
        };

        match &self.phase {
            LanguageChoicePhase::Pending(_) => return,
            LanguageChoicePhase::AwaitingObservation { preference }
                if snapshot.preference != Some(*preference) =>
            {
                return;
            }
            LanguageChoicePhase::AwaitingObservation { .. } => {
                self.phase = LanguageChoicePhase::Ready;
            }
            LanguageChoicePhase::Ready
            | LanguageChoicePhase::Rejected(_)
            | LanguageChoicePhase::Failed { .. } => {}
        }

        let last_known = match &self.read {
            LanguageReadPhase::Unobserved => None,
            LanguageReadPhase::Observed { preference } => *preference,
            LanguageReadPhase::Failed { last_known, .. } => *last_known,
        };
        self.read = match &snapshot.failure {
            Some(failure) => LanguageReadPhase::Failed {
                failure: failure.clone(),
                last_known,
            },
            None => LanguageReadPhase::Observed {
                preference: snapshot.preference,
            },
        };
    }

    pub fn begin(
        &mut self,
        preference: LanguagePreference,
    ) -> Result<PendingLanguageChoice, Failure> {
        if self.persistence != LanguagePersistence::Available {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "language settings store is unavailable",
                true,
            ));
        }
        match &self.phase {
            LanguageChoicePhase::Pending(_) | LanguageChoicePhase::AwaitingObservation { .. } => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "language save is already pending",
                    false,
                ));
            }
            LanguageChoicePhase::Ready
            | LanguageChoicePhase::Rejected(_)
            | LanguageChoicePhase::Failed { .. } => {}
        }
        if self.current_preference() == Some(preference) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "language is already applied",
                false,
            ));
        }
        self.next_token = self.next_token.checked_add(1).ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "language choice token exhausted",
                false,
            )
        })?;
        let pending = PendingLanguageChoice {
            token: self.next_token,
            preference,
        };
        self.phase = LanguageChoicePhase::Pending(pending);
        Ok(pending)
    }

    pub fn begin_retry(&mut self) -> Result<PendingLanguageChoice, Failure> {
        let preference = match &self.phase {
            LanguageChoicePhase::Failed { requested, .. }
                if self.persistence == LanguagePersistence::Available =>
            {
                *requested
            }
            LanguageChoicePhase::Ready
            | LanguageChoicePhase::Pending(_)
            | LanguageChoicePhase::Rejected(_)
            | LanguageChoicePhase::Failed { .. }
            | LanguageChoicePhase::AwaitingObservation { .. } => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "language choice has no retryable failure",
                    false,
                ));
            }
        };
        self.begin(preference)
    }

    pub fn reject(&mut self, failure: Failure) {
        self.phase = LanguageChoicePhase::Rejected(failure);
    }

    /// A demo-only local choice has no durable command receipt.
    pub fn set_local_preference(&mut self, preference: LanguagePreference) {
        self.persistence = LanguagePersistence::Available;
        self.read = LanguageReadPhase::Observed {
            preference: Some(preference),
        };
        self.phase = LanguageChoicePhase::Ready;
    }

    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> LanguageChoiceFinish {
        let pending = match &self.phase {
            LanguageChoicePhase::Pending(pending) if pending.token == token => *pending,
            LanguageChoicePhase::Ready
            | LanguageChoicePhase::Pending(_)
            | LanguageChoicePhase::Rejected(_)
            | LanguageChoicePhase::Failed { .. }
            | LanguageChoicePhase::AwaitingObservation { .. } => {
                return LanguageChoiceFinish::Stale;
            }
        };
        match result {
            Ok(()) => {
                self.read = LanguageReadPhase::Observed {
                    preference: Some(pending.preference),
                };
                self.phase = LanguageChoicePhase::AwaitingObservation {
                    preference: pending.preference,
                };
                LanguageChoiceFinish::Saved(pending.preference)
            }
            Err(failure) => {
                self.phase = LanguageChoicePhase::Failed {
                    requested: pending.preference,
                    failure: failure.clone(),
                };
                LanguageChoiceFinish::Failed(failure)
            }
        }
    }

    fn current_preference(&self) -> Option<LanguagePreference> {
        match &self.read {
            LanguageReadPhase::Unobserved => None,
            LanguageReadPhase::Observed { preference } => *preference,
            LanguageReadPhase::Failed { last_known, .. } => *last_known,
        }
    }

    pub fn can_retry(&self) -> bool {
        matches!(&self.phase, LanguageChoicePhase::Failed { .. })
            && self.persistence == LanguagePersistence::Available
    }
}

#[cfg(test)]
#[path = "language_choice_tests.rs"]
mod tests;
