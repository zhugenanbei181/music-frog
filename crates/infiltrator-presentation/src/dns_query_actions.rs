//! Query draft, result tabs, pagination, and correlated command/readback lifecycle.
use infiltrator_contract::dns_query::{
    DnsQueryOperationId, DnsQueryRequest, DnsQuerySnapshot, DnsRecord, DnsRecordType,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);
pub const QUERY_PAGE_SIZE: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QuerySection {
    #[default]
    Answer,
    Authority,
    Additional,
}

impl QuerySection {
    pub const ALL: [Self; 3] = [Self::Answer, Self::Authority, Self::Additional];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Answer => "dns_query_answers",
            Self::Authority => "dns_query_authority",
            Self::Additional => "dns_query_additional",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryReadback {
    Waiting,
    Observed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryCorrelation {
    Unscoped,
    Operation(DnsQueryOperationId),
}

/// Dialog, command, validation, and terminal-readback phases are mutually exclusive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DnsQueryPhase {
    Closed(QueryCorrelation),
    Editing,
    ValidationFailed(Failure),
    Submitting {
        operation: DnsQueryOperationId,
        readback: QueryReadback,
    },
    AwaitingReadback {
        operation: DnsQueryOperationId,
    },
    Observed {
        operation: DnsQueryOperationId,
    },
    Failed {
        operation: DnsQueryOperationId,
        failure: Failure,
    },
}

#[derive(Clone, Debug)]
pub struct DnsQueryActions {
    phase: DnsQueryPhase,
    pub name: String,
    pub record_type: DnsRecordType,
    pub section: QuerySection,
    pub page: usize,
    pub snapshot: DnsQuerySnapshot,
}

impl Default for DnsQueryActions {
    fn default() -> Self {
        Self {
            phase: DnsQueryPhase::Closed(QueryCorrelation::Unscoped),
            name: String::new(),
            record_type: DnsRecordType::A,
            section: QuerySection::Answer,
            page: 0,
            snapshot: DnsQuerySnapshot::unavailable(),
        }
    }
}

impl DnsQueryActions {
    pub fn phase(&self) -> &DnsQueryPhase {
        &self.phase
    }

    pub fn is_open(&self) -> bool {
        !matches!(&self.phase, DnsQueryPhase::Closed(_))
    }

    pub fn is_pending(&self) -> bool {
        matches!(&self.phase, DnsQueryPhase::Submitting { .. })
    }

    pub fn requested_operation(&self) -> Option<DnsQueryOperationId> {
        match &self.phase {
            DnsQueryPhase::Closed(QueryCorrelation::Operation(operation))
            | DnsQueryPhase::Submitting { operation, .. }
            | DnsQueryPhase::AwaitingReadback { operation }
            | DnsQueryPhase::Observed { operation }
            | DnsQueryPhase::Failed { operation, .. } => Some(*operation),
            DnsQueryPhase::Closed(QueryCorrelation::Unscoped)
            | DnsQueryPhase::Editing
            | DnsQueryPhase::ValidationFailed(_) => None,
        }
    }

    pub fn local_failure(&self) -> Option<&Failure> {
        match &self.phase {
            DnsQueryPhase::ValidationFailed(failure) | DnsQueryPhase::Failed { failure, .. } => {
                Some(failure)
            }
            _ => None,
        }
    }

    pub fn show(&mut self) {
        if !self.is_pending() {
            self.phase = DnsQueryPhase::Editing;
        }
    }

    pub fn cancel(&mut self) -> bool {
        if !self.is_open() || self.is_pending() {
            return false;
        }
        let correlation = self
            .requested_operation()
            .map_or(QueryCorrelation::Unscoped, QueryCorrelation::Operation);
        self.phase = DnsQueryPhase::Closed(correlation);
        true
    }

    pub fn set_name(&mut self, name: String) {
        if !self.is_pending() {
            self.name = name;
        }
    }

    pub fn set_type(&mut self, record_type: DnsRecordType) {
        if !self.is_pending() {
            self.record_type = record_type;
        }
    }

    pub fn select(&mut self, section: QuerySection) {
        self.section = section;
        self.page = 0;
    }

