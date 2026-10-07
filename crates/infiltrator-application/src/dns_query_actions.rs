//! Query draft, result tabs, pagination and terminal correlation shared by both peers.
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
#[derive(Clone, Debug)]
pub struct DnsQueryActions {
    pub open: bool,
    pub name: String,
    pub record_type: DnsRecordType,
    pub section: QuerySection,
    pub page: usize,
    pub pending: Option<u64>,
    pub requested: Option<DnsQueryOperationId>,
    pub failure: Option<Failure>,
    pub snapshot: DnsQuerySnapshot,
}
impl Default for DnsQueryActions {
    fn default() -> Self {
        Self {
            open: false,
            name: String::new(),
            record_type: DnsRecordType::A,
            section: QuerySection::Answer,
            page: 0,
            pending: None,
            requested: None,
            failure: None,
            snapshot: DnsQuerySnapshot::unavailable(),
        }
    }
}
impl DnsQueryActions {
    pub fn show(&mut self) {
        if self.pending.is_none() {
            self.open = true;
            self.requested = None;
            self.failure = None;
        }
    }
    pub fn cancel(&mut self) -> bool {
        if !self.open || self.pending.is_some() {
            return false;
        }
        self.open = false;
        self.failure = None;
        true
    }
    pub fn set_name(&mut self, name: String) {
        if self.pending.is_none() {
            self.name = name;
        }
    }
    pub fn set_type(&mut self, record_type: DnsRecordType) {
        if self.pending.is_none() {
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
    pub fn begin(&mut self) -> Result<(u64, DnsQueryRequest), Failure> {
        if !self.open || self.pending.is_some() {
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
            self.failure = Some(failure.clone());
            return Err(failure);
        }
        let token = NEXT_OPERATION
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "DNS query identity exhausted",
                    false,
                )
            })?;
        self.pending = Some(token);
        self.requested = Some(DnsQueryOperationId(token));
        self.failure = None;
        Ok((token, request))
    }
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> bool {
        if self.pending != Some(token) {
            return false;
        }
        self.pending = None;
        self.failure = result.err();
        true
    }
    pub fn observe(&mut self, snapshot: &DnsQuerySnapshot) {
        if snapshot.revision < self.snapshot.revision
            || (self.requested.is_some()
                && self.requested != snapshot.operation_id
                && self.requested != snapshot.report_id)
        {
            return;
        }
        let replaced = self.snapshot.report_id != snapshot.report_id;
        self.snapshot = snapshot.clone();
        if replaced {
            self.section = QuerySection::Answer;
            self.page = 0;
        }
        self.page = self
            .page
            .min(self.rows().len().saturating_sub(1) / QUERY_PAGE_SIZE);
    }
    pub fn current_failure(&self) -> Option<&Failure> {
        self.failure.as_ref().or_else(|| {
            (self.requested == self.snapshot.operation_id)
                .then_some(self.snapshot.failure.as_ref())
                .flatten()
        })
    }
    pub fn can_guide(&self) -> bool {
        self.open
            && self.pending.is_none()
            && self.current_failure().is_some_and(|failure| {
                matches!(
                    failure.code,
                    ErrorCode::Permission | ErrorCode::Authentication
                )
            })
    }
    pub fn can_retry(&self) -> bool {
        self.open
            && self.pending.is_none()
            && self.failure.as_ref().is_some_and(|failure| {
                failure.retryable
                    || matches!(
                        failure.code,
                        ErrorCode::Permission | ErrorCode::Authentication
                    )
            })
    }
}
