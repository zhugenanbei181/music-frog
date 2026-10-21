//! The sync page resolves a key-level conflict in its merge session.
//!
//! The merge session is surface state the user edits directly: picking the
//! remote value for one key is a local decision the panel renders, so the
//! demo performs it without touching the host's configuration directory.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;

/// The conflicting key the demo resolves: the merge session's first key.
///
/// The demo resolves the first key of the session, so the merge panel's first
/// key row is the control the capture measures.
pub(super) fn demo_conflict_key(state: &AppState) -> Option<String> {
    state.profile.sync_diff.as_ref().and_then(|diff| {
        diff.bundle
            .key_entries()
            .first()
            .map(|(key, _)| key.clone())
    })
}

pub(super) fn activate(state: &mut AppState) -> Task<Message> {
    // The user picks the remote value for one conflicting key: the merge
    // session records the pick and the panel renders the decision.
    let Some(key) = demo_conflict_key(state) else {
        return Task::none();
    };
    state.update(Message::PickSyncDiffKey(key, true))
}
