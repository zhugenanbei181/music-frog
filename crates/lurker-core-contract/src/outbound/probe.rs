use super::identity::{OutboundContractError, OutboundId, SnapshotRevision};
use super::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeSnapshot,
    OutboundSelectionState,
};
use crate::error::Failure;
use crate::identity::CoreScope;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

pub const MAX_OUTBOUND_PROBE_BATCH_CONCURRENCY: u8 = 30;

/// A user request to select one member from an observed selection collection.
///
/// The expected revision is required so the application and adapter can reject
/// a request made against an older inventory observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundSelectionRequest {
    collection_id: OutboundId,
    outbound_id: OutboundId,
    expected_revision: SnapshotRevision,
}

impl OutboundSelectionRequest {
    pub fn new(
        collection_id: OutboundId,
        outbound_id: OutboundId,
        expected_revision: SnapshotRevision,
    ) -> Result<Self, OutboundContractError> {
        if collection_id.engine_id() != outbound_id.engine_id() {
            return Err(OutboundContractError::EngineScopeMismatch);
        }
        Ok(Self {
            collection_id,
            outbound_id,
            expected_revision,
        })
    }

    pub fn collection_id(&self) -> &OutboundId {
        &self.collection_id
    }

    pub fn outbound_id(&self) -> &OutboundId {
        &self.outbound_id
    }

    pub const fn expected_revision(&self) -> SnapshotRevision {
        self.expected_revision
    }

    pub fn validate_scope(&self, scope: &CoreScope) -> Result<(), OutboundContractError> {
        if self.collection_id.engine_id() == &scope.identity.engine
            && self.outbound_id.engine_id() == &scope.identity.engine
        {
            Ok(())
        } else {
            Err(OutboundContractError::EngineScopeMismatch)
        }
    }

    /// Whether this observation is a valid readback for the requested choice.
    pub fn is_confirmed_by(&self, snapshot: &OutboundInventorySnapshot) -> bool {
        if snapshot.revision() < self.expected_revision
            || snapshot.scope().identity.engine != *self.collection_id.engine_id()
        {
            return false;
        }
        let OutboundInventoryState::Ready { collections, .. } = snapshot.inventory() else {
            return false;
        };
        collections.iter().any(|collection| {
            collection.id == self.collection_id
                && collection.member_ids.contains(&self.outbound_id)
                && matches!(
                    &collection.selection,
                    OutboundSelectionState::Ready { selected }
                        if selected == &self.outbound_id
                )
        })
    }
}

/// Cross-surface request to probe one outbound from the exact inventory
/// revision observed by the caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProbeOutboundRequest {
    outbound_id: OutboundId,
    expected_inventory_revision: SnapshotRevision,
    test_url: String,
    timeout_ms: u64,
}

impl ProbeOutboundRequest {
    pub fn new(
        outbound_id: OutboundId,
        expected_inventory_revision: SnapshotRevision,
        test_url: impl Into<String>,
        timeout_ms: u64,
    ) -> Result<Self, OutboundContractError> {
        if timeout_ms == 0 {
            return Err(OutboundContractError::ZeroProbeTimeout);
        }
        Ok(Self {
            outbound_id,
            expected_inventory_revision,
            test_url: validate_probe_url(test_url)?,
            timeout_ms,
        })
    }

    pub fn outbound_id(&self) -> &OutboundId {
        &self.outbound_id
    }

    pub const fn expected_inventory_revision(&self) -> SnapshotRevision {
        self.expected_inventory_revision
    }

    pub fn test_url(&self) -> &str {
        &self.test_url
    }

    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    pub fn validate_scope(&self, scope: &CoreScope) -> Result<(), OutboundContractError> {
        if self.outbound_id.engine_id() == &scope.identity.engine {
            Ok(())
        } else {
            Err(OutboundContractError::EngineScopeMismatch)
        }
    }

    pub fn is_confirmed_by(&self, snapshot: &OutboundProbeSnapshot) -> bool {
        snapshot.outbound_id() == &self.outbound_id
            && snapshot.outbound_id().engine_id() == self.outbound_id.engine_id()
            && snapshot.observed_inventory_revision() == self.expected_inventory_revision
    }
}

