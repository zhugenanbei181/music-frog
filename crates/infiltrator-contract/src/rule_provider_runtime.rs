//! Owner- and revision-fenced observations of controller rule providers.

use lurker_core_contract::identity::{CoreScope, EngineId};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

const MAX_RULE_PROVIDER_NAME_BYTES: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleProviderContractError {
    EmptyName,
    OversizedName,
    ZeroRevision,
    ForeignEngine,
    DuplicateProvider,
    MissingRefreshedProvider,
}

impl Display for RuleProviderContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => formatter.write_str("rule provider name must not be empty"),
            Self::OversizedName => formatter.write_str("rule provider name exceeds 512 bytes"),
            Self::ZeroRevision => formatter.write_str("rule provider revision must be positive"),
            Self::ForeignEngine => {
                formatter.write_str("rule provider belongs to another core engine")
            }
            Self::DuplicateProvider => {
                formatter.write_str("rule provider inventory contains a duplicate identity")
            }
            Self::MissingRefreshedProvider => {
                formatter.write_str("refresh readback does not contain the requested provider")
            }
        }
    }
}

impl Error for RuleProviderContractError {}

/// Opaque rule-provider identity inside one engine namespace.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct RuleProviderId {
    engine_id: EngineId,
    name: String,
}

impl RuleProviderId {
    pub fn new(
        engine_id: EngineId,
        name: impl Into<String>,
    ) -> Result<Self, RuleProviderContractError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(RuleProviderContractError::EmptyName);
        }
        if name.len() > MAX_RULE_PROVIDER_NAME_BYTES {
            return Err(RuleProviderContractError::OversizedName);
        }
        Ok(Self { engine_id, name })
    }

    pub fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Deserialize)]
struct RuleProviderIdFields {
    engine_id: EngineId,
    name: String,
}

impl<'de> Deserialize<'de> for RuleProviderId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = RuleProviderIdFields::deserialize(deserializer)?;
        Self::new(fields.engine_id, fields.name).map_err(D::Error::custom)
    }
}

/// Revision of one observed rule-provider inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct RuleProviderRevision(u64);

impl RuleProviderRevision {
    pub const fn initial() -> Self {
        Self(1)
    }
    pub fn new(value: u64) -> Result<Self, RuleProviderContractError> {
        if value == 0 {
            return Err(RuleProviderContractError::ZeroRevision);
        }
        Ok(Self(value))
    }
    pub const fn value(self) -> u64 {
        self.0
    }
    pub fn next(self) -> Result<Self, RuleProviderContractError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(RuleProviderContractError::ZeroRevision)
    }
}

impl<'de> Deserialize<'de> for RuleProviderRevision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u64::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// Controller-authored facts for one rule provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleProviderFact {
    pub id: RuleProviderId,
    pub provider_type: String,
    pub behavior: String,
    pub vehicle_type: String,
    pub updated_at: String,
    pub rule_count: u32,
}

/// A deterministic inventory bound to the exact active Core scope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuleProviderInventorySnapshot {
    scope: CoreScope,
    revision: RuleProviderRevision,
    providers: Vec<RuleProviderFact>,
}

impl RuleProviderInventorySnapshot {
    pub fn new(
        scope: CoreScope,
        revision: RuleProviderRevision,
        mut providers: Vec<RuleProviderFact>,
    ) -> Result<Self, RuleProviderContractError> {
        let mut identities = HashSet::with_capacity(providers.len());
        for provider in &providers {
            if provider.id.engine_id() != &scope.identity.engine {
                return Err(RuleProviderContractError::ForeignEngine);
            }
            if !identities.insert(provider.id.clone()) {
                return Err(RuleProviderContractError::DuplicateProvider);
            }
        }
        providers.sort_by(|left, right| left.id.name().cmp(right.id.name()));
        Ok(Self {
            scope,
            revision,
            providers,
        })
    }
    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }
    pub const fn revision(&self) -> RuleProviderRevision {
        self.revision
    }
    pub fn providers(&self) -> &[RuleProviderFact] {
        &self.providers
    }
    pub fn provider(&self, id: &RuleProviderId) -> Option<&RuleProviderFact> {
        self.providers.iter().find(|provider| &provider.id == id)
    }
}

#[derive(Deserialize)]
struct RuleProviderInventorySnapshotFields {
    scope: CoreScope,
    revision: RuleProviderRevision,
    providers: Vec<RuleProviderFact>,
}

impl<'de> Deserialize<'de> for RuleProviderInventorySnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = RuleProviderInventorySnapshotFields::deserialize(deserializer)?;
        Self::new(fields.scope, fields.revision, fields.providers).map_err(D::Error::custom)
    }
}

/// Whether the controller actually publishes rule payload data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "rules", rename_all = "snake_case")]
pub enum RuleProviderPayload {
    Published(Vec<String>),
    NotPublished,
}

/// Payload readback fenced to the inventory revision that named the provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuleProviderPayloadSnapshot {
    scope: CoreScope,
    inventory_revision: RuleProviderRevision,
    provider_id: RuleProviderId,
    payload: RuleProviderPayload,
}

