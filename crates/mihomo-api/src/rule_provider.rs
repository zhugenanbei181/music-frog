//! Mihomo rule-provider adapter with owner and observation revision fences.

use crate::client::MihomoClient;
use crate::network_error::network_error;
use crate::types::RuleProvider;
use async_trait::async_trait;
use infiltrator_contract::core_identity::{CoreScope, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_provider_runtime::{
    RuleProviderFact, RuleProviderId, RuleProviderInventorySnapshot, RuleProviderPayload,
    RuleProviderPayloadSnapshot, RuleProviderRefreshReceipt, RuleProviderRevision,
};
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_provider_runtime::RuleProviderPort;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[async_trait]
pub(crate) trait MihomoRuleProviderApi: Send + Sync {
    async fn list_rule_providers(&self) -> crate::error::Result<HashMap<String, RuleProvider>>;
    async fn rule_provider_payload(&self, name: &str) -> crate::error::Result<Option<Vec<String>>>;
    async fn update_rule_provider(&self, name: &str) -> crate::error::Result<()>;
}

#[async_trait]
impl MihomoRuleProviderApi for MihomoClient {
    async fn list_rule_providers(&self) -> crate::error::Result<HashMap<String, RuleProvider>> {
        MihomoClient::get_rule_providers(self).await
    }

    async fn rule_provider_payload(&self, name: &str) -> crate::error::Result<Option<Vec<String>>> {
        MihomoClient::get_rule_provider_payload(self, name).await
    }

    async fn update_rule_provider(&self, name: &str) -> crate::error::Result<()> {
        MihomoClient::update_rule_provider(self, name).await
    }
}

pub struct MihomoRuleProviderAdapter {
    api: Arc<dyn MihomoRuleProviderApi>,
    scope_source: Arc<dyn CoreScopeSource>,
    observation: Mutex<RuleProviderObservation>,
}

impl MihomoRuleProviderAdapter {
    pub fn new(client: Arc<MihomoClient>, scope_source: Arc<dyn CoreScopeSource>) -> Self {
        Self::from_api(client, scope_source)
    }

    fn from_api(
        api: Arc<dyn MihomoRuleProviderApi>,
        scope_source: Arc<dyn CoreScopeSource>,
    ) -> Self {
        Self {
            api,
            scope_source,
            observation: Mutex::new(RuleProviderObservation::Unobserved),
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
                "Mihomo rule-provider adapter received a non-Mihomo scope",
            )));
        }
        Ok(())
    }

    async fn read_facts(&self) -> Result<Vec<RuleProviderFact>, PortError> {
        let providers = self
            .api
            .list_rule_providers()
            .await
            .map_err(network_error)?;
        let engine = EngineId::mihomo();
        let mut facts = Vec::with_capacity(providers.len());
        for (map_name, provider) in providers {
            if map_name != provider.name {
                return Err(rejected(
                    ErrorCode::InvalidState,
                    "Mihomo rule-provider key does not match its controller name",
                ));
            }
            let id = RuleProviderId::new(engine.clone(), provider.name.clone())
                .map_err(|error| rejected(ErrorCode::InvalidState, error.to_string()))?;
            facts.push(RuleProviderFact {
                id,
                provider_type: provider.provider_type,
                behavior: provider.behavior,
                vehicle_type: provider.vehicle_type,
                updated_at: provider.updated_at,
                rule_count: provider.rule_count,
            });
        }
        facts.sort_by(|left, right| left.id.name().cmp(right.id.name()));
        Ok(facts)
    }

    async fn observe_inventory(
        &self,
        scope: &CoreScope,
        observation: &mut RuleProviderObservation,
        facts: Vec<RuleProviderFact>,
    ) -> Result<RuleProviderInventorySnapshot, PortError> {
        let revision = observation.revision_for(scope, &facts)?;
        *observation = RuleProviderObservation::Observed {
            scope: scope.clone(),
            revision,
            facts: facts.clone(),
        };
        RuleProviderInventorySnapshot::new(scope.clone(), revision, facts)
            .map_err(|error| rejected(ErrorCode::InvalidState, error.to_string()))
    }

    async fn preflight(
        &self,
        scope: &CoreScope,
        expected_revision: RuleProviderRevision,
        provider_id: &RuleProviderId,
        observation: &mut RuleProviderObservation,
    ) -> Result<(), PortError> {
        self.verify_scope(scope)?;
        if provider_id.engine_id() != &scope.identity.engine {
            return Err(rejected(
                ErrorCode::InvalidState,
                "rule provider belongs to another engine",
            ));
        }
        let current_revision = match observation {
            RuleProviderObservation::Observed {
                scope: observed_scope,
                revision,
                ..
            } if observed_scope == scope => *revision,
            RuleProviderObservation::Unobserved | RuleProviderObservation::Observed { .. } => {
                return Err(stale_revision());
            }
        };
        if current_revision != expected_revision {
            return Err(stale_revision());
        }
        let facts = self.read_facts().await?;
        self.verify_scope(scope)?;
        let (observed_revision, observed_facts) = match observation {
            RuleProviderObservation::Observed {
                revision, facts, ..
            } => (*revision, facts.clone()),
            RuleProviderObservation::Unobserved => return Err(stale_revision()),
        };
        if facts != observed_facts {
            self.observe_inventory(scope, observation, facts).await?;
            return Err(stale_revision());
        }
        if observed_revision != expected_revision {
            return Err(stale_revision());
        }
        if !observed_facts.iter().any(|fact| &fact.id == provider_id) {
            return Err(PortError::NotFound(format!(
                "rule provider {} is not present in the active Core",
                provider_id.name()
            )));
        }
        Ok(())
    }
}

