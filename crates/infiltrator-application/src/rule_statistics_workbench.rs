//! One native statistics inspector, cleanup confirmation and reset-readback state machine.
use crate::rule_list_editor::RuleListEditor;
use crate::rule_statistics_projection::{
    can_audit_rows, clear_source, disable_zero_hit_rows, zero_hit_rows,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_document::RuleRowId;
use infiltrator_contract::rule_hit_audit::RuleHitAuditSnapshot;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::rule_statistics::RuleStatisticsResetReceipt;
use infiltrator_contract::surface_snapshot::{PageData, PageStatus, RulesPageSnapshot};
use std::sync::atomic::{AtomicU64, Ordering};

pub const STATS_ROWS_PER_PAGE: usize = 8;

static NEXT_RESET: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatisticsTab {
    #[default]
    Summary,
    TopHits,
    Inactive,
}
impl StatisticsTab {
    pub const ALL: [Self; 3] = [Self::Summary, Self::TopHits, Self::Inactive];
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Summary => "rules_stats_summary_tab",
            Self::TopHits => "rules_stats_top_tab",
            Self::Inactive => "rules_stats_inactive_tab",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleCleanupTarget {
    pub id: RuleRowId,
    pub raw: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleCleanupConfirmation {
    pub source: RuleSourceIdentity,
    pub targets: Vec<RuleCleanupTarget>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatisticsResetRequest {
    pub operation: u64,
    pub revision: u64,
    pub source: RuleSourceIdentity,
}

#[derive(Clone, Debug)]
pub enum StatisticsAction {
    Tab(StatisticsTab),
    PreviousPage,
    NextPage,
    Inspect,
    PrepareCleanup,
    CancelCleanup,
    ConfirmCleanup,
    Reset,
    DismissFailure,
    ResetFinished {
        request: StatisticsResetRequest,
        result: Result<RuleStatisticsResetReceipt, Failure>,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleStatisticsWorkbench {
    pub audit: Option<RuleHitAuditSnapshot>,
    pub source: Option<RuleSourceIdentity>,
    pub read_failure: Option<Failure>,
    pub tab: StatisticsTab,
    pub page: usize,
    pub zero_hit_rows: Vec<RuleRowId>,
    pub inspected: bool,
    pub confirmation: Option<RuleCleanupConfirmation>,
    pub last_cleanup_count: Option<usize>,
    pub clear_pending: Option<StatisticsResetRequest>,
    pub clear_failure: Option<Failure>,
    pub awaiting_revision: Option<(RuleSourceIdentity, u64)>,
    revision_floor: u64,
}

impl RuleStatisticsWorkbench {
    pub fn observe(&mut self, page: &PageData<RulesPageSnapshot>) {
        let source = page
            .data
            .as_ref()
            .and_then(|data| data.document.as_ref())
            .map(|document| document.source.clone());
        if let Some(source) = source {
            if self.source.as_ref() != Some(&source) {
                self.audit = None;
                self.zero_hit_rows.clear();
                self.inspected = false;
                self.last_cleanup_count = None;
                self.awaiting_revision = None;
            }
            self.source = Some(source);
        }
        self.read_failure = match &page.status {
            PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
                Some(failure.clone())
            }
            PageStatus::Loading => Some(Failure::new(
                ErrorCode::NotReady,
                "Rule statistics are still loading",
                true,
            )),
            PageStatus::Ready | PageStatus::Empty => None,
        };
        if self.read_failure.is_some() {
            return;
        }
        let next = page
            .data
            .as_ref()
            .and_then(|data| data.hit_audit.as_ref())
            .filter(|audit| self.source.is_some() && audit.source.as_ref() == self.source.as_ref());
        if let Some(next) = next {
            let boundary = self
                .awaiting_revision
                .as_ref()
                .map_or(self.revision_floor, |(_, revision)| *revision);
            if next.revision < boundary {
                return;
            }
            if self.audit.as_ref() != Some(next) {
                self.zero_hit_rows.clear();
                self.inspected = false;
            }
            self.revision_floor = next.revision;
            self.audit = Some(next.clone());
            if self
                .awaiting_revision
                .as_ref()
                .is_some_and(|(source, revision)| {
                    next.source.as_ref() == Some(source) && next.revision >= *revision
                })
            {
                self.awaiting_revision = None;
            }
        } else if self.awaiting_revision.is_none() {
            self.audit = None;
            self.zero_hit_rows.clear();
            self.inspected = false;
        }
    }

    pub fn select_tab(&mut self, tab: StatisticsTab) {
        if self.tab != tab {
            self.tab = tab;
            self.page = 0;
        }
    }
    pub fn previous_page(&mut self) {
        self.page = self.page.saturating_sub(1);
    }
    pub fn next_page(&mut self) {
        let total = self.audit.as_ref().map_or(0, |audit| match self.tab {
            StatisticsTab::Summary => 0,
            StatisticsTab::TopHits => audit.top_hits.len(),
            StatisticsTab::Inactive => audit.dead_rules.len(),
        });
        if self.page + 1 < total.div_ceil(STATS_ROWS_PER_PAGE).max(1) {
            self.page += 1;
        }
    }
    pub fn busy(&self) -> bool {
        self.clear_pending.is_some() || self.awaiting_revision.is_some()
    }
    pub fn current(&self) -> bool {
        self.source.is_some()
            && self.read_failure.is_none()
            && self.awaiting_revision.is_none()
            && self
                .audit
                .as_ref()
                .is_some_and(|audit| audit.source.as_ref() == self.source.as_ref())
    }
    pub fn can_inspect(&self, editor: &RuleListEditor) -> bool {
        self.current()
            && !self.busy()
            && self.confirmation.is_none()
            && can_audit_rows(editor, self.audit.as_ref())
    }
    pub fn inspect(&mut self, editor: &RuleListEditor) -> bool {
        if !self.can_inspect(editor) {
            return false;
        }
        self.zero_hit_rows = zero_hit_rows(editor, self.audit.as_ref());
        self.inspected = true;
        self.last_cleanup_count = None;
        true
    }

    pub fn prepare_cleanup(&mut self, editor: &RuleListEditor) -> bool {
        if !self.current() || self.busy() || self.confirmation.is_some() || !self.inspected {
            return false;
        }
        let allowed = zero_hit_rows(editor, self.audit.as_ref());
        let targets = self
            .zero_hit_rows
            .iter()
            .filter(|id| allowed.contains(id))
            .filter_map(|id| {
                editor.row_index(*id).map(|index| RuleCleanupTarget {
                    id: *id,
                    raw: editor.draft[index].rule.clone(),
                })
            })
            .collect::<Vec<_>>();
        let Some(source) = self.source.clone() else {
            return false;
        };
        if targets.is_empty() {
            return false;
        }
        self.confirmation = Some(RuleCleanupConfirmation { source, targets });
        true
    }
    pub fn can_confirm_cleanup(&self, editor: &RuleListEditor) -> bool {
        let Some(confirmation) = &self.confirmation else {
            return false;
        };
        if !self.current() || self.busy() || self.source.as_ref() != Some(&confirmation.source) {
            return false;
        }
        let allowed = zero_hit_rows(editor, self.audit.as_ref());
        !confirmation.targets.is_empty()
            && confirmation.targets.iter().all(|target| {
                allowed.contains(&target.id)
                    && editor
                        .row_index(target.id)
                        .is_some_and(|index| editor.draft[index].rule == target.raw)
            })
    }
    pub fn cancel_cleanup(&mut self) -> bool {
        self.confirmation.take().is_some()
    }
    pub fn confirm_cleanup(&mut self, editor: &mut RuleListEditor) -> Option<usize> {
        if !self.can_confirm_cleanup(editor) {
            return None;
        }
        let confirmation = self.confirmation.take()?;
        let requested = confirmation
            .targets
            .iter()
            .map(|target| target.id)
            .collect::<Vec<_>>();
        let count = disable_zero_hit_rows(editor, self.audit.as_ref(), &requested);
        self.zero_hit_rows.clear();
        self.inspected = false;
        self.last_cleanup_count = Some(count);
        Some(count)
    }

    pub fn can_reset(&self) -> bool {
        self.current()
            && !self.busy()
            && self.confirmation.is_none()
            && clear_source(self.audit.as_ref()).is_some()
    }
    pub fn begin_reset(&mut self) -> Option<StatisticsResetRequest> {
        if !self.can_reset() {
            return None;
        }
        let source = clear_source(self.audit.as_ref())?.clone();
        let operation = NEXT_RESET
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |next| {
                next.checked_add(1)
            })
            .expect("statistics operation identity exhausted");
        let request = StatisticsResetRequest {
            operation,
            source,
            revision: self.audit.as_ref()?.revision,
        };
        self.clear_pending = Some(request.clone());
        self.clear_failure = None;
        Some(request)
    }
    pub fn finish_reset(
        &mut self,
        request: &StatisticsResetRequest,
        result: Result<RuleStatisticsResetReceipt, Failure>,
    ) -> bool {
        if self.clear_pending.as_ref() != Some(request) {
            return false;
        }
        self.clear_pending = None;
        let result = result.and_then(|receipt| {
            receipt.validate(&request.source)?;
            if self.source.as_ref() != Some(&request.source) || receipt.revision <= request.revision
            {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "Statistics reset source or revision is stale",
                    false,
                ));
            }
            Ok(receipt)
        });
        match result {
            Ok(receipt) => {
                self.revision_floor = self.revision_floor.max(receipt.revision);
                self.awaiting_revision = if self.audit.as_ref().is_some_and(|audit| {
                    audit.source.as_ref() == Some(&receipt.source)
                        && audit.revision >= receipt.revision
                }) {
                    None
                } else {
                    Some((receipt.source, receipt.revision))
                };
                self.clear_failure = None;
            }
            Err(failure) => {
                self.clear_failure = Some(failure);
                self.awaiting_revision = None;
            }
        }
        true
    }
}

#[cfg(test)]
#[path = "rule_statistics_workbench_test.rs"]
mod tests;
