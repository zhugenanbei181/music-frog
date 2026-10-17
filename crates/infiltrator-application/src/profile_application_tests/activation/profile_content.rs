//! Profile content behavior across Core lifecycle states.

use super::*;
use crate::profile_application::ProfileContentSaveOutcome;
use crate::profile_application::SelectedProfileApplyOutcome;
use lurker_core_contract::lifecycle::CoreInactiveLifecycle;
use lurker_core_contract::lifecycle::CoreLifecycle;

#[tokio::test]
async fn stopped_core_profile_content_is_saved_without_starting_or_applying() {
    let store = store_with_profiles("main", &[]);
    let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));
    runtime.set_lifecycle(CoreLifecycle::Stopped);
    let application = ProfileApplication::new(store.clone());
    let expected = store.active_reference();

    let outcome = application
        .save_profile_content_at_reference(
            ProfileRuntimeAccess::CoreOwner(runtime.clone()),
            "main".to_owned(),
            "mode: global\n".to_owned(),
            ApplyStrategy::AlwaysRestart,
            expected,
        )
        .await
        .expect("stopped Core accepts a fenced persisted edit");

    assert_eq!(
        outcome,
        ProfileContentSaveOutcome::StoredWhileInactive(CoreInactiveLifecycle::Stopped,)
    );
    assert_eq!(store.load("main").await.unwrap(), "mode: global\n");
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 1);
    assert_eq!(store.profile_save_count.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_core_profile_content_is_stored_with_a_failed_lifecycle_receipt() {
    let store = store_with_profiles("main", &[]);
    let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));
    runtime.set_lifecycle(CoreLifecycle::Failed);
    let application = ProfileApplication::new(store.clone());

    let outcome = application
        .save_profile_content_at_reference(
            ProfileRuntimeAccess::CoreOwner(runtime.clone()),
            "main".to_owned(),
            "mode: direct\n".to_owned(),
            ApplyStrategy::AlwaysRestart,
            store.active_reference(),
        )
        .await
        .expect("failed Core accepts only a fenced store for next start");

    assert_eq!(
        outcome,
        ProfileContentSaveOutcome::StoredWhileInactive(CoreInactiveLifecycle::Failed,)
    );
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 1);
    assert_eq!(store.profile_save_count.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn selected_profile_apply_reports_stopped_and_failed_separately() {
    for (lifecycle, expected) in [
        (CoreLifecycle::Stopped, CoreInactiveLifecycle::Stopped),
        (CoreLifecycle::Failed, CoreInactiveLifecycle::Failed),
    ] {
        let store = store_with_profiles("main", &[]);
        let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));
        runtime.set_lifecycle(lifecycle);
        let application = ProfileApplication::new(store.clone());
        let reference = store.active_reference();

        let outcome = application
            .apply_selected_profile_reference(
                runtime.clone(),
                &reference,
                ApplyStrategy::PreferReload,
            )
            .await
            .expect("inactive Core skips apply without collapsing the lifecycle state");

        assert_eq!(outcome, SelectedProfileApplyOutcome::NotActive(expected),);
        assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn transitioning_core_rejects_profile_content_before_store_or_runtime_effects() {
    let store = store_with_profiles("main", &[]);
    let runtime = Arc::new(ActivationRuntime::new(store.clone(), []));
    runtime.set_lifecycle(CoreLifecycle::Stopping);
    let application = ProfileApplication::new(store.clone());
    let expected = store.active_reference();

    let failure = application
        .save_profile_content_at_reference(
            ProfileRuntimeAccess::CoreOwner(runtime.clone()),
            "main".to_owned(),
            "mode: global\n".to_owned(),
            ApplyStrategy::AlwaysRestart,
            expected,
        )
        .await
        .expect_err("transitioning Core must fence the edit");

    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(store.load("main").await.unwrap(), "mode: rule\n");
    assert_eq!(store.profile_save_count.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
}
