//! Mihomo GeoIP/GeoSite trigger adapter.

use crate::client::MihomoClient;
use crate::error::MihomoError;
use crate::network_error::network_error;
use async_trait::async_trait;
use infiltrator_contract::geo_database::GeoDatabaseTriggerReceipt;
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use infiltrator_ports::geo_database::GeoDatabasePort;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreScope, EngineId};
use lurker_core_ports::error::PortError;
use std::sync::Arc;

#[async_trait]
pub(crate) trait MihomoGeoDatabaseApi: Send + Sync {
    async fn trigger_geo_database_update(&self) -> Result<(), MihomoError>;
}

#[async_trait]
impl MihomoGeoDatabaseApi for MihomoClient {
    async fn trigger_geo_database_update(&self) -> Result<(), MihomoError> {
        MihomoClient::upgrade_geo(self).await
    }
}

pub struct MihomoGeoDatabaseAdapter {
    api: Arc<dyn MihomoGeoDatabaseApi>,
    scope_source: Arc<dyn CoreScopeSource>,
}

impl MihomoGeoDatabaseAdapter {
    pub fn new(client: Arc<MihomoClient>, scope_source: Arc<dyn CoreScopeSource>) -> Self {
        Self {
            api: client,
            scope_source,
        }
    }

    fn verify_scope(&self, requested: &CoreScope) -> Result<(), PortError> {
        match self.scope_source.scope_observation() {
            CoreScopeObservation::Observed(current) if &current == requested => {}
            CoreScopeObservation::Observed(_) => {
                return Err(rejected(
                    ErrorCode::InvalidState,
                    "active core scope changed",
                ));
            }
            CoreScopeObservation::Unavailable => {
                return Err(rejected(
                    ErrorCode::NotReady,
                    "no active core scope is available",
                ));
            }
        }
        if requested.identity.engine != EngineId::mihomo() {
            return Err(PortError::Rejected(Failure::unsupported(
                "Mihomo Geo database adapter received a non-Mihomo scope",
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl GeoDatabasePort for MihomoGeoDatabaseAdapter {
    async fn trigger(&self, scope: CoreScope) -> Result<GeoDatabaseTriggerReceipt, PortError> {
        self.verify_scope(&scope)?;
        self.api
            .trigger_geo_database_update()
            .await
            .map_err(network_error)?;
        self.verify_scope(&scope)?;
        Ok(GeoDatabaseTriggerReceipt::accepted(scope))
    }
}

fn rejected(code: ErrorCode, message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(code, message, false))
}

#[cfg(test)]
mod tests {
    use super::{MihomoGeoDatabaseAdapter, MihomoGeoDatabaseApi};
    use crate::error::MihomoError;
    use async_trait::async_trait;
    use infiltrator_contract::geo_database::GeoDatabaseTriggerState;
    use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
    use infiltrator_ports::geo_database::GeoDatabasePort;
    use lurker_core_contract::identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeApi(AtomicUsize);
    #[async_trait]
    impl MihomoGeoDatabaseApi for FakeApi {
        async fn trigger_geo_database_update(&self) -> Result<(), MihomoError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct ScopeSource(CoreScope);
    impl CoreScopeSource for ScopeSource {
        fn scope_observation(&self) -> CoreScopeObservation {
            CoreScopeObservation::Observed(self.0.clone())
        }
    }

    fn scope(engine: EngineId, instance: &str) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                engine,
                CoreInstanceId::new(instance).unwrap(),
                Some(CoreVersion::new("v1.19.32").unwrap()),
            ),
            generation: CoreGeneration::new(2),
            session_token: None,
        }
    }

    #[tokio::test]
    async fn geo_trigger_is_scope_bound_and_reports_acceptance_only() {
        let active = scope(EngineId::mihomo(), "core-a");
        let api = Arc::new(FakeApi(AtomicUsize::new(0)));
        let source: Arc<dyn CoreScopeSource> = Arc::new(ScopeSource(active.clone()));
        let adapter = MihomoGeoDatabaseAdapter {
            api: api.clone(),
            scope_source: source,
        };
        let receipt = adapter.trigger(active.clone()).await.unwrap();
        assert_eq!(receipt.scope, active);
        assert_eq!(receipt.state, GeoDatabaseTriggerState::Accepted);
        assert_eq!(api.0.load(Ordering::SeqCst), 1);

        let foreign = scope(EngineId::sing_box(), "core-b");
        assert!(adapter.trigger(foreign).await.is_err());
        assert_eq!(api.0.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn unavailable_scope_does_not_send_trigger() {
        struct Unavailable;
        impl CoreScopeSource for Unavailable {
            fn scope_observation(&self) -> CoreScopeObservation {
                CoreScopeObservation::Unavailable
            }
        }
        let active = scope(EngineId::mihomo(), "core-a");
        let api = Arc::new(FakeApi(AtomicUsize::new(0)));
        let source: Arc<dyn CoreScopeSource> = Arc::new(Unavailable);
        let adapter = MihomoGeoDatabaseAdapter {
            api: api.clone(),
            scope_source: source,
        };
        assert!(adapter.trigger(active).await.is_err());
        assert_eq!(api.0.load(Ordering::SeqCst), 0);
    }
}
