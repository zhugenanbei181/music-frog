//! Coherent editor document/sidecar observations for native matrix scenarios.
use super::snapshot_diff_page_projection;
use super::snapshot_history_fixture;
use infiltrator_bevy_ui::pages::profiles::ProfilesProjection;
use infiltrator_contract::profile_document::{
    DocumentInspection, DocumentValidation, ProfileDocumentSnapshot,
};
use infiltrator_contract::profile_editor_read::{ProfileEditorReadSnapshot, ProfileReadStatus};
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_test_support::subscription_filter_fixture;

pub(super) fn editor_page_projection(
    protection: ProfileWriteProtection,
    content: &str,
) -> ProfilesProjection {
    let mut projection = snapshot_diff_page_projection(None);
    let workspace_identity =
        subscription_filter_fixture::observation("main", content, Default::default())
            .unwrap()
            .workspace_identity;
    let document = ProfileDocumentSnapshot::new(workspace_identity.clone(), content, protection)
        .with_inspection(DocumentInspection::Inspected(DocumentValidation::Valid));
    projection.editor_read = ProfileEditorReadSnapshot {
        profile: Some("main".into()),
        observed_workspace_identity: Some(workspace_identity.clone()),
        verified_workspace_identity: Some(workspace_identity),
        document: ProfileReadStatus::Ready,
        ..Default::default()
    };
    projection.profile_document = Some(document);
    projection.snapshot_history = Some(snapshot_history_fixture());
    projection
}

pub(super) fn editor_options_page_projection(
    content: &str,
    options: Option<ProfileOptionsSnapshot>,
) -> ProfilesProjection {
    let mut projection = editor_page_projection(ProfileWriteProtection::Editable, content);
    if let Some(options) = &options {
        assert_eq!(
            options.workspace_identity.document.revision,
            subscription_filter_fixture::observation("main", content, Default::default(),)
                .unwrap()
                .workspace_identity
                .document
                .revision,
            "native editor facts must describe the supplied document bytes"
        );
        projection.editor_read.observed_workspace_identity =
            Some(options.workspace_identity.clone());
        projection.editor_read.verified_workspace_identity =
            Some(options.workspace_identity.clone());
        projection.editor_read.options = ProfileReadStatus::Ready;
        if let Some(document) = &mut projection.profile_document {
            document.workspace_identity = options.workspace_identity.clone();
        }
    }
    projection.profile_options = options;
    projection
}
