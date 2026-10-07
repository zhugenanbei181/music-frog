//! One draft, cancellation and source-fence model for either native rules surface.
use crate::rule_list_application::{domain_rules, rule_definitions};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_document::{
    RuleDocumentSnapshot, RuleListCommit, RuleListOperationId, RuleRowId,
};
use infiltrator_contract::rule_edit::{RuleDraft, RuleMoveDirection};
use infiltrator_contract::surface_snapshot::{PageData, PageStatus, RulesPageSnapshot};
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::rules::edit::{
    build_custom_rule, move_rule, prepend_rules, toggle_rule_enabled,
};
use infiltrator_domain::rules::game_routing_presets;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);
static NEXT_ROW: AtomicU64 = AtomicU64::new(1);

fn new_row_id() -> RuleRowId {
    RuleRowId(
        NEXT_ROW
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_add(1)
            })
            .expect("rule row identity exhausted"),
    )
}

#[derive(Clone, Debug, Default)]
pub struct RuleListEditor {
    pub base: Option<RuleDocumentSnapshot>,
    pub latest: Option<RuleDocumentSnapshot>,
    pub draft: Vec<RuleEntry>,
    row_ids: Vec<RuleRowId>,
    base_row_indices: HashMap<RuleRowId, usize>,
    pub pending: Option<RuleListOperationId>,
    pub failure: Option<Failure>,
    pub read_failure: Option<Failure>,
    pub awaiting_read: bool,
}
impl RuleListEditor {
    pub fn row_id(&self, index: usize) -> Option<RuleRowId> {
        self.row_ids.get(index).copied()
    }
    pub fn base_row_index(&self, id: RuleRowId) -> Option<usize> {
        self.base_row_indices.get(&id).copied()
    }
    fn reset_row_identities(&mut self) {
        self.row_ids = self.draft.iter().map(|_| new_row_id()).collect();
        self.base_row_indices = self
            .row_ids
            .iter()
            .copied()
            .enumerate()
            .map(|(index, id)| (id, index))
            .collect();
    }
    pub fn row_index(&self, id: RuleRowId) -> Option<usize> {
        self.row_ids.iter().position(|candidate| *candidate == id)
    }
    pub fn can_guide(&self) -> bool {
        self.pending.is_none()
            && !self.awaiting_read
            && self
                .read_failure
                .as_ref()
                .or(self.failure.as_ref())
                .is_some_and(|failure| {
                    matches!(
                        failure.code,
                        ErrorCode::Permission | ErrorCode::Authentication
                    )
                })
    }
    pub fn observe_page(&mut self, page: &PageData<RulesPageSnapshot>) -> bool {
        let document = page.data.as_ref().and_then(|page| page.document.as_ref());
        let failure = match &page.status {
            PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
                Some(failure.clone())
            }
            PageStatus::Loading => Some(Failure::new(
                ErrorCode::NotReady,
                "The current rule source is still loading",
                true,
            )),
            PageStatus::Ready | PageStatus::Empty => None,
        };
        self.observe(document, failure)
    }
    pub fn dirty(&self) -> bool {
        self.base
            .as_ref()
            .is_some_and(|base| base.rules != rule_definitions(&self.draft))
    }
    pub fn source_changed(&self) -> bool {
        self.base
            .as_ref()
            .zip(self.latest.as_ref())
            .is_some_and(|(base, latest)| base.source != latest.source)
    }
    pub fn editable(&self) -> bool {
        self.base.is_some()
            && self.pending.is_none()
            && !self.awaiting_read
            && self.read_failure.is_none()
            && !self.source_changed()
    }
    pub fn can_save(&self) -> bool {
        self.editable() && self.dirty()
    }
    pub fn observe(
        &mut self,
        document: Option<&RuleDocumentSnapshot>,
        failure: Option<Failure>,
    ) -> bool {
        self.read_failure = failure;
        let Some(document) = document else {
            if self.read_failure.is_none() {
                self.read_failure = Some(Failure::new(
                    ErrorCode::NotReady,
                    "The current rule document has not been observed",
                    true,
                ));
            }
            return false;
        };
        if self.read_failure.is_some() {
            return false;
        }
        self.latest = Some(document.clone());
        if self.pending.is_some() {
            return false;
        }
        if self.awaiting_read {
            let same_profile = self
                .base
                .as_ref()
                .is_some_and(|base| base.source.profile == document.source.profile);
            if same_profile && document.rules != rule_definitions(&self.draft) {
                if self
                    .base
                    .as_ref()
                    .is_some_and(|base| base.source != document.source)
                {
                    self.awaiting_read = false;
                    self.failure = Some(Failure::new(
                        ErrorCode::NotReady,
                        "The saved rules changed before they could be observed; retain or discard this draft",
                        true,
                    ));
                }
                return false;
            }
            self.awaiting_read = false;
            return self.adopt(document.clone());
        }
        if self.dirty() {
            return false;
        }
        self.adopt(document.clone())
    }
    fn adopt(&mut self, document: RuleDocumentSnapshot) -> bool {
        if self.base.as_ref() == Some(&document) {
            return false;
        }
        self.draft = domain_rules(&document.rules);
        self.reset_row_identities();
        self.base = Some(document);
        self.failure = None;
        true
    }
    pub fn discard(&mut self) -> bool {
        if self.pending.is_some() || self.awaiting_read {
            return false;
        }
        let Some(document) = self.latest.clone() else {
            return false;
        };
        self.draft = domain_rules(&document.rules);
        self.reset_row_identities();
        self.base = Some(document);
        self.failure = None;
        true
    }
    pub fn toggle(&mut self, id: RuleRowId) -> bool {
        self.editable()
            && self
                .row_index(id)
                .is_some_and(|index| toggle_rule_enabled(&mut self.draft, index))
    }
    pub fn move_rule(&mut self, id: RuleRowId, direction: RuleMoveDirection) -> bool {
        let Some(index) = self.row_index(id) else {
            return false;
        };
        if !self.editable() || !move_rule(&mut self.draft, index, direction) {
            return false;
        }
        let other = match direction {
            RuleMoveDirection::Up => index - 1,
            RuleMoveDirection::Down => index + 1,
        };
        self.row_ids.swap(index, other);
        true
    }
    pub fn remove(&mut self, id: RuleRowId) -> bool {
        let Some(index) = self.row_index(id) else {
            return false;
        };
        if !self.editable() {
            return false;
        }
        self.draft.remove(index);
        self.row_ids.remove(index);
        true
    }
    pub fn prepend(&mut self, entries: impl IntoIterator<Item = RuleEntry>) -> bool {
        if !self.editable() {
            return false;
        }
        let entries: Vec<_> = entries.into_iter().collect();
        let ids: Vec<_> = entries.iter().map(|_| new_row_id()).collect();
        let changed = prepend_rules(&mut self.draft, entries) > 0;
        self.row_ids.splice(0..0, ids);
        changed
    }
    pub fn add(&mut self, draft: &RuleDraft) -> Result<bool, Failure> {
        let entry = build_custom_rule(draft)
            .map_err(|message| Failure::new(ErrorCode::InvalidInput, message, false))?;
        Ok(self.prepend([entry]))
    }
    pub fn game_presets(&mut self, target: &str) -> bool {
        self.prepend(game_routing_presets(target))
    }
    pub fn begin(&mut self) -> Result<(RuleListOperationId, RuleListCommit), Failure> {
        if !self.can_save() {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The draft is unavailable, unchanged, busy or belongs to another document",
                true,
            ));
        }
        let token = NEXT_OPERATION
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_add(1)
            })
            .map_err(|_| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "Rule edit identity exhausted",
                    false,
                )
            })?;
        let operation = RuleListOperationId(token);
        let request = RuleListCommit {
            expected_source: self.base.as_ref().expect("available base").source.clone(),
            rules: rule_definitions(&self.draft),
        };
        self.pending = Some(operation);
        self.failure = None;
        Ok((operation, request))
    }
    pub fn finish(&mut self, operation: RuleListOperationId, result: Result<(), Failure>) -> bool {
        if self.pending != Some(operation) {
            return false;
        }
        self.pending = None;
        match result {
            Ok(()) => {
                self.awaiting_read = true;
                if let Some(latest) = self.latest.clone()
                    && self
                        .base
                        .as_ref()
                        .is_some_and(|base| base.source != latest.source)
                    && latest.rules == rule_definitions(&self.draft)
                {
                    self.observe(Some(&latest), None);
                }
            }
            Err(failure) => self.failure = Some(failure),
        }
        true
    }
}
