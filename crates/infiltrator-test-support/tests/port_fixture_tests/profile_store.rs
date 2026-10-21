//! Behaviour of the isolated profile-store port fixture.

use std::slice;
use infiltrator_contract::aggregator::{AggregationDraft, AggregationTemplate};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use infiltrator_test_support::profile_store_fixtures::{
    CAPTURE_LARGE_PROFILE, CAPTURE_PROFILE, LARGE_PROFILE_RULE_COUNT, ProfileStoreCapture,
};
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_ports::error::PortError;

fn profile_id(name: &str) -> ProfileId {
    ProfileId::new(name).expect("valid capture profile ID")
}

#[tokio::test]
async fn the_store_publishes_the_small_and_the_large_profile() {
    let port = ProfileStoreCapture::new();
    let profiles = port.list_profiles().await.expect("profile list");

    assert_eq!(profiles.len(), 2);
    assert!(
        profiles
            .iter()
            .any(|profile| profile.name == CAPTURE_PROFILE && profile.active)
    );
    assert!(
        profiles
            .iter()
            .any(|profile| profile.name == CAPTURE_LARGE_PROFILE && !profile.active)
    );
}

#[tokio::test]
async fn the_large_profile_really_carries_more_lines_than_one_viewport() {
    let port = ProfileStoreCapture::new();
    let document = port.load(CAPTURE_LARGE_PROFILE).await.expect("document");
    let lines = document.lines().count();

    assert!(
        lines > LARGE_PROFILE_RULE_COUNT,
        "the large document must carry every generated rule"
    );
    assert!(
        lines > 1_000,
        "a bounded editor window needs a document far beyond one viewport"
    );
}

#[tokio::test]
async fn an_unknown_profile_is_not_found() {
    let port = ProfileStoreCapture::new();

    let error = port
        .load("never-stored-profile")
        .await
        .expect_err("an unknown profile must not be found");

    assert!(matches!(error, PortError::NotFound(_)));
}

#[tokio::test]
async fn the_active_profile_cannot_be_deleted() {
    let port = ProfileStoreCapture::new();

    let error = port
        .delete_profile(CAPTURE_PROFILE)
        .await
        .expect_err("the active profile must be protected");

    assert!(matches!(error, PortError::Rejected(_)));
    assert!(port.profile_names().contains(&CAPTURE_PROFILE.to_owned()));
}

#[tokio::test]
async fn an_inactive_profile_can_be_deleted() {
    let port = ProfileStoreCapture::new();
    port.save_inactive_profile("spare-profile", "proxies: []\n")
        .await
        .expect("save");

    port.delete_profile("spare-profile").await.expect("delete");

    assert!(!port.profile_names().contains(&"spare-profile".to_owned()));
}

#[tokio::test]
async fn a_stale_workspace_identity_is_refused_instead_of_overwriting() {
    let port = ProfileStoreCapture::new();
    let observed = port
        .load_workspace(&profile_id(CAPTURE_LARGE_PROFILE))
        .await
        .expect("workspace");

    port.compare_and_save_inactive_workspace(
        &observed.identity(),
        &ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: b"proxies: []\n".to_vec(),
            sidecar_bytes: None,
        },
    )
    .await
    .expect("first write");

    let error = port
        .compare_and_save_inactive_workspace(
            &observed.identity(),
            &ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: b"proxies: []\n".to_vec(),
                sidecar_bytes: None,
            },
        )
        .await
        .expect_err("the stale write must fail");

    assert!(matches!(error, PortError::Rejected(_)));
    let current = port.load(CAPTURE_LARGE_PROFILE).await.expect("document");
    assert_eq!(current, "proxies: []\n");
}

#[tokio::test]
async fn the_aggregation_template_library_round_trips() {
    let port = ProfileStoreCapture::new();
    assert!(port.template_library().is_empty());

    let template = AggregationTemplate {
        name: "regional-capture".to_owned(),
        draft: AggregationDraft {
            source_profiles: vec![CAPTURE_PROFILE.to_owned()],
            target_name: "merged-capture".to_owned(),
            deduplicate: true,
            ..AggregationDraft::default()
        },
        updated_at: "2026-10-20T00:00:00Z".to_owned(),
    };
    port.save_aggregation_templates(slice::from_ref(&template))
        .await
        .expect("save templates");

    let loaded = port
        .load_aggregation_templates()
        .await
        .expect("load templates");
    assert_eq!(loaded, vec![template]);
}

#[tokio::test]
async fn the_active_workspace_follows_the_active_reference() {
    let port = ProfileStoreCapture::new();
    let workspace = port.load_active_workspace().await.expect("workspace");

    assert_eq!(
        workspace.document.identity.profile_id,
        profile_id(CAPTURE_PROFILE)
    );
}
