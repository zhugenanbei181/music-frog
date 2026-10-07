//! An explicit simulation request and its independently readable, correlated result.
use crate::error::{ErrorCode, Failure};
use crate::rule_condition::RuleTraceIssue;
use crate::rule_tracer::{RuleTracerSnapshot, TrafficContextSnapshot};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTraceOperationId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTraceRequest {
    pub query: String,
    pub context: TrafficContextSnapshot,
}
impl RuleTraceRequest {
    pub fn validate(&self) -> Result<(), Failure> {
        let query = self.query.trim();
        if query.is_empty() || query.len() > 253 || query.chars().any(char::is_whitespace) {
            return Err(invalid("Enter a domain, IP address or host:port"));
        }
        if query
            .parse::<SocketAddr>()
            .is_ok_and(|address| address.port() == 0)
        {
            return Err(invalid("Destination port must be 1 through 65535"));
        }
        if self
            .context
            .network
            .as_deref()
            .is_some_and(|network| !matches!(network.to_ascii_lowercase().as_str(), "tcp" | "udp"))
        {
            return Err(invalid("Network must be tcp or udp"));
        }
        if self.context.dscp.is_some_and(|value| value > 63) {
            return Err(invalid("DSCP must be 0 through 63"));
        }
        if query.parse::<IpAddr>().is_err() && query.parse::<SocketAddr>().is_err() {
            let host = if let Some((host, port)) = query.rsplit_once(':') {
                let port = port
                    .parse::<u16>()
                    .map_err(|_| invalid("Invalid destination port"))?;
                if port == 0 {
                    return Err(invalid("Destination port must be 1 through 65535"));
                }
                host
            } else {
                query
            };
            let host = host.strip_suffix('.').unwrap_or(host);
            if host.is_empty()
                || !host.is_ascii()
                || host.split('.').any(|label| {
                    label.is_empty()
                        || label.len() > 63
                        || label.starts_with('-')
                        || label.ends_with('-')
                        || !label
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                })
            {
                return Err(invalid(
                    "Use an ASCII domain name, including punycode when needed",
                ));
            }
        }
        for ip in [
            &self.context.ip,
            &self.context.src_ip,
            &self.context.client_ip,
        ]
        .into_iter()
        .flatten()
        {
            ip.trim()
                .parse::<IpAddr>()
                .map_err(|_| invalid("Invalid sandbox IP address"))?;
        }
        if [
            self.context.port,
            self.context.src_port,
            self.context.in_port,
        ]
        .into_iter()
        .flatten()
        .any(|port| port == 0)
        {
            return Err(invalid("Sandbox ports must be 1 through 65535"));
        }
        Ok(())
    }
}
fn invalid(message: &str) -> Failure {
    Failure::new(ErrorCode::InvalidInput, message, false)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleTraceOperation {
    #[default]
    Idle,
    Running,
    Completed,
    Failed,
    Unsupported,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleTraceExecution {
    pub revision: u64,
    pub operation_id: Option<RuleTraceOperationId>,
    pub report_id: Option<RuleTraceOperationId>,
    pub operation: RuleTraceOperation,
    pub report: Option<RuleTracerSnapshot>,
    pub failure: Option<Failure>,
    #[serde(default)]
    pub issue: Option<RuleTraceIssue>,
}
