//! Native TEA events carry only shared draft choices and correlated command results.
use infiltrator_contract::dns_query::{DnsQueryOperationId, DnsRecordType};
use infiltrator_contract::error::Failure;
use infiltrator_presentation::dns_query_actions::QuerySection;
#[derive(Clone, Debug)]
pub enum QueryAction {
    Open,
    Cancel,
    Name(String),
    RecordType(DnsRecordType),
    Section(QuerySection),
    Previous,
    Next,
    Run,
    Retry,
    Finished {
        token: DnsQueryOperationId,
        result: Result<(), Failure>,
    },
}
