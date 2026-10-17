//! Engine-neutral outbound observation and selection port.

use crate::error::PortError;
use async_trait::async_trait;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::{
    details::OutboundDetailsSnapshot,
    identity::{OutboundId, SnapshotRevision},
    inventory::{OutboundInventorySnapshot, OutboundObservationSnapshot, OutboundProbeSnapshot},
};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutboundProbeRequestError {
    InvalidHttpUrl,
    CredentialsNotAllowed,
    ZeroTimeout,
}

impl Display for OutboundProbeRequestError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHttpUrl => {
                formatter.write_str("probe target must be an absolute HTTP(S) URL")
            }
            Self::CredentialsNotAllowed => {
                formatter.write_str("probe target must not contain credentials")
            }
            Self::ZeroTimeout => formatter.write_str("probe timeout must be greater than zero"),
        }
    }
}

impl Error for OutboundProbeRequestError {}

/// Engine-neutral HTTP target passed between an application port and adapter.
/// This is a port request value, not a cross-surface UI intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeTarget {
    url: String,
}

impl ProbeTarget {
    pub fn http_url(url: impl Into<String>) -> Result<Self, OutboundProbeRequestError> {
        let url = url.into();
        let parsed =
            url::Url::parse(&url).map_err(|_| OutboundProbeRequestError::InvalidHttpUrl)?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err(OutboundProbeRequestError::InvalidHttpUrl);
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(OutboundProbeRequestError::CredentialsNotAllowed);
        }
        Ok(Self {
            url: parsed.to_string(),
        })
    }

    pub fn as_url(&self) -> &str {
        &self.url
    }
}

/// Port-to-adapter probe input. Mihomo adapters apply their wire type's bounds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutboundProbeRequest {
    target: ProbeTarget,
    timeout_ms: u64,
}

impl OutboundProbeRequest {
    pub fn new(target: ProbeTarget, timeout_ms: u64) -> Result<Self, OutboundProbeRequestError> {
        if timeout_ms == 0 {
            return Err(OutboundProbeRequestError::ZeroTimeout);
        }
        Ok(Self { target, timeout_ms })
    }

    pub fn target(&self) -> &ProbeTarget {
        &self.target
    }

    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

/// Reads and changes outbound state for one observed core activation.
///
/// Adapters must compare `expected_revision` before selecting or probing and
/// return observations authored by the same active `CoreScope`. Wire DTOs and
/// engine-specific identifiers stay inside the adapter.
#[async_trait]
pub trait OutboundPort: Send + Sync {
    async fn inventory(&self, scope: CoreScope) -> Result<OutboundInventorySnapshot, PortError>;

    /// Read inventory and details from one adapter observation cycle where
    /// supported. The default composes narrow reads and validates their fence.
    async fn observation(
        &self,
        scope: CoreScope,
    ) -> Result<OutboundObservationSnapshot, PortError> {
        let inventory = self.inventory(scope.clone()).await?;
        let details = self.details(scope, inventory.revision()).await?;
        OutboundObservationSnapshot::new(inventory, details)
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    /// Read details for the same inventory revision observed by the caller.
    /// Implementations that do not expose neutral details fail closed.
    async fn details(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        let _ = (scope, expected_revision);
        Err(PortError::Rejected(Failure::unsupported(
            "outbound details are unsupported by this adapter",
        )))
    }

    async fn select(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        collection_id: OutboundId,
        outbound_id: OutboundId,
    ) -> Result<OutboundInventorySnapshot, PortError>;

    async fn probe(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        outbound_id: OutboundId,
        request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError>;
}

#[cfg(test)]
mod tests {
    use super::{OutboundProbeRequest, OutboundProbeRequestError, ProbeTarget};

    #[test]
    fn probe_target_accepts_only_credential_free_absolute_http_urls() {
        let target =
            ProbeTarget::http_url("https://example.test/health?q=1").expect("valid HTTP URL");
        assert_eq!(target.as_url(), "https://example.test/health?q=1");
        assert!(matches!(
            ProbeTarget::http_url("ftp://example.test/health"),
            Err(OutboundProbeRequestError::InvalidHttpUrl)
        ));
        assert!(matches!(
            ProbeTarget::http_url("https://user:pass@example.test/health"),
            Err(OutboundProbeRequestError::CredentialsNotAllowed)
        ));
    }

    #[test]
    fn probe_timeout_is_positive_and_keeps_generic_u64_range() {
        let target = ProbeTarget::http_url("https://example.test").expect("valid target");
        let request = OutboundProbeRequest::new(target.clone(), u64::from(u32::MAX) + 1)
            .expect("generic request does not inherit Mihomo's u32 wire bound");
        assert_eq!(request.timeout_ms(), u64::from(u32::MAX) + 1);
        assert!(matches!(
            OutboundProbeRequest::new(target, 0),
            Err(OutboundProbeRequestError::ZeroTimeout)
        ));
    }
}
