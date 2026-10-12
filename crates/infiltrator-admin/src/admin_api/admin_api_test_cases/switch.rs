//! Behavior cases for switch.
//! test-intent: behavior

use super::*;
use crate::support::select_test_profile;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceState, ActiveProfileReferenceTarget,
};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_platform::paths::{clear_home_dir_override, set_home_dir_override};
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::profile_store::ProfileStore;
use mihomo_config::manager::ConfigManager;

#[tokio::test]
async fn test_switch_nonexistent_profile_returns_error() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());

    let app = setup_app();
    let payload = SwitchProfilePayload {
        name: "i-do-not-exist".to_string(),
    };

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/profiles/switch")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.status().is_client_error() || response.status().is_server_error());
    clear_home_dir_override();
}

#[tokio::test]
async fn profile_switch_submits_a_typed_owner_target_without_mutating_the_reference() {
    let _guard = TEST_LOCK.lock().await;
    let temp_dir = tempfile::tempdir().unwrap();
    set_home_dir_override(temp_dir.path().to_path_buf());

    let manager = ConfigManager::with_home_and_store(
        temp_dir.path().to_path_buf(),
        DefaultCredentialStore::default(),
    )
    .unwrap();
    ProfileStore::save_inactive_profile(manager.engine_profile_store(), "default", "mode: rule\n")
        .await
        .unwrap();
    ProfileStore::save_inactive_profile(manager.engine_profile_store(), "next", "mode: global\n")
        .await
        .unwrap();
    select_test_profile(&manager, "default").await;

    let (app, actions) = setup_app_with_owner_actions();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/admin/api/profiles/switch")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&SwitchProfilePayload {
                        name: "next".to_owned(),
                    })
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if !actions
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .is_empty()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("typed Core owner action is submitted");
    assert_eq!(
        *actions.lock().unwrap_or_else(|error| error.into_inner()),
        vec![CoreOwnerAction::SelectProfile(
            ActiveProfileReferenceTarget::selected(
                ConfigManager::engine_id(&manager),
                ProfileId::new("next").expect("valid profile ID"),
            ),
        )],
    );
    assert!(matches!(
        ActiveProfileReferenceStore::read(manager.engine_profile_store())
            .await
            .unwrap(),
        ActiveProfileReferenceState::Selected(reference)
            if reference.profile_id.as_str() == "default"
    ));
    clear_home_dir_override();
}
