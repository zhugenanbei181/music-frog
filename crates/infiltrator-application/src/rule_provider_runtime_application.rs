//! Shared rule-provider controller operations and owner/revision validation.

use crate::core_capability_application::{
    CoreCapabilityApplication, CoreCapabilityGateFailure, CoreCapabilityRequirement,
    CoreCommandEffect,
};
use infiltrator_contract::core_capabilities::{CoreOperation, ProviderOperation};
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_provider_runtime::{
    RuleProviderId, RuleProviderInventorySnapshot, RuleProviderPayloadSnapshot,
    RuleProviderRefreshReceipt, RuleProviderRevision,
};
use infiltrator_ports::rule_provider_runtime::RuleProviderPort;
use std::sync::Arc;

const LIST_RULE_PROVIDERS: CoreCapabilityRequirement = CoreCapabilityRequirement::new(
    CoreOperation::Provider(ProviderOperation::ListRule),
    CoreCommandEffect::Read,
);
const UPDATE_RULE_PROVIDER: CoreCapabilityRequirement = CoreCapabilityRequirement::new(
    CoreOperation::Provider(ProviderOperation::UpdateRule),
    CoreCommandEffect::Write,
);

#[derive(Clone)]
pub struct RuleProviderRuntimeApplication {
    port: Arc<dyn RuleProviderPort>,
    capabilities: CoreCapabilityApplication,
}

impl RuleProviderRuntimeApplication {
    pub fn new(port: Arc<dyn RuleProviderPort>, capabilities: CoreCapabilityApplication) -> Self {
        Self { port, capabilities }
    }

    pub async fn inventory(
        &self,
        scope: &CoreScope,
    ) -> Result<RuleProviderInventorySnapshot, Failure> {
        self.authorize(scope, LIST_RULE_PROVIDERS)?;
        let inventory = self
            .port
            .inventory(scope.clone())
            .await
            .map_err(Failure::from)?;
        if inventory.scope() != scope {
            return Err(invalid_scope(
                "rule-provider inventory belongs to another Core scope",
            ));
        }
        Ok(inventory)
    }

    pub async fn refresh(
        &self,
        scope: &CoreScope,
        expected_revision: RuleProviderRevision,
        provider_id: RuleProviderId,
    ) -> Result<RuleProviderRefreshReceipt, Failure> {
        self.authorize(scope, UPDATE_RULE_PROVIDER)?;
        if provider_id.engine_id() != &scope.identity.engine {
            return Err(invalid_scope(
                "rule-provider refresh belongs to another Core engine",
            ));
        }
        let receipt = self
            .port
            .refresh(scope.clone(), expected_revision, provider_id.clone())
            .await
            .map_err(Failure::from)?;
        if receipt.scope() != scope
            || receipt.requested_revision() != expected_revision
            || receipt.provider_id() != &provider_id
            || receipt.inventory().scope() != scope
            || receipt.inventory().revision() < expected_revision
            || receipt.inventory().provider(&provider_id).is_none()
        {
            return Err(invalid_scope(
                "rule-provider refresh receipt failed its owner or revision fence",
            ));
        }
        Ok(receipt)
    }

    pub async fn payload(
        &self,
        scope: &CoreScope,
        expected_revision: RuleProviderRevision,
        provider_id: RuleProviderId,
    ) -> Result<RuleProviderPayloadSnapshot, Failure> {
        self.authorize(scope, LIST_RULE_PROVIDERS)?;
        if provider_id.engine_id() != &scope.identity.engine {
            return Err(invalid_scope(
                "rule-provider payload belongs to another Core engine",
            ));
        }
        let snapshot = self
            .port
            .payload(scope.clone(), expected_revision, provider_id.clone())
            .await
            .map_err(Failure::from)?;
        if snapshot.scope() != scope
            || snapshot.inventory_revision() != expected_revision
            || snapshot.provider_id() != &provider_id
        {
            return Err(invalid_scope(
                "rule-provider payload failed its owner or revision fence",
            ));
        }
        Ok(snapshot)
    }

    /// Resolve a fresh inventory and read one provider payload using the exact
    /// revision that named it. The payload is still subject to the adapter's
    /// second read-before-use fence.
    pub async fn payload_by_name(
        &self,
        scope: &CoreScope,
        name: &str,
    ) -> Result<RuleProviderPayloadSnapshot, Failure> {
        let id = RuleProviderId::new(scope.identity.engine.clone(), name.to_owned())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let inventory = self.inventory(scope).await?;
        if inventory.provider(&id).is_none() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                format!("rule provider {name} is not present in the active Core"),
                false,
            ));
        }
        self.payload(scope, inventory.revision(), id).await
    }

    /// Refresh each provider from one observed inventory, advancing the
    /// expected revision from each same-scope readback before the next write.
    pub async fn refresh_all(
        &self,
        scope: &CoreScope,
    ) -> Result<Vec<RuleProviderRefreshReceipt>, Failure> {
        let mut inventory = self.inventory(scope).await?;
        let provider_ids: Vec<_> = inventory
            .providers()
            .iter()
            .map(|provider| provider.id.clone())
            .collect();
        let mut receipts = Vec::with_capacity(provider_ids.len());
        for provider_id in provider_ids {
            let receipt = self
                .refresh(scope, inventory.revision(), provider_id)
                .await?;
            inventory = receipt.inventory().clone();
            receipts.push(receipt);
        }
        Ok(receipts)
    }

    fn authorize(
        &self,
        scope: &CoreScope,
        requirement: CoreCapabilityRequirement,
    ) -> Result<(), Failure> {
        self.capabilities
            .authorize(Some(scope), requirement)
            .map(|_| ())
            .map_err(CoreCapabilityGateFailure::into_failure)
    }
}

