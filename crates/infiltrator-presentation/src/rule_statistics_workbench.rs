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
pub enum StatisticsReadPhase {
    Unobserved,
    Loading(Failure),
    Ready,
    Failed(Failure),
}
impl Default for StatisticsReadPhase {
    fn default() -> Self {
        Self::Unobserved
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum StatisticsInspectionPhase {
    #[default]
    NotInspected,
    Inspected {
        zero_hit_rows: Vec<RuleRowId>,
    },
    Confirming {
        zero_hit_rows: Vec<RuleRowId>,
        confirmation: RuleCleanupConfirmation,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatisticsResetRequest {
    pub operation: u64,
    pub revision: u64,
    pub source: RuleSourceIdentity,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum StatisticsResetPhase {
    #[default]
    Idle,
    Submitting(StatisticsResetRequest),
    AwaitingReadback {
        source: RuleSourceIdentity,
        revision: u64,
    },
    Failed(Failure),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum StatisticsCleanupFeedback {
    #[default]
    None,
    Completed(usize),
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
    pub tab: StatisticsTab,
    pub page: usize,
    read: StatisticsReadPhase,
    inspection: StatisticsInspectionPhase,
    cleanup_feedback: StatisticsCleanupFeedback,
    reset: StatisticsResetPhase,
    revision_floor: u64,
}

impl RuleStatisticsWorkbench {
    pub fn read_phase(&self) -> &StatisticsReadPhase {
        &self.read
    }

    pub fn inspection_phase(&self) -> &StatisticsInspectionPhase {
        &self.inspection
    }

    pub fn reset_phase(&self) -> &StatisticsResetPhase {
        &self.reset
    }

    pub fn cleanup_feedback(&self) -> &StatisticsCleanupFeedback {
        &self.cleanup_feedback
    }

    pub fn read_failure(&self) -> Option<&Failure> {
        match &self.read {
            StatisticsReadPhase::Loading(failure) | StatisticsReadPhase::Failed(failure) => {
                Some(failure)
            }
            StatisticsReadPhase::Unobserved | StatisticsReadPhase::Ready => None,
        }
    }

    pub fn reset_failure(&self) -> Option<&Failure> {
        match &self.reset {
            StatisticsResetPhase::Failed(failure) => Some(failure),
            StatisticsResetPhase::Idle
            | StatisticsResetPhase::Submitting(_)
            | StatisticsResetPhase::AwaitingReadback { .. } => None,
        }
    }

    pub fn is_inspected(&self) -> bool {
        matches!(
            &self.inspection,
            StatisticsInspectionPhase::Inspected { .. }
                | StatisticsInspectionPhase::Confirming { .. }
        )
    }

    pub fn inspected_rows(&self) -> Option<&[RuleRowId]> {
        match &self.inspection {
            StatisticsInspectionPhase::Inspected { zero_hit_rows }
            | StatisticsInspectionPhase::Confirming { zero_hit_rows, .. } => Some(zero_hit_rows),
            StatisticsInspectionPhase::NotInspected => None,
        }
    }

    pub fn confirmation(&self) -> Option<&RuleCleanupConfirmation> {
        match &self.inspection {
            StatisticsInspectionPhase::Confirming { confirmation, .. } => Some(confirmation),
            StatisticsInspectionPhase::NotInspected
            | StatisticsInspectionPhase::Inspected { .. } => None,
        }
    }

    pub fn reset_request(&self) -> Option<&StatisticsResetRequest> {
        match &self.reset {
            StatisticsResetPhase::Submitting(request) => Some(request),
            StatisticsResetPhase::Idle
            | StatisticsResetPhase::AwaitingReadback { .. }
            | StatisticsResetPhase::Failed(_) => None,
        }
    }

    pub fn observe(&mut self, page: &PageData<RulesPageSnapshot>) {
        let source = page
            .data
            .as_ref()
            .and_then(|data| data.document.as_ref())
            .map(|document| document.source.clone());
        if let Some(source) = source {
            if self.source.as_ref() != Some(&source) {
                self.audit = None;
                self.invalidate_inspection_preserving_review();
                self.cleanup_feedback = StatisticsCleanupFeedback::None;
                if matches!(
                    &self.reset,
                    StatisticsResetPhase::AwaitingReadback { source: pending, .. }
                        if pending != &source
                ) {
                    self.reset = StatisticsResetPhase::Failed(Failure::new(
                        ErrorCode::NotReady,
                        "The rule source changed before reset readback was observed",
                        true,
                    ));
                }
            }
            self.source = Some(source);
        }
        self.read = match &page.status {
            PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
                StatisticsReadPhase::Failed(failure.clone())
            }
            PageStatus::Loading => StatisticsReadPhase::Loading(Failure::new(
                ErrorCode::NotReady,
                "Rule statistics are still loading",
                true,
            )),
            PageStatus::Ready | PageStatus::Empty => StatisticsReadPhase::Ready,
        };
        if !matches!(&self.read, StatisticsReadPhase::Ready) {
            return;
        }
        let next = page
            .data
            .as_ref()
            .and_then(|data| data.hit_audit.as_ref())
            .filter(|audit| self.source.is_some() && audit.source.as_ref() == self.source.as_ref());
        if let Some(next) = next {
            let boundary = match &self.reset {
                StatisticsResetPhase::AwaitingReadback { revision, .. } => *revision,
                StatisticsResetPhase::Idle
                | StatisticsResetPhase::Submitting(_)
                | StatisticsResetPhase::Failed(_) => self.revision_floor,
            };
            if next.revision < boundary {
                return;
            }
            if self.audit.as_ref() != Some(next) {
                self.invalidate_inspection_preserving_review();
            }
            self.revision_floor = next.revision;
            self.audit = Some(next.clone());
            if matches!(
                &self.reset,
                StatisticsResetPhase::AwaitingReadback { source, revision }
                    if next.source.as_ref() == Some(source) && next.revision >= *revision
            ) {
                self.reset = StatisticsResetPhase::Idle;
            }
        } else if !matches!(&self.reset, StatisticsResetPhase::AwaitingReadback { .. }) {
            self.audit = None;
            self.invalidate_inspection_preserving_review();
        }
    }

    fn invalidate_inspection_preserving_review(&mut self) {
        if !matches!(
            &self.inspection,
            StatisticsInspectionPhase::Confirming { .. }
        ) {
            self.inspection = StatisticsInspectionPhase::NotInspected;
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
        matches!(
            &self.reset,
            StatisticsResetPhase::Submitting(_) | StatisticsResetPhase::AwaitingReadback { .. }
        )
    }
    pub fn current(&self) -> bool {
        self.source.is_some()
            && matches!(&self.read, StatisticsReadPhase::Ready)
            && !matches!(&self.reset, StatisticsResetPhase::AwaitingReadback { .. })
            && self
                .audit
                .as_ref()
                .is_some_and(|audit| audit.source.as_ref() == self.source.as_ref())
    }
    pub fn can_inspect(&self, editor: &RuleListEditor) -> bool {
        self.current()
            && !self.busy()
            && self.confirmation().is_none()
            && can_audit_rows(editor, self.audit.as_ref())
    }
    pub fn inspect(&mut self, editor: &RuleListEditor) -> bool {
        if !self.can_inspect(editor) {
            return false;
        }
        self.inspection = StatisticsInspectionPhase::Inspected {
            zero_hit_rows: zero_hit_rows(editor, self.audit.as_ref()),
        };
        self.cleanup_feedback = StatisticsCleanupFeedback::None;
        true
    }

    pub fn prepare_cleanup(&mut self, editor: &RuleListEditor) -> bool {
        if !self.current() || self.busy() || self.confirmation().is_some() {
            return false;
        }
        let StatisticsInspectionPhase::Inspected {
            zero_hit_rows: inspected_rows,
        } = &self.inspection
        else {
            return false;
        };
        let allowed = zero_hit_rows(editor, self.audit.as_ref());
        let targets = inspected_rows
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
        self.inspection = StatisticsInspectionPhase::Confirming {
            zero_hit_rows: inspected_rows.clone(),
            confirmation: RuleCleanupConfirmation { source, targets },
        };
        true
    }
    pub fn can_confirm_cleanup(&self, editor: &RuleListEditor) -> bool {
        let Some(confirmation) = self.confirmation() else {
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
        match std::mem::take(&mut self.inspection) {
            StatisticsInspectionPhase::Confirming { zero_hit_rows, .. } => {
                self.inspection = StatisticsInspectionPhase::Inspected { zero_hit_rows };
                true
            }
            phase => {
                self.inspection = phase;
                false
            }
        }
    }
    pub fn confirm_cleanup(&mut self, editor: &mut RuleListEditor) -> Option<usize> {
        if !self.can_confirm_cleanup(editor) {
            return None;
        }
        let count = disable_zero_hit_rows(
            editor,
            self.audit.as_ref(),
            &self
                .confirmation()?
                .targets
                .iter()
                .map(|target| target.id)
                .collect::<Vec<_>>(),
        );
        self.inspection = StatisticsInspectionPhase::NotInspected;
        self.cleanup_feedback = StatisticsCleanupFeedback::Completed(count);
        Some(count)
    }

    pub fn can_reset(&self) -> bool {
        self.current()
            && !self.busy()
            && self.confirmation().is_none()
            && clear_source(self.audit.as_ref()).is_some()
    }
    pub fn begin_reset(&mut self) -> Option<StatisticsResetRequest> {
        if !self.can_reset() {
            return None;
        }
        let source = clear_source(self.audit.as_ref())?.clone();
        let operation = match NEXT_RESET.try_update(Ordering::SeqCst, Ordering::SeqCst, |next| {
            next.checked_add(1)
        }) {
            Ok(operation) => operation,
            Err(_) => {
                self.reset = StatisticsResetPhase::Failed(Failure::new(
                    ErrorCode::InvalidState,
                    "Statistics reset identity exhausted",
                    false,
                ));
                return None;
            }
        };
        let request = StatisticsResetRequest {
            operation,
            source,
            revision: self.audit.as_ref()?.revision,
        };
        self.reset = StatisticsResetPhase::Submitting(request.clone());
        Some(request)
    }
    pub fn finish_reset(
        &mut self,
        request: &StatisticsResetRequest,
        result: Result<RuleStatisticsResetReceipt, Failure>,
    ) -> bool {
        if self.reset_request() != Some(request) {
            return false;
        }
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
                self.reset = if self.audit.as_ref().is_some_and(|audit| {
                    audit.source.as_ref() == Some(&receipt.source)
                        && audit.revision >= receipt.revision
                }) {
                    StatisticsResetPhase::Idle
                } else {
                    StatisticsResetPhase::AwaitingReadback {
                        source: receipt.source,
                        revision: receipt.revision,
                    }
                };
            }
            Err(failure) => self.reset = StatisticsResetPhase::Failed(failure),
        }
        true
    }
    pub fn dismiss_failure(&mut self) -> bool {
        if matches!(&self.reset, StatisticsResetPhase::Failed(_)) {
            self.reset = StatisticsResetPhase::Idle;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
#[path = "rule_statistics_workbench_test.rs"]
mod tests;