/// Exhaustive neutral scope for a batch probe. Collection targets expand to
/// their observed members; Selected preserves an explicit ordered set; All
/// expands to every observed inventory entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundProbeBatchScope {
    One(OutboundId),
    Collection(OutboundId),
    Selected(Vec<OutboundId>),
    All,
}

impl OutboundProbeBatchScope {
    fn validate_targets(&self) -> Result<(), OutboundContractError> {
        if let Self::Selected(ids) = self {
            if ids.is_empty() {
                return Err(OutboundContractError::EmptySelectedProbeBatch);
            }
            let unique = ids.iter().collect::<HashSet<_>>();
            if unique.len() != ids.len() {
                return Err(OutboundContractError::DuplicateSelectedProbeTarget);
            }
        }
        Ok(())
    }

    pub fn validate_scope(&self, scope: &CoreScope) -> Result<(), OutboundContractError> {
        self.validate_targets()?;
        let outbound_id = match self {
            Self::One(id) | Self::Collection(id) => Some(id),
            Self::Selected(_) => None,
            Self::All => None,
        };
        let selected_engines_match = match self {
            Self::Selected(ids) => ids
                .iter()
                .all(|id| id.engine_id() == &scope.identity.engine),
            _ => true,
        };
        if selected_engines_match
            && outbound_id.is_none_or(|id| id.engine_id() == &scope.identity.engine)
        {
            Ok(())
        } else {
            Err(OutboundContractError::EngineScopeMismatch)
        }
    }
}

/// Bounded fan-out limit for one outbound probe batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundProbeConcurrency(u8);

impl OutboundProbeConcurrency {
    pub fn new(limit: u8) -> Result<Self, OutboundContractError> {
        if !(1..=MAX_OUTBOUND_PROBE_BATCH_CONCURRENCY).contains(&limit) {
            return Err(OutboundContractError::InvalidProbeBatchConcurrency);
        }
        Ok(Self(limit))
    }

    pub const fn limit(self) -> usize {
        self.0 as usize
    }
}

impl<'de> Deserialize<'de> for OutboundProbeConcurrency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u8::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// Cross-surface request to probe an observed outbound set from one exact
/// inventory revision. URL, timeout, and concurrency are validated at
/// construction and remain private so deserialization uses the same checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProbeOutboundBatchRequest {
    target_scope: OutboundProbeBatchScope,
    expected_inventory_revision: SnapshotRevision,
    test_url: String,
    timeout_ms: u64,
    concurrency: OutboundProbeConcurrency,
}

impl ProbeOutboundBatchRequest {
    pub fn new(
        target_scope: OutboundProbeBatchScope,
        expected_inventory_revision: SnapshotRevision,
        test_url: impl Into<String>,
        timeout_ms: u64,
        concurrency: OutboundProbeConcurrency,
    ) -> Result<Self, OutboundContractError> {
        if timeout_ms == 0 {
            return Err(OutboundContractError::ZeroProbeTimeout);
        }
        target_scope.validate_targets()?;
        Ok(Self {
            target_scope,
            expected_inventory_revision,
            test_url: validate_probe_url(test_url)?,
            timeout_ms,
            concurrency,
        })
    }

    pub fn target_scope(&self) -> &OutboundProbeBatchScope {
        &self.target_scope
    }

    pub const fn expected_inventory_revision(&self) -> SnapshotRevision {
        self.expected_inventory_revision
    }

    pub fn test_url(&self) -> &str {
        &self.test_url
    }

    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    pub const fn concurrency(&self) -> OutboundProbeConcurrency {
        self.concurrency
    }

    pub fn validate_scope(&self, scope: &CoreScope) -> Result<(), OutboundContractError> {
        self.target_scope.validate_scope(scope)
    }
}

#[derive(Deserialize)]
struct ProbeOutboundBatchRequestFields {
    target_scope: OutboundProbeBatchScope,
    expected_inventory_revision: SnapshotRevision,
    test_url: String,
    timeout_ms: u64,
    concurrency: OutboundProbeConcurrency,
}

impl<'de> Deserialize<'de> for ProbeOutboundBatchRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = ProbeOutboundBatchRequestFields::deserialize(deserializer)?;
        Self::new(
            fields.target_scope,
            fields.expected_inventory_revision,
            fields.test_url,
            fields.timeout_ms,
            fields.concurrency,
        )
        .map_err(D::Error::custom)
    }
}