impl RuleProviderPayloadSnapshot {
    pub fn new(
        scope: CoreScope,
        inventory_revision: RuleProviderRevision,
        provider_id: RuleProviderId,
        payload: RuleProviderPayload,
    ) -> Result<Self, RuleProviderContractError> {
        if provider_id.engine_id() != &scope.identity.engine {
            return Err(RuleProviderContractError::ForeignEngine);
        }
        Ok(Self {
            scope,
            inventory_revision,
            provider_id,
            payload,
        })
    }
    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }
    pub const fn inventory_revision(&self) -> RuleProviderRevision {
        self.inventory_revision
    }
    pub fn provider_id(&self) -> &RuleProviderId {
        &self.provider_id
    }
    pub fn payload(&self) -> &RuleProviderPayload {
        &self.payload
    }
}

#[derive(Deserialize)]
struct RuleProviderPayloadSnapshotFields {
    scope: CoreScope,
    inventory_revision: RuleProviderRevision,
    provider_id: RuleProviderId,
    payload: RuleProviderPayload,
}

impl<'de> Deserialize<'de> for RuleProviderPayloadSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = RuleProviderPayloadSnapshotFields::deserialize(deserializer)?;
        Self::new(
            fields.scope,
            fields.inventory_revision,
            fields.provider_id,
            fields.payload,
        )
        .map_err(D::Error::custom)
    }
}

/// Accepted controller refresh plus same-scope list readback. It does not
/// claim that an asynchronous download has completed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuleProviderRefreshReceipt {
    scope: CoreScope,
    requested_revision: RuleProviderRevision,
    inventory: RuleProviderInventorySnapshot,
    provider_id: RuleProviderId,
}

impl RuleProviderRefreshReceipt {
    pub fn accepted(
        scope: CoreScope,
        requested_revision: RuleProviderRevision,
        inventory: RuleProviderInventorySnapshot,
        provider_id: RuleProviderId,
    ) -> Result<Self, RuleProviderContractError> {
        if inventory.scope() != &scope || provider_id.engine_id() != &scope.identity.engine {
            return Err(RuleProviderContractError::ForeignEngine);
        }
        if inventory.provider(&provider_id).is_none() {
            return Err(RuleProviderContractError::MissingRefreshedProvider);
        }
        Ok(Self {
            scope,
            requested_revision,
            inventory,
            provider_id,
        })
    }
    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }
    pub const fn requested_revision(&self) -> RuleProviderRevision {
        self.requested_revision
    }
    pub fn inventory(&self) -> &RuleProviderInventorySnapshot {
        &self.inventory
    }
    pub fn provider_id(&self) -> &RuleProviderId {
        &self.provider_id
    }
}

#[derive(Deserialize)]
struct RuleProviderRefreshReceiptFields {
    scope: CoreScope,
    requested_revision: RuleProviderRevision,
    inventory: RuleProviderInventorySnapshot,
    provider_id: RuleProviderId,
}

impl<'de> Deserialize<'de> for RuleProviderRefreshReceipt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = RuleProviderRefreshReceiptFields::deserialize(deserializer)?;
        Self::accepted(
            fields.scope,
            fields.requested_revision,
            fields.inventory,
            fields.provider_id,
        )
        .map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RuleProviderContractError, RuleProviderFact, RuleProviderId, RuleProviderInventorySnapshot,
        RuleProviderRevision,
    };
    use lurker_core_contract::identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };

    fn scope(engine: &str) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new(engine).unwrap(),
                CoreInstanceId::new("core-a").unwrap(),
                Some(CoreVersion::new("v1.19.32").unwrap()),
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        }
    }
    fn fact(engine: &str, name: &str) -> RuleProviderFact {
        RuleProviderFact {
            id: RuleProviderId::new(EngineId::new(engine).unwrap(), name).unwrap(),
            provider_type: "http".to_owned(),
            behavior: "domain".to_owned(),
            vehicle_type: "HTTP".to_owned(),
            updated_at: "2026-10-14T00:00:00Z".to_owned(),
            rule_count: 1,
        }
    }

    #[test]
    fn inventory_is_sorted_and_rejects_foreign_or_duplicate_provider_ids() {
        let inventory = RuleProviderInventorySnapshot::new(
            scope("mihomo"),
            RuleProviderRevision::initial(),
            vec![fact("mihomo", "z"), fact("mihomo", "a")],
        )
        .unwrap();
        assert_eq!(inventory.providers()[0].id.name(), "a");
        assert!(matches!(
            RuleProviderInventorySnapshot::new(
                scope("mihomo"),
                RuleProviderRevision::initial(),
                vec![fact("sing-box", "foreign")]
            ),
            Err(RuleProviderContractError::ForeignEngine)
        ));
        assert!(matches!(
            RuleProviderInventorySnapshot::new(
                scope("mihomo"),
                RuleProviderRevision::initial(),
                vec![fact("mihomo", "same"), fact("mihomo", "same")]
            ),
            Err(RuleProviderContractError::DuplicateProvider)
        ));
    }

    #[test]
    fn revisions_are_positive_and_advance_without_wrapping() {
        assert!(RuleProviderRevision::new(0).is_err());
        assert_eq!(RuleProviderRevision::initial().value(), 1);
        assert_eq!(
            RuleProviderRevision::new(8)
                .unwrap()
                .next()
                .unwrap()
                .value(),
            9
        );
        assert!(RuleProviderRevision::new(u64::MAX).unwrap().next().is_err());
    }
}
