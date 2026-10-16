//! Selection and activation lifecycle behavior.

use crate::profile_application::ProfileRuntimeAccess;
use infiltrator_contract::snapshot::CoreInactiveLifecycle;
use infiltrator_contract::snapshot::CoreLifecycle;
use super::*;
use infiltrator_contract::error::FailureReason;

#[tokio::test]
async fn activation_stale_revision_does_not_dispatch_or_overwrite_the_winner() {
    let store = store_with_profiles("main", &["next", "rival"]);
    store.change_reference_before_next_compare(target("rival"));
    let application = ProfileApplication::new(store.clone());

    let failure = application
        .activate_profile::<ActivationRuntime>(
            ProfileRuntimeAccess::Offline,
            "next",
        )
        .await
        .expect_err("stale selection must be rejected");

    assert_eq!(failure.code, ErrorCode::NotReady);
    assert!(matches!(
        failure.reason,
        Some(FailureReason::ActiveProfileReferenceStale {
            expected_revision: 1,
            observed_revision: 2
        })
    ));
    assert_eq!(selected_profile(&store.active_reference()), ("rival", 2));
    assert_eq!(store.reference_write_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn offline_selection_is_rejected_while_a_core_owner_is_bound() {
    let store = store_with_profiles("main", &["next"]);
    let runtime = ActivationRuntime::new(store.clone(), []);
    let application = ProfileApplication::new(store.clone());

    let failure = application
        .select_profile("next")
        .await
        .expect_err("offline selection must not desynchronize a live owner");

    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(selected_profile(&store.active_reference()), ("main", 1));
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn managed_activation_updates_selection_without_starting_a_stopped_core() {
    let store = store_with_profiles("main", &["next"]);
    let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));
    runtime.set_lifecycle(CoreLifecycle::Stopped);
    let application = ProfileApplication::new(store.clone());

    let outcome = application
        .activate_profile(ProfileRuntimeAccess::CoreOwner(runtime.clone()), "next")
        .await
        .expect("stopped Core can select a profile without applying it");

    assert_eq!(
        outcome,
        ProfileActivationOutcome::SelectedWhileInactive(
            CoreInactiveLifecycle::Stopped,
        )
    );
    assert_eq!(selected_profile(&store.active_reference()), ("next", 2));
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn managed_activation_while_core_failed_keeps_the_failed_state() {
    let store = store_with_profiles("main", &["next"]);
    let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));
    runtime.set_lifecycle(CoreLifecycle::Failed);
    let application = ProfileApplication::new(store.clone());

    let outcome = application
        .activate_profile(ProfileRuntimeAccess::CoreOwner(runtime.clone()), "next")
        .await
        .expect("failed Core can persist the selected reference for next start");

    assert_eq!(
        outcome,
        ProfileActivationOutcome::SelectedWhileInactive(
            CoreInactiveLifecycle::Failed,
        )
    );
    assert_eq!(selected_profile(&store.active_reference()), ("next", 2));
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn transitioning_core_rejects_activation_before_reference_or_apply_effects() {
    let store = store_with_profiles("main", &["next"]);
    let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));
    runtime.set_lifecycle(CoreLifecycle::Starting);
    let application = ProfileApplication::new(store.clone());

    let failure = application
        .activate_profile(ProfileRuntimeAccess::CoreOwner(runtime.clone()), "next")
        .await
        .expect_err("transitioning lifecycle must fence selection");

    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(selected_profile(&store.active_reference()), ("main", 1));
    assert_eq!(store.reference_write_count.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
}
