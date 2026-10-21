//! The one typed page model of the sync page, shared by every surface.
//!
//! The shared [`crate::sync_projection`] already owns the copy of the sync
//! observation, but only Bevy consumed it, and every surface kept its own
//! phase state for the same two operations: synchronising now and resolving
//! the current conflict.
//!
//! This module owns the page state around that projection. The observation
//! stays the contract's truth, the conflict resolution is one typed value
//! instead of a set of per-surface buttons, and the two operations are typed
//! phase machines with their own tokens.

use crate::operation_phase::OperationPhase;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_contract::sync_snapshot::{
    SnapshotItemSnapshot, SyncConflictSnapshot, SyncPageSnapshot,
};
use lurker_core_contract::page::PageStatus;

/// How the user resolves the current conflict.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConflictResolution {
    /// Keep the local configuration.
    #[default]
    KeepLocal,
    /// Take the remote configuration.
    TakeRemote,
    /// Merge both configurations.
    MergeBoth,
}

/// One conflicting field of the current conflict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncConflictField {
    /// The configuration key that conflicts.
    pub key: String,
    /// The local value.
    pub local_value: String,
    /// The remote value.
    pub remote_value: String,
}

/// The one page model of the sync page.
///
/// A surface renders the sync page from this model alone: the status, the
/// conflict, the conflict fields and the operation phases come from here, and
/// the surface only dispatches typed intents back.
#[derive(Clone, Debug, Default)]
pub struct SyncPageModel {
    page: Option<SyncPageSnapshot>,
    resolution: ConflictResolution,
    sync_now: OperationPhase,
    resolve_conflict: OperationPhase,
}

impl SyncPageModel {
    /// Fold the owner's sync page snapshot into the model.
    pub fn observe_page(&mut self, page: SyncPageSnapshot) {
        self.page = Some(page);
    }

    /// The sync status the owner reported.
    ///
    /// A page that has not been observed reports `Unknown`: the surface never
    /// shows "disconnected" for a page it has not read.
    pub fn status(&self) -> SyncStatus {
        self.page
            .as_ref()
            .map_or(SyncStatus::Unknown, |page| page.status)
    }

    /// The server the owner reported.
    pub fn server_url(&self) -> &str {
        self.page
            .as_ref()
            .map_or("", |page| page.server_url.as_str())
    }

    /// The user name the owner reported.
    pub fn username(&self) -> &str {
        self.page.as_ref().map_or("", |page| page.username.as_str())
    }

    /// The last successful synchronisation the owner reported.
    pub fn last_sync(&self) -> Option<&str> {
        self.page
            .as_ref()
            .and_then(|page| page.last_sync.as_deref())
    }

    /// Whether the owner reports a synchronisation in flight.
    ///
    /// The answer comes from the owner, not from a local flag: a surface
    /// never shows "synchronising" for a page the owner did not say so.
    pub fn is_syncing(&self) -> bool {
        self.status() == SyncStatus::Syncing
    }

    /// Whether the owner synchronises automatically.
    pub fn auto_sync(&self) -> Option<bool> {
        self.page.as_ref().map(|page| page.auto_sync)
    }

    /// The conflict the owner reported.
    pub fn conflict(&self) -> Option<&SyncConflictSnapshot> {
        self.page.as_ref().and_then(|page| page.conflict.as_ref())
    }

    /// The conflicting fields of the current conflict.
    pub fn conflict_fields(&self) -> Vec<SyncConflictField> {
        self.conflict()
            .map(|conflict| {
                conflict
                    .conflicting_keys
                    .iter()
                    .map(|(key, local_value, remote_value)| SyncConflictField {
                        key: String::clone(key),
                        local_value: String::clone(local_value),
                        remote_value: String::clone(remote_value),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The status of the snapshot history the owner published.
    pub fn history_status(&self) -> PageStatus {
        self.page
            .as_ref()
            .map_or(PageStatus::Loading, |page| page.history_status.clone())
    }

    /// The snapshots the owner published.
    pub fn snapshots(&self) -> &[SnapshotItemSnapshot] {
        match &self.page {
            Some(page) => &page.snapshots,
            None => &[],
        }
    }

    /// How the user resolves the current conflict.
    pub fn resolution(&self) -> ConflictResolution {
        self.resolution
    }

    /// Select how the user resolves the current conflict.
    pub fn select_resolution(&mut self, resolution: ConflictResolution) {
        self.resolution = resolution;
    }

    /// The owner intent that resolves the current conflict.
    ///
    /// The owner vocabulary has a keep-local and a take-remote command; a
    /// merge of both configurations is the owner's own merge step, so the
    /// merge selection resolves to the keep-local intent and the surface
    /// still shows what the user chose.
    pub fn resolve_conflict_intent(&self) -> Option<CommandIntent> {
        self.conflict()?;
        Some(match self.resolution {
            ConflictResolution::KeepLocal | ConflictResolution::MergeBoth => {
                CommandIntent::ResolveConflictKeepLocal
            }
            ConflictResolution::TakeRemote => CommandIntent::ResolveConflictTakeRemote,
        })
    }

    /// The owner intent that asks the host to synchronise now.
    pub fn sync_now_intent(&self) -> CommandIntent {
        CommandIntent::SyncNow
    }

    /// The phase of the submitted synchronisation.
    pub fn sync_now_phase(&self) -> &OperationPhase {
        &self.sync_now
    }

    /// The phase of the submitted synchronisation, for the fold that changes it.
    pub fn sync_now_phase_mut(&mut self) -> &mut OperationPhase {
        &mut self.sync_now
    }

    /// The phase of the submitted conflict resolution.
    pub fn resolve_phase(&self) -> &OperationPhase {
        &self.resolve_conflict
    }

    /// The phase of the submitted conflict resolution, for the fold that changes it.
    pub fn resolve_phase_mut(&mut self) -> &mut OperationPhase {
        &mut self.resolve_conflict
    }
}

#[cfg(test)]
#[path = "sync_page_model_tests.rs"]
mod tests;
