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
        self.next_token = self.next_token.wrapping_add(1);
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
mod tests {
    use super::*;

    #[test]
    fn durable_failure_stale_results_retry_and_old_reader_cannot_publish_or_undo_a_choice() {
        let original = LanguageSettingsSnapshot {
            preference: Some(LanguagePreference::SimplifiedChinese),
            can_persist: true,
            failure: None,
        };
        let mut state = LanguageChoiceState::default();
        assert!(state.begin_retry().is_err());
        state.observe(&original);
        let pending = state.begin(LanguagePreference::English).unwrap();
        assert!(state.begin(LanguagePreference::System).is_err());
        assert_eq!(
            state.finish(pending.token + 1, Ok(())),
            LanguageChoiceFinish::Stale
        );
        let failure = Failure::new(ErrorCode::Storage, "save failed", true);
        assert_eq!(
            state.finish(pending.token, Err(failure.clone())),
            LanguageChoiceFinish::Failed(failure.clone())
        );
        assert_eq!(state.current_preference(), original.preference);
        let retry_preference = match &state.phase {
            LanguageChoicePhase::Failed { requested, .. } => *requested,
            LanguageChoicePhase::Ready
            | LanguageChoicePhase::Pending(_)
            | LanguageChoicePhase::Rejected(_)
            | LanguageChoicePhase::AwaitingObservation { .. } => {
                panic!("failed save retains its requested choice")
            }
        };
        let retry = state.begin_retry().unwrap();
        assert_eq!(retry.preference, retry_preference);
        assert_eq!(
            state.finish(retry.token, Ok(())),
            LanguageChoiceFinish::Saved(LanguagePreference::English)
        );
        state.observe(&original);
        assert_eq!(
            state.current_preference(),
            Some(LanguagePreference::English)
        );
        assert_eq!(
            state.finish(pending.token, Ok(())),
            LanguageChoiceFinish::Stale
        );
    }

    #[test]
    fn invalid_stored_language_and_read_failure_do_not_invent_a_saved_choice() {
        let settings = AppSettings {
            language: "unsupported".into(),
            ..Default::default()
        };
        let snapshot = project_language(Some(&Ok(settings)));
        assert_eq!(snapshot.preference, None);
        assert!(snapshot.failure.is_some());
        assert!(snapshot.can_persist);
        let failure = Failure::new(ErrorCode::Storage, "read failed", true);
        let snapshot = project_language(Some(&Err(failure.clone())));
        assert_eq!(snapshot.failure, Some(failure));
        assert!(!snapshot.can_persist);
    }

    #[test]
    fn successful_refresh_clears_read_failures_without_erasing_write_failures() {
        let good = project_language(Some(&Ok(AppSettings::default())));
        let read = Failure::new(ErrorCode::Storage, "read denied", true);
        let mut state = LanguageChoiceState::default();
        state.observe(&good);
        state.observe(&project_language(Some(&Err(read.clone()))));
        assert_eq!(
            state.read,
            LanguageReadPhase::Failed {
                failure: read,
                last_known: good.preference,
            }
        );
        assert_eq!(state.current_preference(), good.preference);
        state.observe(&good);
        assert_eq!(
            state.read,
            LanguageReadPhase::Observed {
                preference: good.preference
            }
        );
        let pending = state.begin(LanguagePreference::English).unwrap();
        let write = Failure::new(ErrorCode::Storage, "write denied", true);
        state.finish(pending.token, Err(write.clone()));
        state.observe(&good);
        assert_eq!(
            state.phase,
            LanguageChoicePhase::Failed {
                requested: LanguagePreference::English,
                failure: write,
            }
        );
        assert_eq!(state.current_preference(), good.preference);
    }

    #[test]
    fn local_demo_choice_and_rejection_are_typed_phases() {
        let mut state = LanguageChoiceState::default();
        state.set_local_preference(LanguagePreference::English);
        assert_eq!(state.phase, LanguageChoicePhase::Ready);
        assert_eq!(
            state.current_preference(),
            Some(LanguagePreference::English)
        );
        let failure = Failure::new(ErrorCode::InvalidInput, "invalid language", false);
        state.reject(failure.clone());
        assert_eq!(state.phase, LanguageChoicePhase::Rejected(failure));
    }
}