enum RuleProviderObservation {
    Unobserved,
    Observed {
        scope: CoreScope,
        revision: RuleProviderRevision,
        facts: Vec<RuleProviderFact>,
    },
}

impl RuleProviderObservation {
    fn revision_for(
        &self,
        scope: &CoreScope,
        facts: &[RuleProviderFact],
    ) -> Result<RuleProviderRevision, PortError> {
        match self {
            Self::Unobserved => Ok(RuleProviderRevision::initial()),
            Self::Observed {
                scope: previous_scope,
                ..
            } if previous_scope != scope => Ok(RuleProviderRevision::initial()),
            Self::Observed {
                revision,
                facts: previous_facts,
                ..
            } if previous_facts != facts => revision
                .next()
                .map_err(|error| PortError::Failed(error.to_string())),
            Self::Observed { revision, .. } => Ok(*revision),
        }
    }
}

#[async_trait]
impl RuleProviderPort for MihomoRuleProviderAdapter {
    async fn inventory(
        &self,
        scope: CoreScope,
    ) -> Result<RuleProviderInventorySnapshot, PortError> {
        self.verify_scope(&scope)?;
        let mut observation = self.observation.lock().await;
        let facts = self.read_facts().await?;
        self.verify_scope(&scope)?;
        self.observe_inventory(&scope, &mut observation, facts)
            .await
    }

    async fn refresh(
        &self,
        scope: CoreScope,
        expected_revision: RuleProviderRevision,
        provider_id: RuleProviderId,
    ) -> Result<RuleProviderRefreshReceipt, PortError> {
        self.verify_scope(&scope)?;
        if provider_id.engine_id() != &scope.identity.engine {
            return Err(rejected(
                ErrorCode::InvalidState,
                "rule provider belongs to another engine",
            ));
        }
        let mut observation = self.observation.lock().await;
        self.preflight(&scope, expected_revision, &provider_id, &mut observation)
            .await?;
        self.api
            .update_rule_provider(provider_id.name())
            .await
            .map_err(network_error)?;
        self.verify_scope(&scope)?;
        let facts = self.read_facts().await?;
        self.verify_scope(&scope)?;
        let inventory = self
            .observe_inventory(&scope, &mut observation, facts)
            .await?;
        RuleProviderRefreshReceipt::accepted(scope, expected_revision, inventory, provider_id)
            .map_err(|error| rejected(ErrorCode::InvalidState, error.to_string()))
    }

    async fn payload(
        &self,
        scope: CoreScope,
        expected_revision: RuleProviderRevision,
        provider_id: RuleProviderId,
    ) -> Result<RuleProviderPayloadSnapshot, PortError> {
        self.verify_scope(&scope)?;
        if provider_id.engine_id() != &scope.identity.engine {
            return Err(rejected(
                ErrorCode::InvalidState,
                "rule provider belongs to another engine",
            ));
        }
        let mut observation = self.observation.lock().await;
        self.preflight(&scope, expected_revision, &provider_id, &mut observation)
            .await?;
        let payload = self
            .api
            .rule_provider_payload(provider_id.name())
            .await
            .map_err(network_error)?;
        self.verify_scope(&scope)?;
        let payload = match payload.filter(|rules| !rules.is_empty()) {
            Some(rules) => RuleProviderPayload::Published(rules),
            None => RuleProviderPayload::NotPublished,
        };
        RuleProviderPayloadSnapshot::new(scope, expected_revision, provider_id, payload)
            .map_err(|error| rejected(ErrorCode::InvalidState, error.to_string()))
    }
}

fn rejected(code: ErrorCode, message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(code, message, false))
}

fn stale_revision() -> PortError {
    rejected(
        ErrorCode::InvalidState,
        "rule-provider inventory revision is stale",
    )
}

