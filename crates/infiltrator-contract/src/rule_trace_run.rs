//! An explicit simulation request and its independently readable, correlated result.
use crate::profile_document_identity::ProfileWorkspaceIdentity;
use crate::rule_condition::RuleTraceIssue;
use crate::rule_tracer::{RuleTracerSnapshot, TrafficContextSnapshot};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::inventory::OutboundObservationSnapshot;
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

/// The owner fence for one simulation. It captures the active Core activation
/// and the exact typed profile workspace whose rules were observed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTraceOwnerBinding {
    pub scope: CoreScope,
    pub workspace: ProfileWorkspaceIdentity,
}

impl RuleTraceOwnerBinding {
    pub fn new(scope: CoreScope, workspace: ProfileWorkspaceIdentity) -> Result<Self, Failure> {
        let binding = Self { scope, workspace };
        binding.validate_engine()?;
        Ok(binding)
    }

    pub fn validate_engine(&self) -> Result<(), Failure> {
        if self.workspace.document.engine_id != self.scope.identity.engine {
            return Err(Failure::unsupported(
                "rule trace profile belongs to another engine",
            ));
        }
        Ok(())
    }

    pub fn validate_observation(
        &self,
        observed_scope: &CoreScope,
        observed_workspace: &ProfileWorkspaceIdentity,
    ) -> Result<(), Failure> {
        self.validate_engine()?;
        if &self.scope != observed_scope {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Core scope changed; refresh before simulating rules",
                true,
            ));
        }
        if &self.workspace != observed_workspace {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Profile workspace changed; refresh before simulating rules",
                true,
            ));
        }
        if observed_workspace.document.engine_id != observed_scope.identity.engine {
            return Err(Failure::unsupported(
                "observed profile belongs to another engine",
            ));
        }
        Ok(())
    }
}

/// Application input after the command adapter binds user input to the
/// validated Core owner and current typed profile workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedRuleTraceRequest {
    pub owner: RuleTraceOwnerBinding,
    pub request: RuleTraceRequest,
}

/// Facts available to the local AST trace about the active runtime's outbounds.
/// A live session must carry an adapter observation; only an explicitly
/// unavailable observation can select the offline projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum RuleTraceOutboundFacts {
    Observed(Box<OutboundObservationSnapshot>),
    Unavailable(Failure),
}

impl RuleTraceOutboundFacts {
    pub fn observed(observation: OutboundObservationSnapshot) -> Self {
        Self::Observed(Box::new(observation))
    }
}

impl ScopedRuleTraceRequest {
    pub fn new(owner: RuleTraceOwnerBinding, request: RuleTraceRequest) -> Result<Self, Failure> {
        owner.validate_engine()?;
        request.validate()?;
        Ok(Self { owner, request })
    }

    pub fn validate_observation(
        &self,
        observed_scope: &CoreScope,
        observed_workspace: &ProfileWorkspaceIdentity,
    ) -> Result<(), Failure> {
        self.owner
            .validate_observation(observed_scope, observed_workspace)?;
        self.request.validate()
    }
}

/// Explicit owner state in the shared read model. Idle and rejected runs do
/// not synthesize an owner binding.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleTraceOwnerObservation {
    #[default]
    Unbound,
    Observed(Box<RuleTraceOwnerBinding>),
}

impl RuleTraceOwnerObservation {
    pub fn observed(owner: RuleTraceOwnerBinding) -> Self {
        Self::Observed(Box::new(owner))
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
    #[serde(default)]
    pub owner: RuleTraceOwnerObservation,
}
