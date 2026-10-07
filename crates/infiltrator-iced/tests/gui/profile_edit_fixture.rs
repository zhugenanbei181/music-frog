//! Source-bound read fixtures for focused TEA state tests.
//! test-intent: behavior
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::profile_edit::{ProfileDocumentReadReply, ProfileOptionsReadReply};
use infiltrator_contract::profile_document::ProfileDocumentSnapshot;
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_domain::profile_source::identify_profile_source;
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
    let mut snapshot =
        ProfileDocumentSnapshot::new(profile, &content, ProfileWriteProtection::Editable);
    snapshot.source = Some(identify_profile_source(profile.into(), &content, None));
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
        .profile
        .clone();
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
