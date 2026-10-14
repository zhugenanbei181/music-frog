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

#[test]
fn token_exhaustion_is_a_typed_rejection_without_a_partial_transition() {
    let mut state = LanguageChoiceState::default();
    state.persistence = LanguagePersistence::Available;
    state.next_token = u64::MAX;

    assert_eq!(
        state.begin(LanguagePreference::English).unwrap_err().code,
        ErrorCode::InvalidState
    );
    assert_eq!(state.phase, LanguageChoicePhase::Ready);
}
