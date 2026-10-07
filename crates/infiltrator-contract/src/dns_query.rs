//! Neutral DNS controller query identity, complete response and operation facts.
use crate::error::{ErrorCode, Failure};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsRecordType {
    #[default]
    A,
    Aaaa,
    Cname,
    Mx,
    Ns,
    Txt,
    Soa,
    Ptr,
    Srv,
    Caa,
    Https,
    Svcb,
    Any,
}
impl DnsRecordType {
    pub const ALL: [Self; 13] = [
        Self::A,
        Self::Aaaa,
        Self::Cname,
        Self::Mx,
        Self::Ns,
        Self::Txt,
        Self::Soa,
        Self::Ptr,
        Self::Srv,
        Self::Caa,
        Self::Https,
        Self::Svcb,
        Self::Any,
    ];
    pub const fn wire(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::Aaaa => "AAAA",
            Self::Cname => "CNAME",
            Self::Mx => "MX",
            Self::Ns => "NS",
            Self::Txt => "TXT",
            Self::Soa => "SOA",
            Self::Ptr => "PTR",
            Self::Srv => "SRV",
            Self::Caa => "CAA",
            Self::Https => "HTTPS",
            Self::Svcb => "SVCB",
            Self::Any => "ANY",
        }
    }
    pub const fn number(self) -> u16 {
        match self {
            Self::A => 1,
            Self::Ns => 2,
            Self::Cname => 5,
            Self::Soa => 6,
            Self::Ptr => 12,
            Self::Mx => 15,
            Self::Txt => 16,
            Self::Aaaa => 28,
            Self::Srv => 33,
            Self::Svcb => 64,
            Self::Https => 65,
            Self::Any => 255,
            Self::Caa => 257,
        }
    }
}
impl fmt::Display for DnsRecordType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.wire())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsQueryRequest {
    pub name: String,
    pub record_type: DnsRecordType,
}
impl DnsQueryRequest {
    pub fn validate(&self) -> Result<(), Failure> {
        let name = self.name.strip_suffix('.').unwrap_or(&self.name);
        if self.name == "." {
            return Ok(());
        }
        if name.is_empty()
            || name.len() > 253
            || !name.is_ascii()
            || name.split('.').any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || !label
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
            })
        {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "Enter a DNS name with nonempty ASCII labels (use punycode for international names)",
                false,
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsQuestion {
    pub name: String,
    pub record_type: u16,
    pub class: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsRecord {
    pub name: String,
    pub record_type: u16,
    pub ttl: u32,
    pub data: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsQueryResponse {
    pub status: u16,
    pub questions: Vec<DnsQuestion>,
    pub truncated: bool,
    pub recursion_desired: bool,
    pub recursion_available: bool,
    pub authenticated_data: bool,
    pub checking_disabled: bool,
    pub answers: Vec<DnsRecord>,
    pub authority: Vec<DnsRecord>,
    pub additional: Vec<DnsRecord>,
}
impl DnsQueryResponse {
    pub fn validate_for(&self, request: &DnsQueryRequest) -> Result<(), Failure> {
        if self.questions.len() != 1
            || !self.questions[0]
                .name
                .strip_suffix('.')
                .unwrap_or(&self.questions[0].name)
                .eq_ignore_ascii_case(request.name.strip_suffix('.').unwrap_or(&request.name))
            || self.questions[0].record_type != request.record_type.number()
            || self.questions[0].class != 1
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS response question does not match the submitted query",
                false,
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsQueryOperationId(pub u64);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsQueryOperation {
    #[default]
    Idle,
    Running,
    Completed,
    Failed,
    Unsupported,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsQueryReport {
    pub request: DnsQueryRequest,
    pub response: DnsQueryResponse,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsQuerySnapshot {
    pub revision: u64,
    pub operation_id: Option<DnsQueryOperationId>,
    pub report_id: Option<DnsQueryOperationId>,
    pub operation: DnsQueryOperation,
    pub report: Option<DnsQueryReport>,
    pub failure: Option<Failure>,
}
impl DnsQuerySnapshot {
    pub fn unavailable() -> Self {
        Self {
            operation: DnsQueryOperation::Unsupported,
            failure: Some(Failure::unsupported("DNS query service is unavailable")),
            ..Self::default()
        }
    }
}
