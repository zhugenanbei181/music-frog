//! Coherent editor document/sidecar observations for native matrix scenarios.
use super::snapshot_diff_page_projection;
use super::snapshot_history_fixture;
use infiltrator_bevy_ui::pages::profiles::ProfilesProjection;
use infiltrator_contract::profile_document::ProfileDocumentSnapshot;
use infiltrator_contract::profile_editor_read::{ProfileEditorReadSnapshot, ProfileReadStatus};
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_domain::profile_source::identify_profile_source;

pub(super) fn editor_page_projection(
    protection: ProfileWriteProtection,
    content: &str,
) -> ProfilesProjection {
    let mut projection = snapshot_diff_page_projection(None);
    let mut document = ProfileDocumentSnapshot::new("main", content, protection);
    document.source = Some(identify_profile_source("main".into(), content, None));
    projection.editor_read = ProfileEditorReadSnapshot {
        profile: Some("main".into()),
        observed_source: document.source.clone(),
        verified_source: document.source.clone(),
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
            options.source.document_hash,
            identify_profile_source("main".into(), content, None).document_hash,
            "native editor facts must describe the supplied document bytes"
        );
        projection.editor_read.observed_source = Some(options.source.clone());
        projection.editor_read.verified_source = Some(options.source.clone());
        projection.editor_read.options = ProfileReadStatus::Ready;
        if let Some(document) = &mut projection.profile_document {
            document.source = Some(options.source.clone());
        }
    }
    projection.profile_options = options;
    projection
}
