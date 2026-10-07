//! Neutral Hosts draft: stable rows, explicit migration and correlated durable completion.
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_hosts::{
    DnsHostEntry, DnsHostsIssue, DnsHostsProfile, validate_hosts,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::{PageData, PageStatus};
use infiltrator_domain::dns_hosts::{hosts_entries_from_map, hosts_map_from_entries};
use std::collections::HashSet;
use std::slice::from_ref;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostDraftRow {
    pub id: u64,
    pub entry: DnsHostEntry,
}
#[derive(Clone, Debug)]
pub struct PendingHosts {
    pub token: u64,
    pub patch: DnsSettingsPatch,
    pub entries: Vec<DnsHostEntry>,
}
#[derive(Clone, Debug, Default)]
pub struct DnsHostsEditor {
    pub open: bool,
    pub rows: Vec<HostDraftRow>,
    pub applied: Option<DnsHostsProfile>,
    pub address: String,
    pub domain: String,
    pub editing: Option<u64>,
    pub dirty: bool,
    pub importing_legacy: bool,
    pub ready: bool,
    pub read_failure: Option<Failure>,
    pub failure: Option<Failure>,
    pub issues: Vec<DnsHostsIssue>,
    pub pending: Option<PendingHosts>,
    latest: Option<DnsHostsProfile>,
    awaiting: Option<Vec<DnsHostEntry>>,
    next_row: u64,
    next_token: u64,
}
impl DnsHostsEditor {
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
                    self.ready = false;
                    self.failure = Some(Failure::new(
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
        self.ready = false;
        match &snapshot.status {
            PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
                self.read_failure = Some(failure.clone());
                return;
            }
            PageStatus::Loading => {
                self.read_failure = Some(Failure::new(
                    ErrorCode::NotReady,
                    "Hosts observation is loading",
                    true,
                ));
                return;
            }
            PageStatus::Ready | PageStatus::Empty => {}
        }
        let Some(profile) = snapshot
            .data
            .as_ref()
            .filter(|profile| !profile.profile.is_empty())
        else {
            self.read_failure = Some(Failure::new(
                ErrorCode::InvalidState,
                "Hosts observation has no profile identity",
                false,
            ));
            return;
        };
        if self
            .awaiting
            .as_ref()
            .is_some_and(|entries| entries != &profile.entries)
            && self
                .applied
                .as_ref()
                .is_some_and(|applied| applied.profile == profile.profile)
        {
            self.ready = true;
            self.read_failure = None;
            return;
        }
        self.latest = Some(profile.clone());
        if self.pending.is_some() {
            if self
                .applied
                .as_ref()
                .is_some_and(|applied| applied.profile != profile.profile)
            {
                self.read_failure = Some(Failure::new(
                    ErrorCode::InvalidState,
                    "active profile changed during Hosts save",
                    false,
                ));
            } else {
                self.ready = true;
                self.read_failure = None;
            }
            return;
        }

        if self.pending.is_none()
            && (self.dirty
                || self.editing.is_some()
                || !self.address.is_empty()
                || !self.domain.is_empty())
            && self.applied.as_ref().is_some_and(|applied| {
                applied.profile != profile.profile
                    || self.awaiting.is_none() && applied.entries != profile.entries
            })
        {
            self.read_failure = Some(Failure::new(
                ErrorCode::InvalidState,
                "configuration changed; cancel this draft to review the latest mappings",
                false,
            ));
            return;
        }
        self.ready = true;
        self.read_failure = None;
        if self
            .awaiting
            .as_ref()
            .is_some_and(|entries| entries != &profile.entries)
        {
            return;
        }
        self.awaiting = None;
        self.applied = Some(profile.clone());
        if !self.dirty && self.pending.is_none() {
            self.seed(&profile.entries);
        }
    }
    pub fn show(&mut self) {
        self.open = true;
    }
    pub fn edit_address(&mut self, value: String) {
        if self.pending.is_none() {
            self.address = value;
            self.issues.clear();
        }
    }
    pub fn edit_domain(&mut self, value: String) {
        if self.pending.is_none() {
            self.domain = value;
            self.issues.clear();
        }
    }
    pub fn select(&mut self, id: u64) {
        if self.pending.is_some() {
            return;
        }
        if self.editing.is_some() || !self.address.is_empty() || !self.domain.is_empty() {
            self.failure = Some(Failure::new(
                ErrorCode::InvalidState,
                "confirm or discard the current row input before editing another mapping",
                false,
            ));
            return;
        }
        if let Some(row) = self.rows.iter().find(|row| row.id == id) {
            self.address = row.entry.address.clone();
            self.domain = row.entry.domain.clone();
            self.editing = Some(id);
            self.issues.clear();
        }
    }
    pub fn cancel_row(&mut self) {
        if self.pending.is_some() {
            return;
        }
        self.address.clear();
        self.domain.clear();
        self.editing = None;
        self.issues.clear();
        self.failure = None;
    }
    pub fn commit_row(&mut self) -> bool {
        if self.pending.is_some() {
            return false;
        }
        let entry = DnsHostEntry {
            address: self.address.trim().into(),
            domain: self.domain.trim().into(),
        };
        self.issues = validate_hosts(from_ref(&entry));
        if !self.issues.is_empty() {
            return false;
        }
        if let Some(id) = self.editing {
            let Some(row) = self.rows.iter_mut().find(|row| row.id == id) else {
                return false;
            };
            row.entry = entry;
        } else if !self.rows.iter().any(|row| row.entry == entry) {
            let id = self.next_row;
            let Some(next) = self.next_row.checked_add(1) else {
                self.failure = Some(Failure::new(
                    ErrorCode::InvalidState,
                    "Hosts row identity space exhausted",
                    false,
                ));
                return false;
            };
            self.next_row = next;
            self.rows.push(HostDraftRow { id, entry });
        }
        self.address.clear();
        self.domain.clear();
        self.editing = None;
        self.dirty = true;
        self.failure = None;
        true
    }
    pub fn remove(&mut self, id: u64) {
        if self.pending.is_some() {
            return;
        }
        let count = self.rows.len();
        self.rows.retain(|row| row.id != id);
        if self.rows.len() != count {
            self.dirty = true;
            if self.editing == Some(id) {
                self.address.clear();
                self.domain.clear();
                self.editing = None;
            }
        }
    }
    pub fn import_legacy(&mut self) {
        if self.pending.is_some()
            || self.importing_legacy
            || self.editing.is_some()
            || !self.address.is_empty()
            || !self.domain.is_empty()
        {
            return;
        }
        let Some(profile) = self.applied.clone() else {
            return;
        };
        for entry in profile.legacy_entries {
            if !self.rows.iter().any(|row| row.entry == entry) {
                self.address = entry.address;
                self.domain = entry.domain;
                self.editing = None;
                if !self.commit_row() {
                    return;
                }
            }
        }
        self.importing_legacy = true;
        self.dirty = true;
    }
    pub fn cancel(&mut self) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.open = false;
        self.address.clear();
        self.domain.clear();
        self.editing = None;
        self.dirty = false;
        self.importing_legacy = false;
        self.failure = None;
        self.issues.clear();
        self.awaiting = None;
        if let Some(profile) = self.latest.clone() {
            self.seed(&profile.entries);
            self.applied = Some(profile);
        }
        true
    }
    pub fn can_apply(&self) -> bool {
        self.open
            && self.ready
            && self.dirty
            && self.pending.is_none()
            && self.address.is_empty()
            && self.domain.is_empty()
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
            remove_legacy_hosts: self.importing_legacy,
            ..Default::default()
        };
        let pending = PendingHosts {
            token: self.next_token,
            patch,
            entries,
        };
        self.pending = Some(pending.clone());
        self.failure = None;
        Ok(pending)
    }
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> bool {
        let Some(pending) = self
            .pending
            .as_ref()
            .filter(|pending| pending.token == token)
            .cloned()
        else {
            return false;
        };
        self.pending = None;
        match result {
            Ok(()) => {
                if let Some(profile) = &mut self.applied {
                    profile.entries = pending.entries.clone();
                    if pending.patch.remove_legacy_hosts {
                        profile.legacy_entries.clear();
                    }
                }
                if self.latest.as_ref().is_some_and(|latest| {
                    self.applied
                        .as_ref()
                        .is_some_and(|applied| latest.profile == applied.profile)
                }) {
                    self.latest = self.applied.clone();
                }
                self.seed(&pending.entries);
                self.awaiting = Some(pending.entries);
                self.dirty = false;
                self.importing_legacy = false;
                self.failure = None;
            }
            Err(failure) => self.failure = Some(failure),
        }
        true
    }
}

#[cfg(test)]
#[path = "dns_hosts_editor_test.rs"]
mod tests;