    pub fn rows(&self) -> &[DnsRecord] {
        self.snapshot
            .report
            .as_ref()
            .map(|report| match self.section {
                QuerySection::Answer => report.response.answers.as_slice(),
                QuerySection::Authority => report.response.authority.as_slice(),
                QuerySection::Additional => report.response.additional.as_slice(),
            })
            .unwrap_or_default()
    }

    pub fn next(&mut self) {
        if self.page < self.rows().len().saturating_sub(1) / QUERY_PAGE_SIZE {
            self.page += 1;
        }
    }

    pub fn previous(&mut self) {
        self.page = self.page.saturating_sub(1);
    }

    pub fn begin(&mut self) -> Result<(DnsQueryOperationId, DnsQueryRequest), Failure> {
        if !self.is_open() || self.is_pending() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Open the DNS query panel before submitting",
                false,
            ));
        }
        let request = DnsQueryRequest {
            name: self.name.trim().to_string(),
            record_type: self.record_type,
        };
        if let Err(failure) = request.validate() {
            self.phase = DnsQueryPhase::ValidationFailed(failure.clone());
            return Err(failure);
        }
        let operation = NEXT_OPERATION
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map(DnsQueryOperationId)
            .map_err(|_| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "DNS query identity exhausted",
                    false,
                )
            })?;
        self.phase = DnsQueryPhase::Submitting {
            operation,
            readback: QueryReadback::Waiting,
        };
        Ok((operation, request))
    }

    pub fn finish(&mut self, operation: DnsQueryOperationId, result: Result<(), Failure>) -> bool {
        let DnsQueryPhase::Submitting {
            operation: expected,
            readback,
        } = &self.phase
        else {
            return false;
        };
        if operation != *expected {
            return false;
        }
        self.phase = match result {
            Err(failure) => DnsQueryPhase::Failed { operation, failure },
            Ok(())
                if *readback == QueryReadback::Observed
                    || self.snapshot.report_id == Some(operation) =>
            {
                DnsQueryPhase::Observed { operation }
            }
            Ok(()) => DnsQueryPhase::AwaitingReadback { operation },
        };
        true
    }

    pub fn observe(&mut self, snapshot: &DnsQuerySnapshot) {
        let correlation = self.requested_operation();
        if snapshot.revision < self.snapshot.revision
            || correlation.is_some_and(|operation| {
                snapshot.operation_id != Some(operation) && snapshot.report_id != Some(operation)
            })
        {
            return;
        }
        let replaced = self.snapshot.report_id != snapshot.report_id;
        self.snapshot.clone_from(snapshot);
        self.phase = match self.phase.clone() {
            DnsQueryPhase::Submitting { operation, .. }
                if snapshot.report_id == Some(operation) =>
            {
                DnsQueryPhase::Submitting {
                    operation,
                    readback: QueryReadback::Observed,
                }
            }
            DnsQueryPhase::AwaitingReadback { operation }
                if snapshot.report_id == Some(operation) =>
            {
                DnsQueryPhase::Observed { operation }
            }
            phase => phase,
        };
        if replaced {
            self.section = QuerySection::Answer;
            self.page = 0;
        }
        self.page = self
            .page
            .min(self.rows().len().saturating_sub(1) / QUERY_PAGE_SIZE);
    }

    pub fn current_failure(&self) -> Option<&Failure> {
        self.local_failure().or_else(|| {
            (self.requested_operation() == self.snapshot.operation_id)
                .then_some(self.snapshot.failure.as_ref())
                .flatten()
        })
    }

    pub fn can_guide(&self) -> bool {
        self.is_open()
            && !self.is_pending()
            && self.current_failure().is_some_and(|failure| {
                matches!(
                    failure.code,
                    ErrorCode::Permission | ErrorCode::Authentication
                )
            })
    }

    pub fn can_retry(&self) -> bool {
        self.is_open()
            && !self.is_pending()
            && self.local_failure().is_some_and(|failure| {
                failure.retryable
                    || matches!(
                        failure.code,
                        ErrorCode::Permission | ErrorCode::Authentication
                    )
            })
    }
}
