//! Source-bound read fixtures for focused TEA state tests.
//! test-intent: behavior
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::profile_edit::{ProfileDocumentReadReply, ProfileOptionsReadReply};
use infiltrator_contract::profile_document::{DocumentInspection, ProfileDocumentSnapshot};
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_test_support::subscription_filter_fixture;
use lurker_core_contract::profile_document::DocumentValidation;
use std::path::PathBuf;

pub fn document(state: &mut AppState, path: PathBuf, content: String) -> Message {
    let _ = state.update(Message::EditProfile(path.clone()));
    let ticket = state
        .editor
        .document_load
        .as_ref()
        .expect("read prepared")
        .0;
    let profile = path.file_stem().unwrap().to_str().unwrap();
    let workspace_identity =
        subscription_filter_fixture::observation(profile, &content, Default::default())
            .expect("fixture identity")
            .workspace_identity;
    let snapshot = ProfileDocumentSnapshot::new(
        workspace_identity,
        &content,
        ProfileWriteProtection::Editable,
    )
    .with_inspection(DocumentInspection::Inspected(DocumentValidation::Valid));
    Message::ProfileContentLoaded(ProfileDocumentReadReply {
        ticket,
        path,
        result: Ok(snapshot),
    })
}
pub fn options(state: &mut AppState, content: String) -> Message {
    let profile = state
        .editor
        .document_session
        .source()
        .unwrap()
        .document
        .profile_id
        .as_str()
        .to_owned();
    state.editor.next_editor_read += 1;
    let ticket = state.editor.next_editor_read;
    state.editor.mixin_load = Some((ticket, profile.clone()));
    let source = state.editor.document_session.source().unwrap().clone();
    Message::MixinLoaded(ProfileOptionsReadReply {
        ticket,
        profile,
        result: Ok(ProfileOptionsSnapshot::new(
            source,
            content,
            Default::default(),
        )),
    })
}
