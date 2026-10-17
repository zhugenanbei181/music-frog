//! Narrow owner- and revision-fenced controller operations for rule providers.

use async_trait::async_trait;
use infiltrator_contract::rule_provider_runtime::{
    RuleProviderId, RuleProviderInventorySnapshot, RuleProviderPayloadSnapshot,
    RuleProviderRefreshReceipt, RuleProviderRevision,
};
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::CoreScope;
use lurker_core_ports::error::PortError;

#[async_trait]
pub trait RuleProviderPort: Send + Sync {
    async fn inventory(&self, scope: CoreScope)
    -> Result<RuleProviderInventorySnapshot, PortError>;

    async fn refresh(
        &self,
        scope: CoreScope,
        expected_revision: RuleProviderRevision,
        provider_id: RuleProviderId,
    ) -> Result<RuleProviderRefreshReceipt, PortError>;

    async fn payload(
        &self,
        scope: CoreScope,
        expected_revision: RuleProviderRevision,
        provider_id: RuleProviderId,
    ) -> Result<RuleProviderPayloadSnapshot, PortError>;
}

pub struct UnsupportedRuleProviderPort {
    reason: String,
}

impl UnsupportedRuleProviderPort {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[async_trait]
impl RuleProviderPort for UnsupportedRuleProviderPort {
    async fn inventory(
        &self,
        _scope: CoreScope,
    ) -> Result<RuleProviderInventorySnapshot, PortError> {
        Err(unsupported(&self.reason))
    }

    async fn refresh(
        &self,
        _scope: CoreScope,
        _expected_revision: RuleProviderRevision,
        _provider_id: RuleProviderId,
    ) -> Result<RuleProviderRefreshReceipt, PortError> {
        Err(unsupported(&self.reason))
    }

    async fn payload(
        &self,
        _scope: CoreScope,
        _expected_revision: RuleProviderRevision,
        _provider_id: RuleProviderId,
    ) -> Result<RuleProviderPayloadSnapshot, PortError> {
        Err(unsupported(&self.reason))
    }
}

fn unsupported(reason: &str) -> PortError {
    PortError::Rejected(Failure::unsupported(reason))
}
