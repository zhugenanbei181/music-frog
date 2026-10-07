//! Native TEA events carry only shared draft choices and correlated command results.
use infiltrator_application::dns_query_actions::QuerySection;
use infiltrator_contract::dns_query::DnsRecordType;
use infiltrator_contract::error::Failure;
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
        token: u64,
        result: Result<(), Failure>,
    },
}
