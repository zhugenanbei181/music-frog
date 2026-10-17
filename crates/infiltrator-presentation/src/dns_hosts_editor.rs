//! Hosts draft reducer with explicit observation, editing, and save lifecycles.
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_hosts::{
    DnsHostEntry, DnsHostsIssue, DnsHostsProfile, validate_hosts,
};
use infiltrator_domain::dns_hosts::{hosts_entries_from_map, hosts_map_from_entries};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::{PageData, PageStatus};
use std::collections::HashSet;
use std::mem;
use std::slice::from_ref;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostDraftRow {
    pub id: u64,
    pub entry: DnsHostEntry,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingHosts {
    pub token: u64,
    pub patch: DnsSettingsPatch,
    pub entries: Vec<DnsHostEntry>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HostsSession {
    #[default]
    Closed,
    Open,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum HostsObservation {
    #[default]
    Unobserved,
    Loading,
    Ready,
    Failed(Failure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostsDraftOrigin {
    UserEdits,
    LegacyMigration,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HostsDraftPhase {
    #[default]
    Clean,
    Modified(HostsDraftOrigin),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum HostsRowDraft {
    #[default]
    Empty,
    New {
        address: String,
        domain: String,
    },
    Editing {
        id: u64,
        address: String,
        domain: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum HostsEditorFeedback {
    #[default]
    Clear,
    Failure(Failure),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum HostsSavePhase {
    #[default]
    Idle,
    Submitting(PendingHosts),
    AwaitingReadback {
        entries: Vec<DnsHostEntry>,
    },
    Failed(Failure),
}

#[derive(Clone, Debug, Default)]
pub struct DnsHostsEditor {
    session: HostsSession,
    observation: HostsObservation,
    draft: HostsDraftPhase,
    row_draft: HostsRowDraft,
    feedback: HostsEditorFeedback,
    save: HostsSavePhase,
    pub rows: Vec<HostDraftRow>,
    pub applied: Option<DnsHostsProfile>,
    pub issues: Vec<DnsHostsIssue>,
    latest: Option<DnsHostsProfile>,
    next_row: u64,
    next_token: u64,
}

impl DnsHostsEditor {
    pub fn is_open(&self) -> bool {
        matches!(&self.session, HostsSession::Open)
    }

    pub fn is_ready(&self) -> bool {
        matches!(&self.observation, HostsObservation::Ready)
    }

    pub fn is_dirty(&self) -> bool {
        matches!(&self.draft, HostsDraftPhase::Modified(_))
    }

    pub fn is_importing_legacy(&self) -> bool {
        matches!(
            &self.draft,
            HostsDraftPhase::Modified(HostsDraftOrigin::LegacyMigration)
        )
    }

    pub fn is_editing(&self) -> bool {
        matches!(&self.row_draft, HostsRowDraft::Editing { .. })
    }

    pub fn is_pending(&self) -> bool {
        matches!(&self.save, HostsSavePhase::Submitting(_))
    }

    pub fn is_awaiting_readback(&self) -> bool {
        matches!(&self.save, HostsSavePhase::AwaitingReadback { .. })
    }

    pub fn observation(&self) -> &HostsObservation {
        &self.observation
    }

    pub fn row_draft(&self) -> &HostsRowDraft {
        &self.row_draft
    }

    pub fn address(&self) -> &str {
        match &self.row_draft {
            HostsRowDraft::Empty => "",
            HostsRowDraft::New { address, .. } | HostsRowDraft::Editing { address, .. } => address,
        }
    }

    pub fn domain(&self) -> &str {
        match &self.row_draft {
            HostsRowDraft::Empty => "",
            HostsRowDraft::New { domain, .. } | HostsRowDraft::Editing { domain, .. } => domain,
        }
    }

    pub fn read_failure(&self) -> Option<&Failure> {
        match &self.observation {
            HostsObservation::Failed(failure) => Some(failure),
            _ => None,
        }
    }

    pub fn failure(&self) -> Option<&Failure> {
        match &self.save {
            HostsSavePhase::Failed(failure) => Some(failure),
            _ => match &self.feedback {
                HostsEditorFeedback::Failure(failure) => Some(failure),
                HostsEditorFeedback::Clear => None,
            },
        }
    }

    pub fn pending(&self) -> Option<&PendingHosts> {
        match &self.save {
            HostsSavePhase::Submitting(pending) => Some(pending),
            _ => None,
        }
    }

    fn seed(&mut self, entries: &[DnsHostEntry]) {
        let mut claimed = HashSet::new();
        let mut wanted = Vec::new();
        for entry in entries {
            let id = if let Some(row) = self
                .rows
                .iter()
                .find(|row| &row.entry == entry && !claimed.contains(&row.id))
            {
                row.id
            } else {
                let Some(next) = self.next_row.checked_add(1) else {
                    self.observation = HostsObservation::Failed(Failure::new(
                        ErrorCode::InvalidState,
                        "Hosts row identity space exhausted",
                        false,
                    ));
                    return;
                };
                let id = self.next_row;
                self.next_row = next;
                id
            };
            claimed.insert(id);
            wanted.push(HostDraftRow {
                id,
                entry: entry.clone(),
            });
        }
        self.rows = wanted;
    }

    pub fn observe(&mut self, snapshot: &PageData<DnsHostsProfile>) {
        self.observation = HostsObservation::Loading;
        match &snapshot.status {
            PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
                self.observation = HostsObservation::Failed(failure.clone());
                return;
            }
            PageStatus::Loading => return,
            PageStatus::Ready | PageStatus::Empty => {}
        }
        let Some(profile) = snapshot
            .data
            .as_ref()
            .filter(|profile| !profile.profile.is_empty())
        else {
            self.observation = HostsObservation::Failed(Failure::new(
                ErrorCode::InvalidState,
                "Hosts observation has no profile identity",
                false,
            ));
            return;
        };
        if let HostsSavePhase::AwaitingReadback { entries } = &self.save
            && entries != &profile.entries
            && self
                .applied
                .as_ref()
                .is_some_and(|applied| applied.profile == profile.profile)
        {
            self.observation = HostsObservation::Ready;
            return;
        }
        self.latest = Some(profile.clone());
        if self.is_pending() {
            if self
                .applied
                .as_ref()
                .is_some_and(|applied| applied.profile != profile.profile)
            {
                self.observation = HostsObservation::Failed(Failure::new(
                    ErrorCode::InvalidState,
                    "active profile changed during Hosts save",
                    false,
                ));
            } else {
                self.observation = HostsObservation::Ready;
            }
            return;
        }

        if (self.is_dirty() || !self.address().is_empty() || !self.domain().is_empty())
            && self.applied.as_ref().is_some_and(|applied| {
                applied.profile != profile.profile
                    || !matches!(&self.save, HostsSavePhase::AwaitingReadback { .. })
                        && applied.entries != profile.entries
            })
        {
            self.observation = HostsObservation::Failed(Failure::new(
                ErrorCode::InvalidState,
                "configuration changed; cancel this draft to review the latest mappings",
                false,
            ));
            return;
        }
        self.observation = HostsObservation::Ready;
        if let HostsSavePhase::AwaitingReadback { entries } = &self.save {
            if entries != &profile.entries {
                return;
            }
            self.save = HostsSavePhase::Idle;
        }
        self.applied = Some(profile.clone());
        if !self.is_dirty() && !self.is_pending() {
            self.seed(&profile.entries);
        }
    }

    pub fn show(&mut self) {
        self.session = HostsSession::Open;
    }

    fn update_row_draft(&mut self, address: Option<String>, domain: Option<String>) {
        self.row_draft = match mem::take(&mut self.row_draft) {
            HostsRowDraft::Empty => HostsRowDraft::New {
                address: address.unwrap_or_default(),
                domain: domain.unwrap_or_default(),
            },
            HostsRowDraft::New {
                address: current_address,
                domain: current_domain,
            } => HostsRowDraft::New {
                address: address.unwrap_or(current_address),
                domain: domain.unwrap_or(current_domain),
            },
            HostsRowDraft::Editing {
                id,
                address: current_address,
                domain: current_domain,
            } => HostsRowDraft::Editing {
                id,
                address: address.unwrap_or(current_address),
                domain: domain.unwrap_or(current_domain),
            },
        };
    }

    pub fn edit_address(&mut self, value: String) {
        if !self.is_pending() {
            self.update_row_draft(Some(value), None);
            self.issues.clear();
        }
    }

    pub fn edit_domain(&mut self, value: String) {
        if !self.is_pending() {
            self.update_row_draft(None, Some(value));
            self.issues.clear();
        }
    }

    pub fn select(&mut self, id: u64) {
        if self.is_pending() {
            return;
        }
        if !matches!(&self.row_draft, HostsRowDraft::Empty) {
            self.feedback = HostsEditorFeedback::Failure(Failure::new(
                ErrorCode::InvalidState,
                "confirm or discard the current row input before editing another mapping",
                false,
            ));
            return;
        }
        if let Some(row) = self.rows.iter().find(|row| row.id == id) {
            self.row_draft = HostsRowDraft::Editing {
                id,
                address: row.entry.address.clone(),
                domain: row.entry.domain.clone(),
            };
            self.issues.clear();
            self.feedback = HostsEditorFeedback::Clear;
        }
    }

    pub fn cancel_row(&mut self) {
        if self.is_pending() {
            return;
        }
        self.row_draft = HostsRowDraft::Empty;
        self.issues.clear();
        self.feedback = HostsEditorFeedback::Clear;
    }

    fn mark_modified(&mut self, origin: HostsDraftOrigin) {
        let current = match self.draft {
            HostsDraftPhase::Modified(HostsDraftOrigin::LegacyMigration) => {
                HostsDraftOrigin::LegacyMigration
            }
            _ => origin,
        };
        self.draft = HostsDraftPhase::Modified(current);
    }

    pub fn commit_row(&mut self) -> bool {
        if self.is_pending() {
            return false;
        }
        let entry = DnsHostEntry {
            address: self.address().trim().into(),
            domain: self.domain().trim().into(),
        };
        self.issues = validate_hosts(from_ref(&entry));
        if !self.issues.is_empty() {
            return false;
        }
        match &self.row_draft {
            HostsRowDraft::Editing { id, .. } => {
                let Some(row) = self.rows.iter_mut().find(|row| row.id == *id) else {
                    return false;
                };
                row.entry = entry;
            }
            HostsRowDraft::Empty | HostsRowDraft::New { .. } => {
                if !self.rows.iter().any(|row| row.entry == entry) {
                    let id = self.next_row;
                    let Some(next) = self.next_row.checked_add(1) else {
                        self.feedback = HostsEditorFeedback::Failure(Failure::new(
                            ErrorCode::InvalidState,
                            "Hosts row identity space exhausted",
                            false,
                        ));
                        return false;
                    };
                    self.next_row = next;
                    self.rows.push(HostDraftRow { id, entry });
                }
            }
        }
        self.row_draft = HostsRowDraft::Empty;
        self.mark_modified(HostsDraftOrigin::UserEdits);
        self.feedback = HostsEditorFeedback::Clear;
        true
    }

    pub fn remove(&mut self, id: u64) {
        if self.is_pending() {
            return;
        }
        let count = self.rows.len();
        self.rows.retain(|row| row.id != id);
        if self.rows.len() != count {
            self.mark_modified(HostsDraftOrigin::UserEdits);
            if matches!(&self.row_draft, HostsRowDraft::Editing { id: selected, .. } if *selected == id)
            {
                self.row_draft = HostsRowDraft::Empty;
            }
        }
    }

    pub fn import_legacy(&mut self) {
        if self.is_pending()
            || self.is_importing_legacy()
            || !matches!(&self.row_draft, HostsRowDraft::Empty)
        {
            return;
        }
        let Some(profile) = self.applied.clone() else {
            return;
        };
        self.mark_modified(HostsDraftOrigin::LegacyMigration);
        for entry in profile.legacy_entries {
            if !self.rows.iter().any(|row| row.entry == entry) {
                self.row_draft = HostsRowDraft::New {
                    address: entry.address,
                    domain: entry.domain,
                };
                if !self.commit_row() {
                    return;
                }
            }
        }
        self.mark_modified(HostsDraftOrigin::LegacyMigration);
    }

    pub fn cancel(&mut self) -> bool {
        if self.is_pending() {
            return false;
        }
        self.session = HostsSession::Closed;
        self.row_draft = HostsRowDraft::Empty;
        self.draft = HostsDraftPhase::Clean;
        self.feedback = HostsEditorFeedback::Clear;
        self.save = HostsSavePhase::Idle;
        self.issues.clear();
        if let Some(profile) = self.latest.clone() {
            self.seed(&profile.entries);
            self.applied = Some(profile);
        }
        true
    }

    pub fn can_apply(&self) -> bool {
        self.is_open()
            && self.is_ready()
            && self.is_dirty()
            && !self.is_pending()
            && self.address().is_empty()
            && self.domain().is_empty()
            && validate_hosts(
                &self
                    .rows
                    .iter()
                    .map(|row| row.entry.clone())
                    .collect::<Vec<_>>(),
            )
            .is_empty()
    }

    pub fn begin(&mut self) -> Result<PendingHosts, Failure> {
        if !self.can_apply() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Hosts draft cannot be applied",
                false,
            ));
        }
        let profile = self.applied.as_ref().ok_or_else(|| {
            Failure::new(ErrorCode::NotReady, "Hosts profile is unavailable", true)
        })?;
        let draft: Vec<_> = self.rows.iter().map(|row| row.entry.clone()).collect();
        let entries = hosts_map_from_entries(&draft)
            .and_then(|map| hosts_entries_from_map(&map))
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        self.next_token = self.next_token.checked_add(1).ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "Hosts request identity space exhausted",
                false,
            )
        })?;
        let patch = DnsSettingsPatch {
            expected_profile: Some(profile.profile.clone()),
            expected_hosts: Some(profile.entries.clone()),
            hosts: (!entries.is_empty()).then_some(entries.clone()),
            clear_hosts: entries.is_empty(),
            remove_legacy_hosts: self.is_importing_legacy(),
            ..Default::default()
        };
        let pending = PendingHosts {
            token: self.next_token,
            patch,
            entries,
        };
        self.save = HostsSavePhase::Submitting(pending.clone());
        self.feedback = HostsEditorFeedback::Clear;
        Ok(pending)
    }

    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> bool {
        let HostsSavePhase::Submitting(pending) = &self.save else {
            return false;
        };
        if pending.token != token {
            return false;
        }
        let pending = pending.clone();
        match result {
            Ok(()) => {
                if let Some(profile) = &mut self.applied {
                    profile.entries.clone_from(&pending.entries);
                    if pending.patch.remove_legacy_hosts {
                        profile.legacy_entries.clear();
                    }
                }
                if self.latest.as_ref().is_some_and(|latest| {
                    self.applied
                        .as_ref()
                        .is_some_and(|applied| latest.profile == applied.profile)
                }) {
                    self.latest.clone_from(&self.applied);
                }
                self.seed(&pending.entries);
                self.save = HostsSavePhase::AwaitingReadback {
                    entries: pending.entries,
                };
                self.draft = HostsDraftPhase::Clean;
                self.feedback = HostsEditorFeedback::Clear;
            }
            Err(failure) => self.save = HostsSavePhase::Failed(failure),
        }
        true
    }
}

#[cfg(test)]
#[path = "dns_hosts_editor_tests.rs"]
mod tests;