fn invalid_scope(message: impl Into<String>) -> Failure {
    Failure::new(ErrorCode::InvalidState, message, false)
}

#[cfg(test)]
mod tests {
    use super::RuleProviderRuntimeApplication;
    use crate::core_capability_test_support::{capability_application, core_scope};
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
    };
    use infiltrator_contract::rule_provider_runtime::{
        RuleProviderFact, RuleProviderId, RuleProviderInventorySnapshot, RuleProviderPayload,
        RuleProviderPayloadSnapshot, RuleProviderRefreshReceipt, RuleProviderRevision,
    };
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::rule_provider_runtime::RuleProviderPort;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakePort {
        inventory: RuleProviderInventorySnapshot,
        calls: AtomicUsize,
        foreign_readback: bool,
    }

    #[async_trait::async_trait]
    impl RuleProviderPort for FakePort {
        async fn inventory(
            &self,
            _scope: CoreScope,
        ) -> Result<RuleProviderInventorySnapshot, PortError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.inventory.clone())
        }
        async fn refresh(
            &self,
            scope: CoreScope,
            revision: RuleProviderRevision,
            id: RuleProviderId,
        ) -> Result<RuleProviderRefreshReceipt, PortError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let readback = if self.foreign_readback {
                inventory(
                    foreign_scope(),
                    RuleProviderRevision::new(revision.value() + 1).unwrap(),
                )
            } else {
                inventory(
                    scope.clone(),
                    RuleProviderRevision::new(revision.value() + 1).unwrap(),
                )
            };
            RuleProviderRefreshReceipt::accepted(scope, revision, readback, id)
                .map_err(|error| PortError::Failed(error.to_string()))
        }
        async fn payload(
            &self,
            scope: CoreScope,
            revision: RuleProviderRevision,
            id: RuleProviderId,
        ) -> Result<RuleProviderPayloadSnapshot, PortError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            RuleProviderPayloadSnapshot::new(scope, revision, id, RuleProviderPayload::NotPublished)
                .map_err(|error| PortError::Failed(error.to_string()))
        }
    }

    fn foreign_scope() -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::mihomo(),
                CoreInstanceId::new("other").unwrap(),
                Some(infiltrator_contract::core_identity::CoreVersion::new("1.19.12").unwrap()),
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        }
    }

    fn inventory(
        scope: CoreScope,
        revision: RuleProviderRevision,
    ) -> RuleProviderInventorySnapshot {
        RuleProviderInventorySnapshot::new(
            scope,
            revision,
            vec![RuleProviderFact {
                id: RuleProviderId::new(EngineId::mihomo(), "rules").unwrap(),
                provider_type: "http".to_owned(),
                behavior: "domain".to_owned(),
                vehicle_type: "HTTP".to_owned(),
                updated_at: "now".to_owned(),
                rule_count: 1,
            }],
        )
        .unwrap()
    }

    fn app(port: Arc<FakePort>) -> RuleProviderRuntimeApplication {
        RuleProviderRuntimeApplication::new(port, capability_application())
    }

    #[tokio::test]
    async fn list_refresh_and_payload_preserve_scope_and_revision() {
        let scope = core_scope();
        let port = Arc::new(FakePort {
            inventory: inventory(scope.clone(), RuleProviderRevision::initial()),
            calls: AtomicUsize::new(0),
            foreign_readback: false,
        });
        let app = app(port.clone());
        let inventory = app.inventory(&scope).await.unwrap();
        let id = RuleProviderId::new(scope.identity.engine.clone(), "rules").unwrap();
        let receipt = app
            .refresh(&scope, inventory.revision(), id.clone())
            .await
            .unwrap();
        assert_eq!(receipt.inventory().revision().value(), 2);
        let payload = app.payload(&scope, inventory.revision(), id).await.unwrap();
        assert_eq!(payload.scope(), &scope);
        assert_eq!(port.calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn foreign_readback_is_rejected_by_the_application_use_case() {
        let scope = core_scope();
        let port = Arc::new(FakePort {
            inventory: inventory(scope.clone(), RuleProviderRevision::initial()),
            calls: AtomicUsize::new(0),
            foreign_readback: true,
        });
        let id = RuleProviderId::new(scope.identity.engine.clone(), "rules").unwrap();
        assert!(
            app(port)
                .refresh(&scope, RuleProviderRevision::initial(), id)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn absent_capability_refuses_before_port_dispatch() {
        let scope = core_scope();
        let port = Arc::new(FakePort {
            inventory: inventory(scope.clone(), RuleProviderRevision::initial()),
            calls: AtomicUsize::new(0),
            foreign_readback: false,
        });
        let app = RuleProviderRuntimeApplication::new(
            port.clone(),
            crate::core_capability_application::CoreCapabilityApplication::without_provider(),
        );
        assert!(app.inventory(&scope).await.is_err());
        assert_eq!(port.calls.load(Ordering::SeqCst), 0);
    }
}
