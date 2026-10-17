//! Active-profile reference recovery and concurrency behavior.

use super::*;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileRecoveryOutcome, CoreApplyFailureKind,
};
use infiltrator_contract::error::FailureReason;
use tokio::sync::Barrier;

#[tokio::test]
async fn concurrent_switches_with_one_observed_revision_have_one_cas_winner() {
    let store = store_with_profiles("main", &["first", "second"]);
    store.set_reference_read_barrier(Arc::new(Barrier::new(2)));
    let application = ProfileApplication::new(store.clone());
    let runtime = Arc::new(ActivationRuntime::new(
        store.clone(),
        [ApplyBehavior::Succeed],
    ));
    let first_application = application.clone();
    let first_runtime = runtime.clone();
    let second_application = application;
    let second_runtime = runtime.clone();

    let first = tokio::spawn(async move {
        first_application
            .activate_profile(ProfileRuntimeAccess::CoreOwner(first_runtime), "first")
            .await
    });
    let second = tokio::spawn(async move {
        second_application
            .activate_profile(ProfileRuntimeAccess::CoreOwner(second_runtime), "second")
            .await
    });
    let outcomes = [
        first.await.expect("first task"),
        second.await.expect("second task"),
    ];

    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| result
                .as_ref()
                .is_err_and(|failure| failure.code == ErrorCode::NotReady))
            .count(),
        1
    );
    let active_reference = store.active_reference();
    let (selected, revision) = selected_profile(&active_reference);
    assert!(matches!(selected, "first" | "second"));
    assert_eq!(revision, 2);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
    let applied = runtime.references.lock().expect("runtime references");
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].profile_id.as_str(), selected);
    assert_eq!(applied[0].revision.value(), revision);
}

#[tokio::test]
async fn activation_applies_the_exact_committed_reference() {
    let store = store_with_profiles("main", &["next"]);
    let application = ProfileApplication::new(store.clone());
    let runtime = Arc::new(ActivationRuntime::new(
        store.clone(),
        [ApplyBehavior::Succeed],
    ));

    assert_eq!(
        application
            .activate_profile(ProfileRuntimeAccess::CoreOwner(runtime.clone()), "next")
            .await
            .expect("activation"),
        ProfileActivationOutcome::SelectedAndApplied
    );

    let active_reference = store.active_reference();
    let (selected, revision) = selected_profile(&active_reference);
    assert_eq!(selected, "next");
    assert_eq!(revision, 2);
    let applied = runtime.references.lock().expect("runtime references");
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].profile_id.as_str(), selected);
    assert_eq!(applied[0].revision.value(), revision);
}

#[tokio::test]
async fn failed_apply_conditionally_restores_and_reapplies_the_previous_reference() {
    let store = store_with_profiles("main", &["next"]);
    let application = ProfileApplication::new(store.clone());
    let runtime = Arc::new(ActivationRuntime::new(
        store.clone(),
        [ApplyBehavior::Fail, ApplyBehavior::Succeed],
    ));

    let failure = application
        .activate_profile(ProfileRuntimeAccess::CoreOwner(runtime.clone()), "next")
        .await
        .expect_err("failed apply remains an error");

    assert!(matches!(
        failure.reason,
        Some(FailureReason::ActiveProfileRecovery(
            ActiveProfileRecoveryOutcome::RestoredAndReapplied {
                restored_revision: 3,
                apply_failure: CoreApplyFailureKind::Adapter,
            }
        ))
    ));
    assert_eq!(selected_profile(&store.active_reference()), ("main", 3));
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 2);
    let applied = runtime.references.lock().expect("runtime references");
    assert_eq!(applied[0].profile_id.as_str(), "next");
    assert_eq!(applied[0].revision.value(), 2);
    assert_eq!(applied[1].profile_id.as_str(), "main");
    assert_eq!(applied[1].revision.value(), 3);
}

#[tokio::test]
async fn failed_conditional_recovery_preserves_a_later_selection_without_reapply() {
    let store = store_with_profiles("main", &["next", "rival"]);
    let application = ProfileApplication::new(store.clone());
    let runtime = Arc::new(ActivationRuntime::new(
        store.clone(),
        [ApplyBehavior::SwitchThenFail("rival".to_owned())],
    ));

    let failure = application
        .activate_profile(ProfileRuntimeAccess::CoreOwner(runtime.clone()), "next")
        .await
        .expect_err("target apply failed");

    assert!(matches!(
        failure.reason,
        Some(FailureReason::ActiveProfileRecovery(
            ActiveProfileRecoveryOutcome::Superseded {
                expected_revision: 2,
                observed_revision: 3,
                apply_failure: CoreApplyFailureKind::Adapter,
            }
        ))
    ));
    assert_eq!(selected_profile(&store.active_reference()), ("rival", 3));
    assert_eq!(store.reference_write_count.load(Ordering::SeqCst), 2);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        runtime.references.lock().expect("runtime references").len(),
        1
    );
}

#[tokio::test]
async fn runtime_activation_from_unselected_fails_before_selection_or_dispatch() {
    let store = Arc::new(FakeStore::with_profile("next", "mode: rule\n", false));
    let application = ProfileApplication::new(store.clone());
    let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));

    let failure = application
        .activate_profile(ProfileRuntimeAccess::CoreOwner(runtime.clone()), "next")
        .await
        .expect_err("no previous reference cannot be recovered safely");

    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert!(matches!(
        store.active_reference(),
        ActiveProfileReferenceState::Unselected { .. }
    ));
    assert_eq!(store.reference_write_count.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
}
