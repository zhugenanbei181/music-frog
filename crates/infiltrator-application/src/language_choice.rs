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
#[derive(Clone, Debug, Default)]
pub struct LanguageChoiceState {
    pub applied: Option<LanguagePreference>,
    pub requested: Option<LanguagePreference>,
    pub pending: Option<PendingLanguageChoice>,
    pub failure: Option<Failure>,
    pub can_persist: bool,
    awaiting: Option<LanguagePreference>,
    read_failure: Option<Failure>,
    next_token: u64,
}
impl LanguageChoiceState {
    pub fn observe(&mut self, snapshot: &LanguageSettingsSnapshot) {
        self.can_persist = snapshot.can_persist;
        if let Some(expected) = self.awaiting {
            if snapshot.preference != Some(expected) {
                return;
            }
            self.awaiting = None;
        }
        if self.pending.is_some() {
            return;
        }
        if snapshot.failure.is_some() {
            if self.failure.is_none() || self.failure == self.read_failure {
                self.failure = snapshot.failure.clone();
            }
            self.read_failure = snapshot.failure.clone();
            return;
        }
        if self.failure == self.read_failure {
            self.failure = None;
        }
        self.read_failure = None;
        self.applied = snapshot.preference;
    }
    pub fn begin(
        &mut self,
        preference: LanguagePreference,
    ) -> Result<PendingLanguageChoice, Failure> {
        if !self.can_persist {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "language settings store is unavailable",
                true,
            ));
        }
        if self.pending.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "language save is already pending",
                false,
            ));
        }
        if self.applied == Some(preference) {
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
        self.requested = Some(preference);
        self.pending = Some(pending);
        self.failure = None;
        Ok(pending)
    }
    pub fn finish(
        &mut self,
        token: u64,
        result: Result<(), Failure>,
    ) -> Option<Result<LanguagePreference, Failure>> {
        let pending = self.pending.filter(|pending| pending.token == token)?;
        self.pending = None;
        Some(match result {
            Ok(()) => {
                self.applied = Some(pending.preference);
                self.awaiting = Some(pending.preference);
                self.requested = None;
                self.failure = None;
                Ok(pending.preference)
            }
            Err(failure) => {
                self.failure = Some(failure.clone());
                Err(failure)
            }
        })
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
        state.observe(&original);
        let pending = state.begin(LanguagePreference::English).unwrap();
        assert!(state.begin(LanguagePreference::System).is_err());
        assert!(state.finish(pending.token + 1, Ok(())).is_none());
        let failure = Failure::new(ErrorCode::Storage, "save failed", true);
        assert_eq!(
            state.finish(pending.token, Err(failure.clone())),
            Some(Err(failure))
        );
        assert_eq!(state.applied, original.preference);
        let retry = state.begin(state.requested.unwrap()).unwrap();
        assert_eq!(
            state.finish(retry.token, Ok(())),
            Some(Ok(LanguagePreference::English))
        );
        state.observe(&original);
        assert_eq!(state.applied, Some(LanguagePreference::English));
        assert!(state.finish(pending.token, Ok(())).is_none());
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
    fn successful_refresh_clears_only_read_failures_and_keeps_unsaved_command_failures() {
        let good = project_language(Some(&Ok(AppSettings::default())));
        let read = Failure::new(ErrorCode::Storage, "read denied", true);
        let mut state = LanguageChoiceState::default();
        state.observe(&good);
        state.observe(&project_language(Some(&Err(read.clone()))));
        assert_eq!(state.failure, Some(read));
        assert_eq!(state.applied, good.preference);
        state.observe(&good);
        assert!(state.failure.is_none());
        let pending = state.begin(LanguagePreference::English).unwrap();
        let write = Failure::new(ErrorCode::Storage, "write denied", true);
        state.finish(pending.token, Err(write.clone()));
        state.observe(&good);
        assert_eq!(state.failure, Some(write));
        assert_eq!(state.requested, Some(LanguagePreference::English));
    }
}
