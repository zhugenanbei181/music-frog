//! Engine-neutral outbound selection and probe requests.
//!
//! Every request carries the exact revision the caller observed. The
//! application and the adapter can therefore refuse a request that was made
//! against a stale observation instead of applying it to a newer state.

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

/// The largest number of outbounds one probe batch may probe in parallel.
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
    /// Builds a selection request for one member of one collection.
    ///
    /// Fails with [`OutboundContractError::EngineScopeMismatch`] when the
    /// collection and the member belong to different engines.
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

    /// Returns the collection the request selects inside.
    pub fn collection_id(&self) -> &OutboundId {
        &self.collection_id
    }

    /// Returns the member the request selects.
    pub fn outbound_id(&self) -> &OutboundId {
        &self.outbound_id
    }

    /// Returns the inventory revision the request was based on.
    pub const fn expected_revision(&self) -> SnapshotRevision {
        self.expected_revision
    }

    /// Checks that the request belongs to the engine of `scope`.
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
    /// Builds a single-outbound probe request.
    ///
    /// Fails with [`OutboundContractError::ZeroProbeTimeout`] for a zero
    /// timeout, with [`OutboundContractError::InvalidProbeUrl`] when the test
    /// URL is not an absolute HTTP(S) URL, and with
    /// [`OutboundContractError::ProbeCredentialsNotAllowed`] when the URL
    /// carries embedded credentials.
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

    /// Returns the outbound to probe.
    pub fn outbound_id(&self) -> &OutboundId {
        &self.outbound_id
    }

    /// Returns the inventory revision the request was based on.
    pub const fn expected_inventory_revision(&self) -> SnapshotRevision {
        self.expected_inventory_revision
    }

    /// Returns the normalised test URL the probe uses.
    pub fn test_url(&self) -> &str {
        &self.test_url
    }

    /// Returns the probe timeout in milliseconds.
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Checks that the request belongs to the engine of `scope`.
    pub fn validate_scope(&self, scope: &CoreScope) -> Result<(), OutboundContractError> {
        if self.outbound_id.engine_id() == &scope.identity.engine {
            Ok(())
        } else {
            Err(OutboundContractError::EngineScopeMismatch)
        }
    }

    /// Whether the probe snapshot is the readback of this request.
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
    /// Probe exactly one outbound.
    One(OutboundId),
    /// Probe every observed member of one collection.
    Collection(OutboundId),
    /// Probe an explicit, ordered set of outbounds.
    Selected(Vec<OutboundId>),
    /// Probe every outbound of the observed inventory.
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

    /// Checks that every target of the scope belongs to the engine of `scope`.
    ///
    /// Fails with [`OutboundContractError::EngineScopeMismatch`] for a foreign
    /// engine, with [`OutboundContractError::EmptySelectedProbeBatch`] for an
    /// empty explicit set, and with
    /// [`OutboundContractError::DuplicateSelectedProbeTarget`] when the
    /// explicit set repeats an outbound.
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
///
/// The limit is validated once, at construction, so no adapter has to
/// re-check the bound before it starts the fan-out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct OutboundProbeConcurrency(u8);

impl OutboundProbeConcurrency {
    /// Builds a concurrency limit.
    ///
    /// Fails with [`OutboundContractError::InvalidProbeBatchConcurrency`] when
    /// the limit is zero or above
    /// [`MAX_OUTBOUND_PROBE_BATCH_CONCURRENCY`].
    pub fn new(limit: u8) -> Result<Self, OutboundContractError> {
        if !(1..=MAX_OUTBOUND_PROBE_BATCH_CONCURRENCY).contains(&limit) {
            return Err(OutboundContractError::InvalidProbeBatchConcurrency);
        }
        Ok(Self(limit))
    }

    /// Returns the limit as the fan-out width an adapter should use.
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
    /// Builds a batch probe request.
    ///
    /// Fails with [`OutboundContractError::ZeroProbeTimeout`] for a zero
    /// timeout, with [`OutboundContractError::InvalidProbeUrl`] for a test URL
    /// that is not an absolute HTTP(S) URL, with
    /// [`OutboundContractError::ProbeCredentialsNotAllowed`] for a URL with
    /// embedded credentials, and with the target-specific errors of
    /// [`OutboundProbeBatchScope::validate_scope`].
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

    /// Returns the scope the batch probes.
    pub fn target_scope(&self) -> &OutboundProbeBatchScope {
        &self.target_scope
    }

    /// Returns the inventory revision the request was based on.
    pub const fn expected_inventory_revision(&self) -> SnapshotRevision {
        self.expected_inventory_revision
    }

    /// Returns the normalised test URL the batch uses.
    pub fn test_url(&self) -> &str {
        &self.test_url
    }

    /// Returns the per-target timeout in milliseconds.
    pub const fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Returns the fan-out limit of the batch.
    pub const fn concurrency(&self) -> OutboundProbeConcurrency {
        self.concurrency
    }

    /// Checks that every target of the batch belongs to the engine of `scope`.
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
    /// The outbound this result belongs to.
    pub outbound_id: OutboundId,
    /// The terminal outcome of the probe.
    pub outcome: OutboundProbeBatchOutcome,
}

/// The terminal outcome of probing one batch member.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundProbeBatchOutcome {
    /// The probe finished and measured a delay.
    Measured {
        /// The measured round-trip delay in milliseconds.
        delay_ms: u32,
    },
    /// The probe did not answer within its deadline.
    TimedOut,
    /// The backend cannot probe this outbound.
    Unsupported,
    /// The probe failed.
    Failed {
        /// The typed reason the probe failed.
        failure: Failure,
    },
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
    /// Builds a batch snapshot and validates it against its scope and request.
    ///
    /// Fails with [`OutboundContractError::EngineScopeMismatch`] when a result
    /// belongs to another engine, with
    /// [`OutboundContractError::DuplicateOutboundProbeResultId`] when two
    /// results share an outbound, and with
    /// [`OutboundContractError::ProbeBatchResultMismatch`] when the result set
    /// does not correspond to the requested scope.
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

    /// Returns the core scope this snapshot is fenced to.
    pub fn scope(&self) -> &CoreScope {
        &self.scope
    }

    /// Returns the request this snapshot answers.
    pub fn request(&self) -> &ProbeOutboundBatchRequest {
        &self.request
    }

    /// Returns the detail revision observed after the batch completed.
    pub const fn detail_revision(&self) -> SnapshotRevision {
        self.detail_revision
    }

    /// Returns the terminal result of every probed member.
    pub fn items(&self) -> &[OutboundProbeBatchItem] {
        &self.items
    }

    /// Whether this snapshot answers exactly `request`.
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

/// Validates a probe target URL and returns its normalised form.
///
/// Only absolute HTTP(S) URLs without embedded credentials are accepted, so a
/// probe can never leak a secret through the target it is given.
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