/// One terminal result for a member of an outbound probe batch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundProbeBatchItem {
    pub outbound_id: OutboundId,
    pub outcome: OutboundProbeBatchOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundProbeBatchOutcome {
    Measured { delay_ms: u32 },
    TimedOut,
    Unsupported,
    Failed { failure: Failure },
}

/// Immutable, request-correlated results for a complete batch probe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundProbeBatchSnapshot {
    scope: CoreScope,
    request: ProbeOutboundBatchRequest,
    detail_revision: SnapshotRevision,
    items: Vec<OutboundProbeBatchItem>,
}

impl OutboundProbeBatchSnapshot {
    pub fn new(
        scope: CoreScope,
        request: ProbeOutboundBatchRequest,
        detail_revision: SnapshotRevision,
        items: Vec<OutboundProbeBatchItem>,
    ) -> Result<Self, OutboundContractError> {
        request.validate_scope(&scope)?;
        let mut ids = HashSet::new();
        for item in &items {
            if item.outbound_id.engine_id() != &scope.identity.engine {
                return Err(OutboundContractError::EngineScopeMismatch);
            }
            if !ids.insert(&item.outbound_id) {
                return Err(OutboundContractError::DuplicateOutboundProbeResultId);
            }
        }
        match request.target_scope() {
            OutboundProbeBatchScope::One(outbound_id)
                if items.len() != 1 || items[0].outbound_id != *outbound_id =>
            {
                return Err(OutboundContractError::ProbeBatchResultMismatch);
            }
            OutboundProbeBatchScope::Selected(outbound_ids)
                if items.len() != outbound_ids.len()
                    || items
                        .iter()
                        .zip(outbound_ids)
                        .any(|(item, outbound_id)| &item.outbound_id != outbound_id) =>
            {
                return Err(OutboundContractError::ProbeBatchResultMismatch);
            }
            _ => {}
        }
        Ok(Self {
            scope,
            request,
            detail_revision,
            items,
        })
    }

    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

    pub fn request(&self) -> &ProbeOutboundBatchRequest {
        &self.request
    }

    pub const fn detail_revision(&self) -> SnapshotRevision {
        self.detail_revision
    }

    pub fn items(&self) -> &[OutboundProbeBatchItem] {
        &self.items
    }

    pub fn is_confirmed_by(&self, request: &ProbeOutboundBatchRequest) -> bool {
        &self.request == request
    }
}

#[derive(Deserialize)]
struct OutboundProbeBatchSnapshotFields {
    scope: CoreScope,
    request: ProbeOutboundBatchRequest,
    detail_revision: SnapshotRevision,
    items: Vec<OutboundProbeBatchItem>,
}

impl<'de> Deserialize<'de> for OutboundProbeBatchSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = OutboundProbeBatchSnapshotFields::deserialize(deserializer)?;
        Self::new(
            fields.scope,
            fields.request,
            fields.detail_revision,
            fields.items,
        )
        .map_err(D::Error::custom)
    }
}

fn validate_probe_url(url: impl Into<String>) -> Result<String, OutboundContractError> {
    let parsed =
        url::Url::parse(&url.into()).map_err(|_| OutboundContractError::InvalidProbeUrl)?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(OutboundContractError::InvalidProbeUrl);
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(OutboundContractError::ProbeCredentialsNotAllowed);
    }
    Ok(parsed.to_string())
}

#[derive(Deserialize)]
struct ProbeOutboundRequestFields {
    outbound_id: OutboundId,
    expected_inventory_revision: SnapshotRevision,
    test_url: String,
    timeout_ms: u64,
}

impl<'de> Deserialize<'de> for ProbeOutboundRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = ProbeOutboundRequestFields::deserialize(deserializer)?;
        Self::new(
            fields.outbound_id,
            fields.expected_inventory_revision,
            fields.test_url,
            fields.timeout_ms,
        )
        .map_err(D::Error::custom)
    }
}

#[derive(Deserialize)]
struct OutboundSelectionRequestFields {
    collection_id: OutboundId,
    outbound_id: OutboundId,
    expected_revision: SnapshotRevision,
}

impl<'de> Deserialize<'de> for OutboundSelectionRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = OutboundSelectionRequestFields::deserialize(deserializer)?;
        Self::new(
            fields.collection_id,
            fields.outbound_id,
            fields.expected_revision,
        )
        .map_err(D::Error::custom)
    }
}
