//! Core-scoped Geo database trigger use-case.

use crate::core_capability_application::{
    CoreCapabilityApplication, CoreCapabilityGateFailure, CoreCapabilityRequirement,
    CoreCommandEffect,
};
use infiltrator_contract::core_capabilities::{CoreOperation, GeoOperation};
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::geo_database::GeoDatabaseTriggerReceipt;
use infiltrator_ports::geo_database::GeoDatabasePort;
use std::sync::Arc;

const TRIGGER_GEO_DATABASES: CoreCapabilityRequirement = CoreCapabilityRequirement::new(
    CoreOperation::Geo(GeoOperation::UpdateDatabases),
    CoreCommandEffect::Write,
);

#[derive(Clone)]
pub struct GeoDatabaseApplication {
    port: Arc<dyn GeoDatabasePort>,
    capabilities: CoreCapabilityApplication,
}

impl GeoDatabaseApplication {
    pub fn new(port: Arc<dyn GeoDatabasePort>, capabilities: CoreCapabilityApplication) -> Self {
        Self { port, capabilities }
    }

    /// A successful receipt means the controller accepted the asynchronous
    /// trigger. It does not claim that database download or installation ended.
    pub async fn trigger(&self, scope: &CoreScope) -> Result<GeoDatabaseTriggerReceipt, Failure> {
        self.capabilities
            .authorize(Some(scope), TRIGGER_GEO_DATABASES)
            .map_err(CoreCapabilityGateFailure::into_failure)?;
        let receipt = self
            .port
            .trigger(scope.clone())
            .await
            .map_err(Failure::from)?;
        if receipt.scope != *scope {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "Geo database trigger receipt belongs to another Core scope",
                false,
            ));
        }
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::GeoDatabaseApplication;
    use crate::core_capability_application::CoreCapabilityApplication;
    use crate::core_capability_test_support::{capability_application, core_scope};
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };
    use infiltrator_contract::geo_database::GeoDatabaseTriggerReceipt;
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::geo_database::GeoDatabasePort;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakePort {
        calls: AtomicUsize,
        foreign_readback: bool,
    }
    #[async_trait::async_trait]
    impl GeoDatabasePort for FakePort {
        async fn trigger(&self, scope: CoreScope) -> Result<GeoDatabaseTriggerReceipt, PortError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.foreign_readback {
                let foreign = CoreScope {
                    identity: CoreIdentity::new(
                        EngineId::mihomo(),
                        CoreInstanceId::new("other").unwrap(),
                        Some(CoreVersion::new("1.19.12").unwrap()),
                    ),
                    generation: CoreGeneration::new(1),
                    session_token: None,
                };
                Ok(GeoDatabaseTriggerReceipt::accepted(foreign))
            } else {
                Ok(GeoDatabaseTriggerReceipt::accepted(scope))
            }
        }
    }

    #[tokio::test]
    async fn trigger_returns_scope_bound_acceptance_without_claiming_completion() {
        let scope = core_scope();
        let port = Arc::new(FakePort {
            calls: AtomicUsize::new(0),
            foreign_readback: false,
        });
        let app = GeoDatabaseApplication::new(port.clone(), capability_application());
        let receipt = app.trigger(&scope).await.unwrap();
        assert_eq!(receipt.scope, scope);
        assert_eq!(port.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn foreign_receipt_is_rejected_and_missing_capability_has_no_side_effect() {
        let scope = core_scope();
        let port = Arc::new(FakePort {
            calls: AtomicUsize::new(0),
            foreign_readback: true,
        });
        let app = GeoDatabaseApplication::new(port.clone(), capability_application());
        assert!(app.trigger(&scope).await.is_err());

        let no_capability = GeoDatabaseApplication::new(
            port.clone(),
            CoreCapabilityApplication::without_provider(),
        );
        assert!(no_capability.trigger(&scope).await.is_err());
        assert_eq!(port.calls.load(Ordering::SeqCst), 1);
    }
}
