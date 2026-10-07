//! Product-local editor facts are separated by profile, source bytes and read operation.
use crate::profile_application::ProfileApplication;
use infiltrator_contract::error::Failure;
use infiltrator_contract::profile_document::ProfileDocumentSnapshot;
use infiltrator_contract::profile_editor_read::{
    ProfileEditorReadSnapshot, ProfileEditorSnapshot, ProfileReadStatus,
};
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_domain::profile_source::hash_document_bytes;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
#[derive(Clone, Copy)]
pub(crate) enum EditorFacet {
    Document,
    Options,
}
#[derive(Clone)]
pub(crate) struct EditorRead {
    owner: u64,
    profile: String,
    epoch: u64,
    sequence: u64,
    facet: EditorFacet,
}
#[derive(Default)]
struct ProfileFacts {
    source: Option<ProfileSourceIdentity>,
    sequence: u64,
    document: Option<ProfileDocumentSnapshot>,
    options: Option<ProfileOptionsSnapshot>,
    document_status: ProfileReadStatus,
    options_status: ProfileReadStatus,
}
pub(crate) struct EditorObservations {
    owner: u64,
    selected: Option<String>,
    epoch: u64,
    sequence: u64,
    document_read: u64,
    options_read: u64,
    facts: BTreeMap<String, ProfileFacts>,
}
impl Default for EditorObservations {
    fn default() -> Self {
        static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
        let owner = NEXT_OWNER
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("editor observation owner exhausted");
        Self {
            owner,
            selected: None,
            epoch: 0,
            sequence: 0,
            document_read: 0,
            options_read: 0,
            facts: BTreeMap::new(),
        }
    }
}
impl EditorObservations {
    pub fn begin(&mut self, profile: &str, facet: EditorFacet) -> EditorRead {
        if self.selected.as_deref() != Some(profile) {
            self.epoch = self
                .epoch
                .checked_add(1)
                .expect("editor observation epoch exhausted");
            self.selected = Some(profile.into());
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .expect("editor observation sequence exhausted");
        match facet {
            EditorFacet::Document => self.document_read = self.sequence,
            EditorFacet::Options => self.options_read = self.sequence,
        }
        EditorRead {
            owner: self.owner,
            profile: profile.into(),
            epoch: self.epoch,
            sequence: self.sequence,
            facet,
        }
    }
    fn accepts_read(&self, read: &EditorRead) -> bool {
        read.owner == self.owner
            && read.epoch == self.epoch
            && self.selected.as_ref() == Some(&read.profile)
            && read.sequence
                == match read.facet {
                    EditorFacet::Document => self.document_read,
                    EditorFacet::Options => self.options_read,
                }
    }
    fn source(
        &mut self,
        source: &ProfileSourceIdentity,
        sequence: u64,
    ) -> Option<&mut ProfileFacts> {
        let facts = self.facts.entry(source.profile.clone()).or_default();
        if facts.source.as_ref() != Some(source) {
            if sequence < facts.sequence {
                return None;
            }
            facts.document = None;
            facts.options = None;
            facts.document_status = ProfileReadStatus::Unobserved;
            facts.options_status = ProfileReadStatus::Unobserved;
            facts.source = Some(source.clone());
        }
        facts.sequence = facts.sequence.max(sequence);
        Some(facts)
    }
    pub fn document(&mut self, read: &EditorRead, document: ProfileDocumentSnapshot) -> bool {
        let Some(source) = document.source.clone() else {
            return false;
        };
        if document.profile != source.profile
            || hash_document_bytes(&document.content) != source.document_hash
            || document.line_count != document.content.lines().count()
            || !self.accepts_read(read)
            || read.profile != source.profile
            || !matches!(read.facet, EditorFacet::Document)
        {
            return false;
        }
        let Some(facts) = self.source(&source, read.sequence) else {
            return false;
        };
        facts.document = Some(document);
        facts.document_status = ProfileReadStatus::Ready;
        true
    }
    pub fn options(&mut self, read: &EditorRead, options: ProfileOptionsSnapshot) -> bool {
        if !self.accepts_read(read)
            || read.profile != options.source.profile
            || !matches!(read.facet, EditorFacet::Options)
        {
            return false;
        }
        let Some(facts) = self.source(&options.source, read.sequence) else {
            return false;
        };
        facts.options = Some(options);
        facts.options_status = ProfileReadStatus::Ready;
        true
    }
    pub fn loading(&mut self, read: &EditorRead) {
        if !self.accepts_read(read) {
            return;
        }
        let facts = self.facts.entry(read.profile.clone()).or_default();
        match read.facet {
            EditorFacet::Document => facts.document_status = ProfileReadStatus::Loading,
            EditorFacet::Options => facts.options_status = ProfileReadStatus::Loading,
        }
    }
    pub fn failure(&mut self, read: &EditorRead, failure: Failure) -> bool {
        if !self.accepts_read(read) {
            return false;
        }
        let facts = self.facts.entry(read.profile.clone()).or_default();
        match read.facet {
            EditorFacet::Document => facts.document_status = ProfileReadStatus::Failed(failure),
            EditorFacet::Options => facts.options_status = ProfileReadStatus::Failed(failure),
        }
        true
    }
    pub fn read_snapshot(&self) -> ProfileEditorReadSnapshot {
        let mut snapshot = ProfileEditorReadSnapshot {
            profile: self.selected.clone(),
            ..Default::default()
        };
        if let Some(facts) = self
            .selected
            .as_ref()
            .and_then(|profile| self.facts.get(profile))
        {
            snapshot.observed_source = facts.source.clone();
            snapshot.document = facts.document_status.clone();
            snapshot.options = facts.options_status.clone();
        }
        snapshot
    }
    pub fn retire(&mut self, profile: &str) {
        self.facts.remove(profile);
        if self.selected.as_deref() == Some(profile) {
            self.epoch = self
                .epoch
                .checked_add(1)
                .expect("editor observation epoch exhausted");
            self.selected = None;
        }
    }
    pub fn current(
        &self,
    ) -> (
        Option<ProfileDocumentSnapshot>,
        Option<ProfileOptionsSnapshot>,
    ) {
        self.selected
            .as_ref()
            .and_then(|profile| self.facts.get(profile))
            .map(|facts| (facts.document.clone(), facts.options.clone()))
            .unwrap_or_default()
    }
    pub fn for_source(
        &self,
        source: &ProfileSourceIdentity,
    ) -> (
        Option<ProfileDocumentSnapshot>,
        Option<ProfileOptionsSnapshot>,
    ) {
        self.facts
            .get(&source.profile)
            .filter(|facts| facts.source.as_ref() == Some(source))
            .map(|facts| (facts.document.clone(), facts.options.clone()))
            .unwrap_or_default()
    }
}

impl ProfileApplication {
    pub(crate) fn retire_editor_profile(&self, profile: &str) {
        self.editor_observations
            .lock()
            .expect("editor observations")
            .retire(profile);
    }
    pub(crate) fn begin_editor_read(&self, profile: &str, facet: EditorFacet) -> EditorRead {
        self.editor_observations
            .lock()
            .expect("editor observations")
            .begin(profile, facet)
    }
    pub(crate) fn observe_document(
        &self,
        read: &EditorRead,
        document: ProfileDocumentSnapshot,
    ) -> bool {
        self.editor_observations
            .lock()
            .expect("editor observations")
            .document(read, document)
    }
    pub(crate) fn observe_options(
        &self,
        read: &EditorRead,
        options: ProfileOptionsSnapshot,
    ) -> bool {
        self.editor_observations
            .lock()
            .expect("editor observations")
            .options(read, options)
    }
    pub(crate) fn mark_editor_loading(&self, read: &EditorRead) {
        self.editor_observations
            .lock()
            .expect("editor observations")
            .loading(read);
    }
    pub(crate) fn fail_editor_read(&self, read: &EditorRead, failure: Failure) -> bool {
        self.editor_observations
            .lock()
            .expect("editor observations")
            .failure(read, failure)
    }
    pub fn editor_read_status(&self) -> ProfileEditorReadSnapshot {
        self.editor_observations
            .lock()
            .expect("editor observations")
            .read_snapshot()
    }
    pub async fn verified_editor_snapshot(&self) -> ProfileEditorSnapshot {
        let (mut snapshot, epoch, sequence) = {
            let owner = self
                .editor_observations
                .lock()
                .expect("editor observations");
            let (document, options) = owner.current();
            (
                ProfileEditorSnapshot {
                    document,
                    options,
                    read: owner.read_snapshot(),
                },
                owner.epoch,
                owner.sequence,
            )
        };
        let Some(profile) = snapshot.read.profile.clone() else {
            return snapshot;
        };
        match self.load_workspace(&profile).await {
            Ok(workspace) => {
                if let Some(document) = snapshot.document.as_mut()
                    && document.source.as_ref() == Some(&workspace.source)
                {
                    document.write_protection = workspace.write_protection;
                }
                snapshot.read.verified_source = Some(workspace.source);
            }
            Err(failure) => snapshot.read.verification_failure = Some(failure),
        }
        let owner = self
            .editor_observations
            .lock()
            .expect("editor observations");
        if owner.epoch != epoch || owner.sequence != sequence {
            let (document, options) = owner.current();
            return ProfileEditorSnapshot {
                document,
                options,
                read: owner.read_snapshot(),
            };
        }
        snapshot
    }
    pub fn editor_observations(
        &self,
    ) -> (
        Option<ProfileDocumentSnapshot>,
        Option<ProfileOptionsSnapshot>,
    ) {
        self.editor_observations
            .lock()
            .expect("editor observations")
            .current()
    }
    pub fn editor_observations_for(
        &self,
        source: &ProfileSourceIdentity,
    ) -> (
        Option<ProfileDocumentSnapshot>,
        Option<ProfileOptionsSnapshot>,
    ) {
        self.editor_observations
            .lock()
            .expect("editor observations")
            .for_source(source)
    }
}
