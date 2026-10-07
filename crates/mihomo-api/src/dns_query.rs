//! Exact v1.19.18 controller response decoding at the outbound boundary.
use crate::client::MihomoClient;
use crate::error::MihomoError;
use crate::runtime_gateway::network_error;
use infiltrator_contract::dns_query::{DnsQueryRequest, DnsQueryResponse, DnsQuestion, DnsRecord};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_ports::dns_query::DnsQueryPort;
use infiltrator_ports::error::PortError;
use serde::Deserialize;

#[derive(Deserialize)]
struct Question {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Qtype")]
    record_type: u16,
    #[serde(rename = "Qclass")]
    class: u16,
}
#[derive(Deserialize)]
struct Record {
    name: String,
    #[serde(rename = "type")]
    record_type: u16,
    #[serde(rename = "TTL")]
    ttl: u32,
    data: String,
}
#[derive(Deserialize)]
struct Response {
    #[serde(rename = "Status")]
    status: u16,
    #[serde(rename = "Question")]
    questions: Vec<Question>,
    #[serde(rename = "TC")]
    truncated: bool,
    #[serde(rename = "RD")]
    recursion_desired: bool,
    #[serde(rename = "RA")]
    recursion_available: bool,
    #[serde(rename = "AD")]
    authenticated_data: bool,
    #[serde(rename = "CD")]
    checking_disabled: bool,
    #[serde(rename = "Answer", default)]
    answers: Vec<Record>,
    #[serde(rename = "Authority", default)]
    authority: Vec<Record>,
    #[serde(rename = "Additional", default)]
    additional: Vec<Record>,
}
fn query_error(error: MihomoError) -> PortError {
    if matches!(&error, MihomoError::Json(_))
        || matches!(&error, MihomoError::Http(http) if http.is_decode())
    {
        PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            format!("Malformed controller DNS response: {error}"),
            false,
        ))
    } else {
        network_error(error)
    }
}
fn records(rows: Vec<Record>) -> Vec<DnsRecord> {
    rows.into_iter()
        .map(|row| DnsRecord {
            name: row.name,
            record_type: row.record_type,
            ttl: row.ttl,
            data: row.data,
        })
        .collect()
}
#[async_trait::async_trait]
impl DnsQueryPort for MihomoClient {
    async fn query(&self, request: &DnsQueryRequest) -> Result<DnsQueryResponse, PortError> {
        request.validate().map_err(PortError::Rejected)?;
        let value = self
            .get_dns_query(&request.name, request.record_type.wire())
            .await
            .map_err(query_error)?;
        let response: Response = serde_json::from_value(value).map_err(|error| {
            PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                format!("Malformed controller DNS response: {error}"),
                false,
            ))
        })?;
        let response = DnsQueryResponse {
            status: response.status,
            questions: response
                .questions
                .into_iter()
                .map(|question| DnsQuestion {
                    name: question.name,
                    record_type: question.record_type,
                    class: question.class,
                })
                .collect(),
            truncated: response.truncated,
            recursion_desired: response.recursion_desired,
            recursion_available: response.recursion_available,
            authenticated_data: response.authenticated_data,
            checking_disabled: response.checking_disabled,
            answers: records(response.answers),
            authority: records(response.authority),
            additional: records(response.additional),
        };
        response
            .validate_for(request)
            .map_err(PortError::Rejected)?;
        Ok(response)
    }
}

#[cfg(test)]
#[path = "dns_query_test.rs"]
mod tests;