#[cfg(test)]
mod tests {
    use super::{MihomoRuleProviderAdapter, MihomoRuleProviderApi};
    use crate::error::Result;
    use crate::types::RuleProvider;
    use async_trait::async_trait;
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };
    use infiltrator_contract::rule_provider_runtime::{
        RuleProviderId, RuleProviderPayload, RuleProviderRevision,
    };
    use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::rule_provider_runtime::RuleProviderPort;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct MutableScope(Arc<Mutex<CoreScope>>);

    impl CoreScopeSource for MutableScope {
        fn scope_observation(&self) -> CoreScopeObservation {
            match self.0.lock() {
                Ok(scope) => CoreScopeObservation::Observed(scope.clone()),
                Err(_) => CoreScopeObservation::Unavailable,
            }
        }
    }

    struct FakeApi {
        providers: Mutex<HashMap<String, RuleProvider>>,
        lists: AtomicUsize,
        updates: AtomicUsize,
        payload: Option<Vec<String>>,
    }

    impl FakeApi {
        fn new() -> Self {
            Self {
                providers: Mutex::new(HashMap::from([(
                    "rules".to_owned(),
                    RuleProvider {
                        name: "rules".to_owned(),
                        provider_type: "http".to_owned(),
                        behavior: "domain".to_owned(),
                        vehicle_type: "HTTP".to_owned(),
                        updated_at: "before".to_owned(),
                        rule_count: 1,
                        payload: Vec::new(),
                    },
                )])),
                lists: AtomicUsize::new(0),
                updates: AtomicUsize::new(0),
                payload: None,
            }
        }

        fn mutate(&self) {
            if let Some(provider) = self.providers.lock().unwrap().get_mut("rules") {
                provider.rule_count += 1;
                provider.updated_at = "changed-outside-adapter".to_owned();
            }
        }
    }

    #[async_trait]
    impl MihomoRuleProviderApi for FakeApi {
        async fn list_rule_providers(&self) -> Result<HashMap<String, RuleProvider>> {
            self.lists.fetch_add(1, Ordering::SeqCst);
            Ok(self.providers.lock().unwrap().clone())
        }
        async fn rule_provider_payload(&self, _name: &str) -> Result<Option<Vec<String>>> {
            Ok(self.payload.clone())
        }
        async fn update_rule_provider(&self, name: &str) -> Result<()> {
            self.updates.fetch_add(1, Ordering::SeqCst);
            if let Some(provider) = self.providers.lock().unwrap().get_mut(name) {
                provider.rule_count += 1;
                provider.updated_at = "updated".to_owned();
            }
            Ok(())
        }
    }

    fn scope(instance: &str) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::mihomo(),
                CoreInstanceId::new(instance).unwrap(),
                Some(CoreVersion::new("v1.19.32").unwrap()),
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        }
    }

    fn adapter(api: Arc<FakeApi>, current: CoreScope) -> MihomoRuleProviderAdapter {
        let source: Arc<dyn CoreScopeSource> =
            Arc::new(MutableScope(Arc::new(Mutex::new(current))));
        MihomoRuleProviderAdapter::from_api(api, source)
    }

    #[tokio::test]
    async fn inventory_refresh_and_payload_use_scope_revision_and_readback() {
        let api = Arc::new(FakeApi::new());
        let scope = scope("core-a");
        let adapter = adapter(api.clone(), scope.clone());
        let first = adapter.inventory(scope.clone()).await.unwrap();
        let id = RuleProviderId::new(EngineId::mihomo(), "rules").unwrap();

        api.mutate();
        let stale = adapter
            .refresh(scope.clone(), first.revision(), id.clone())
            .await
            .unwrap_err();
        assert!(matches!(stale, PortError::Rejected(_)));
        assert_eq!(api.updates.load(Ordering::SeqCst), 0);

        let second = adapter.inventory(scope.clone()).await.unwrap();
        assert!(second.revision() > first.revision());
        let receipt = adapter
            .refresh(scope.clone(), second.revision(), id.clone())
            .await
            .unwrap();
        assert_eq!(receipt.scope(), &scope);
        assert_eq!(receipt.provider_id(), &id);
        assert_eq!(receipt.inventory().providers()[0].rule_count, 3);
        assert_eq!(api.updates.load(Ordering::SeqCst), 1);

        let payload = adapter
            .payload(scope, receipt.inventory().revision(), id)
            .await
            .unwrap();
        assert_eq!(payload.payload(), &RuleProviderPayload::NotPublished);
    }

    #[tokio::test]
    async fn foreign_scope_and_provider_are_rejected_before_controller_calls() {
        let api = Arc::new(FakeApi::new());
        let active = scope("core-a");
        let adapter = adapter(api.clone(), active.clone());
        let foreign = scope("core-b");
        assert!(adapter.inventory(foreign).await.is_err());
        let foreign_id = RuleProviderId::new(EngineId::sing_box(), "rules").unwrap();
        assert!(
            adapter
                .refresh(active, RuleProviderRevision::initial(), foreign_id)
                .await
                .is_err()
        );
        assert_eq!(api.lists.load(Ordering::SeqCst), 0);
        assert_eq!(api.updates.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unobserved_revision_cannot_dispatch_mutation() {
        let api = Arc::new(FakeApi::new());
        let scope = scope("core-a");
        let adapter = adapter(api.clone(), scope.clone());
        let id = RuleProviderId::new(EngineId::mihomo(), "rules").unwrap();
        assert!(
            adapter
                .refresh(scope, RuleProviderRevision::initial(), id)
                .await
                .is_err()
        );
        assert_eq!(api.updates.load(Ordering::SeqCst), 0);
    }
}
