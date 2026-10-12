//! Strict, engine-neutral contract for adapter operation declarations.
//!
//! A parsed declaration is descriptive only. The validated wrapper is the
//! only type that can answer whether an operation has verified evidence, and
//! it can do so only after checking completeness and the engine evidence lock.

use crate::core_capabilities::{
    AdapterRevision, CoreOperation, CoreVersionRequirement, EngineBinaryTarget,
    EngineCapabilityDecision,
};
use crate::core_identity::{CoreVersion, EngineId};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

const DECLARATION_SCHEMA_VERSION: u32 = 1;

/// One engine's complete operation support declaration.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterOperationDeclarationFile {
    pub schema_version: u32,
    pub engine: EngineId,
    pub operations: Vec<AdapterOperationDeclaration>,
}

/// A typed operation and its explicit evidence state.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterOperationDeclaration {
    pub operation: CoreOperation,
    pub support: AdapterOperationSupport,
}

/// Ownership for operations provided outside an engine's controller API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterOperationOwner {
    CoreLifecycle,
    VersionManagement,
}

/// An explicit adapter declaration state. `Supported` carries engine evidence;
/// `OwnerManaged` names a shared host/application owner outside the engine API.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdapterOperationSupport {
    Supported { evidence_id: OperationEvidenceId },
    OwnerManaged { owner: AdapterOperationOwner },
    EvidenceOpen { reason: String },
    Unsupported { reason: String },
}

/// A registered evidence identifier from an engine/version/target/revision lock.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct OperationEvidenceId(String);

impl OperationEvidenceId {
    pub fn new(value: impl Into<String>) -> Result<Self, AdapterOperationDeclarationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AdapterOperationDeclarationError::EmptyEvidenceId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for OperationEvidenceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// Evidence IDs registered across the locked assets and versions of one engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationEvidenceLock {
    engine: EngineId,
    assets: Vec<OperationEvidenceAsset>,
}

/// A single locked engine binary whose operation evidence is scoped to its
/// release, concrete target, and adapter implementation revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationEvidenceAsset {
    pub version: CoreVersion,
    pub target: EngineBinaryTarget,
    pub adapter_revision: AdapterRevision,
    pub operations: HashSet<OperationEvidenceId>,
}

impl OperationEvidenceLock {
    /// Parse the shared target-scoped evidence manifest used by engine
    /// adapters. Keeping this mapping in the contract prevents each provider
    /// from reinterpreting version, target, operation, and revision fields.
    pub fn from_manifest_json(source: &str, expected_engine: &str) -> Result<Self, String> {
        let manifest: serde_json::Value = serde_json::from_str(source)
            .map_err(|error| format!("evidence manifest JSON: {error}"))?;
        if manifest
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            != Some(2)
        {
            return Err("evidence manifest must use target-scoped schema version 2".to_owned());
        }
        if manifest.get("engine").and_then(serde_json::Value::as_str) != Some(expected_engine) {
            return Err(format!(
                "evidence manifest engine must be {expected_engine}"
            ));
        }
        let assets = manifest
            .get("assets")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "evidence manifest has no assets array".to_owned())?;
        let evidence_assets = assets
            .iter()
            .map(|asset| {
                let version = asset
                    .get("version")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "evidence asset has no pinned version".to_owned())?;
                let operations = asset
                    .get("operations")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| format!("evidence asset {version} has no operations array"))?
                    .iter()
                    .map(|operation| {
                        let value = operation.as_str().ok_or_else(|| {
                            format!("evidence asset {version} has a non-string operation")
                        })?;
                        OperationEvidenceId::new(value.to_owned())
                            .map_err(|error| error.to_string())
                    })
                    .collect::<Result<HashSet<_>, String>>()?;
                let target_name = asset
                    .get("binary_target")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| format!("evidence asset {version} has no binary target"))?;
                let target =
                    EngineBinaryTarget::from_target_triple(target_name).ok_or_else(|| {
                        format!("evidence asset {version} has unknown target {target_name}")
                    })?;
                let adapter_revision = asset
                    .get("adapter_revision")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| format!("evidence asset {version} has no adapter revision"))?;
                Ok(OperationEvidenceAsset {
                    version: CoreVersion::new(version.to_owned())
                        .map_err(|error| error.to_string())?,
                    target,
                    adapter_revision: AdapterRevision::new(adapter_revision),
                    operations,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let engine = EngineId::new(expected_engine).map_err(|error| error.to_string())?;
        Self::new(engine, evidence_assets).map_err(|error| error.to_string())
    }

    pub fn new(
        engine: EngineId,
        assets: impl IntoIterator<Item = OperationEvidenceAsset>,
    ) -> Result<Self, AdapterOperationDeclarationError> {
        let assets: Vec<_> = assets.into_iter().collect();
        if assets.is_empty() {
            return Err(AdapterOperationDeclarationError::EmptyEvidenceLockAssets);
        }
        let mut scopes = HashSet::with_capacity(assets.len());
        for asset in &assets {
            if asset.adapter_revision.value() == 0 {
                return Err(
                    AdapterOperationDeclarationError::InvalidEvidenceAdapterRevision {
                        version: asset.version.raw().to_owned(),
                        target: asset.target,
                    },
                );
            }
            if asset.target == EngineBinaryTarget::Unregistered {
                return Err(
                    AdapterOperationDeclarationError::UnregisteredEvidenceTarget {
                        version: asset.version.raw().to_owned(),
                    },
                );
            }
            if !scopes.insert((asset.version.clone(), asset.target, asset.adapter_revision)) {
                return Err(
                    AdapterOperationDeclarationError::DuplicateEvidenceAssetScope {
                        version: asset.version.raw().to_owned(),
                        target: asset.target,
                        adapter_revision: asset.adapter_revision.value(),
                    },
                );
            }
        }
        Ok(Self { engine, assets })
    }

    pub fn engine(&self) -> &EngineId {
        &self.engine
    }

    pub fn versions(&self) -> impl Iterator<Item = &CoreVersion> {
        self.assets.iter().map(|asset| &asset.version)
    }

    pub fn contains(&self, evidence_id: &OperationEvidenceId) -> bool {
        self.assets
            .iter()
            .any(|asset| asset.operations.contains(evidence_id))
    }

    pub fn contains_for_scope(
        &self,
        evidence_id: &OperationEvidenceId,
        version: &CoreVersion,
        target: EngineBinaryTarget,
        adapter_revision: AdapterRevision,
    ) -> bool {
        self.assets.iter().any(|asset| {
            &asset.version == version
                && asset.target == target
                && asset.adapter_revision == adapter_revision
                && asset.operations.contains(evidence_id)
        })
    }

    pub fn contains_for_target(
        &self,
        evidence_id: &OperationEvidenceId,
        target: EngineBinaryTarget,
        adapter_revision: AdapterRevision,
    ) -> bool {
        self.assets.iter().any(|asset| {
            asset.target == target
                && asset.adapter_revision == adapter_revision
                && asset.operations.contains(evidence_id)
        })
    }

    pub fn contains_target_at_revision(
        &self,
        target: EngineBinaryTarget,
        adapter_revision: AdapterRevision,
    ) -> bool {
        self.assets
            .iter()
            .any(|asset| asset.target == target && asset.adapter_revision == adapter_revision)
    }
}

/// A declaration whose operation set and supported evidence IDs are registered
/// against the engine-neutral contract. Runtime availability still requires
/// an exact target/revision match and, for active-engine operations, an exact
/// version/target/revision asset match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedAdapterOperationDeclarations {
    engine: EngineId,
    operations: HashMap<CoreOperation, AdapterOperationSupport>,
    evidence_lock: OperationEvidenceLock,
}

impl ValidatedAdapterOperationDeclarations {
    pub fn validate(
        declaration: AdapterOperationDeclarationFile,
        expected_engine: &str,
        evidence_lock: &OperationEvidenceLock,
    ) -> Result<Self, AdapterOperationDeclarationError> {
        if declaration.schema_version != DECLARATION_SCHEMA_VERSION {
            return Err(AdapterOperationDeclarationError::UnsupportedSchemaVersion(
                declaration.schema_version,
            ));
        }
        if declaration.engine.as_str() != expected_engine {
            return Err(AdapterOperationDeclarationError::EngineMismatch {
                expected: expected_engine.to_owned(),
                actual: declaration.engine.as_str().to_owned(),
            });
        }
        if declaration.engine != evidence_lock.engine {
            return Err(
                AdapterOperationDeclarationError::EvidenceLockEngineMismatch {
                    declaration_engine: declaration.engine.as_str().to_owned(),
                    lock_engine: evidence_lock.engine.as_str().to_owned(),
                },
            );
        }

        let expected: HashSet<_> = CoreOperation::all().iter().copied().collect();
        let mut operations = HashMap::with_capacity(declaration.operations.len());

        for item in declaration.operations {
            if !expected.contains(&item.operation) {
                return Err(AdapterOperationDeclarationError::OperationNotInContract(
                    item.operation,
                ));
            }
            if operations.contains_key(&item.operation) {
                return Err(AdapterOperationDeclarationError::DuplicateOperation(
                    item.operation,
                ));
            }

            validate_support(item.operation, &item.support, evidence_lock)?;
            operations.insert(item.operation, item.support);
        }

        let missing = CoreOperation::all()
            .iter()
            .copied()
            .find(|operation| !operations.contains_key(operation));
        if let Some(operation) = missing {
            return Err(AdapterOperationDeclarationError::MissingOperation(
                operation,
            ));
        }

        Ok(Self {
            engine: declaration.engine,
            operations,
            evidence_lock: evidence_lock.clone(),
        })
    }

    pub fn engine(&self) -> &EngineId {
        &self.engine
    }

    pub fn support(&self, operation: CoreOperation) -> &AdapterOperationSupport {
        self.operations
            .get(&operation)
            .expect("validated declaration covers every CoreOperation")
    }

    /// Resolve engine support from the same operation declaration and
    /// target/version evidence lock for every product host. Host integration
    /// and concrete adapter wiring remain separate capability dimensions.
    pub fn engine_decision_for_scope(
        &self,
        operation: CoreOperation,
        active_version: Option<&CoreVersion>,
        binary_target: EngineBinaryTarget,
        adapter_revision: AdapterRevision,
    ) -> EngineCapabilityDecision {
        match self.support(operation) {
            AdapterOperationSupport::Unsupported { reason } => {
                EngineCapabilityDecision::Unsupported {
                    reason: reason.clone(),
                }
            }
            AdapterOperationSupport::OwnerManaged { owner } => {
                EngineCapabilityDecision::OwnerManaged { owner: *owner }
            }
            AdapterOperationSupport::EvidenceOpen { reason } => {
                let scope = active_version.map_or_else(
                    || {
                        format!(
                            "target {binary_target:?} at adapter revision {}",
                            adapter_revision.value()
                        )
                    },
                    |version| {
                        format!(
                            "exact version `{}` / target {binary_target:?} / adapter revision {}",
                            version.raw(),
                            adapter_revision.value()
                        )
                    },
                );
                EngineCapabilityDecision::VersionUnavailable {
                    reason: format!("operation evidence remains open for {scope}: {reason}"),
                }
            }
            AdapterOperationSupport::Supported { .. } => match operation.version_requirement() {
                CoreVersionRequirement::IndependentOfActiveVersion => {
                    if self.has_verified_support_for_target(
                        operation,
                        binary_target,
                        adapter_revision,
                    ) {
                        EngineCapabilityDecision::Supported
                    } else {
                        EngineCapabilityDecision::VersionUnavailable {
                            reason: format!(
                                "no operation evidence is registered for {operation:?} at target {binary_target:?} and adapter revision {}",
                                adapter_revision.value()
                            ),
                        }
                    }
                }
                CoreVersionRequirement::RequiresActiveVersion => {
                    let Some(active_version) = active_version else {
                        return EngineCapabilityDecision::VersionUnavailable {
                            reason: "active core version is unavailable".to_owned(),
                        };
                    };
                    if self.has_verified_support_for_scope(
                        operation,
                        active_version,
                        binary_target,
                        adapter_revision,
                    ) {
                        EngineCapabilityDecision::Supported
                    } else {
                        EngineCapabilityDecision::VersionUnavailable {
                            reason: format!(
                                "no behavior evidence is registered for {operation:?} at exact version `{}`",
                                active_version.raw()
                            ),
                        }
                    }
                }
            },
        }
    }

    /// Returns evidence only for an operation that passed declaration and lock validation.
    /// `OwnerManaged`, `EvidenceOpen`, and `Unsupported` always return `None`.
    pub fn verified_supported_evidence(
        &self,
        operation: CoreOperation,
    ) -> Option<&OperationEvidenceId> {
        match self.support(operation) {
            AdapterOperationSupport::Supported { evidence_id } => Some(evidence_id),
            AdapterOperationSupport::OwnerManaged { .. }
            | AdapterOperationSupport::EvidenceOpen { .. }
            | AdapterOperationSupport::Unsupported { .. } => None,
        }
    }

    /// Returns true only when this operation has evidence for the exact active
    /// engine version, binary target, and adapter implementation revision.
    pub fn has_verified_support_for_scope(
        &self,
        operation: CoreOperation,
        version: &CoreVersion,
        target: EngineBinaryTarget,
        adapter_revision: AdapterRevision,
    ) -> bool {
        self.verified_supported_evidence(operation)
            .is_some_and(|evidence_id| {
                self.evidence_lock.contains_for_scope(
                    evidence_id,
                    version,
                    target,
                    adapter_revision,
                )
            })
    }

    /// Returns true only when this operation has evidence for the exact
    /// binary target and adapter revision, regardless of the active version.
    /// This is reserved for operations such as installing a candidate engine
    /// version before any version is active.
    pub fn has_verified_support_for_target(
        &self,
        operation: CoreOperation,
        target: EngineBinaryTarget,
        adapter_revision: AdapterRevision,
    ) -> bool {
        self.verified_supported_evidence(operation)
            .is_some_and(|evidence_id| {
                self.evidence_lock
                    .contains_for_target(evidence_id, target, adapter_revision)
            })
    }

    pub fn has_registered_target(
        &self,
        target: EngineBinaryTarget,
        adapter_revision: AdapterRevision,
    ) -> bool {
        self.evidence_lock
            .contains_target_at_revision(target, adapter_revision)
    }
}

fn validate_support(
    operation: CoreOperation,
    support: &AdapterOperationSupport,
    evidence_lock: &OperationEvidenceLock,
) -> Result<(), AdapterOperationDeclarationError> {
    match support {
        AdapterOperationSupport::Supported { evidence_id } => {
            let expected = canonical_evidence_id(operation);
            if evidence_id.as_str() != expected {
                return Err(
                    AdapterOperationDeclarationError::EvidenceIdOperationMismatch {
                        operation,
                        expected: expected.to_owned(),
                        actual: evidence_id.as_str().to_owned(),
                    },
                );
            }
            if !evidence_lock.contains(evidence_id) {
                return Err(AdapterOperationDeclarationError::EvidenceNotRegistered {
                    operation,
                    evidence_id: evidence_id.as_str().to_owned(),
                });
            }
        }
        AdapterOperationSupport::OwnerManaged { owner } => {
            let expected = operation.owner_managed_owner().ok_or(
                AdapterOperationDeclarationError::OwnerManagedOperationNotAllowed(operation),
            )?;
            if *owner != expected {
                return Err(
                    AdapterOperationDeclarationError::OwnerManagedOperationMismatch {
                        operation,
                        expected,
                        actual: *owner,
                    },
                );
            }
        }
        AdapterOperationSupport::EvidenceOpen { reason }
        | AdapterOperationSupport::Unsupported { reason }
            if reason.trim().is_empty() =>
        {
            return Err(AdapterOperationDeclarationError::EmptyReason(operation));
        }
        AdapterOperationSupport::EvidenceOpen { .. }
        | AdapterOperationSupport::Unsupported { .. } => {}
    }
    Ok(())
}

fn canonical_evidence_id(operation: CoreOperation) -> &'static str {
    use crate::core_capabilities::{
        ConnectionOperation, CoreGarbageCollectionOperation, CoreLifecycleOperation, DnsOperation,
        FakeIpCacheOperation, GeoOperation, LogsOperation, OutboundOperation, ProfileOperation,
        ProviderOperation, ProxyControlOperation, RulesOperation, SubscriptionOperation,
        TunOperation, VersionOperation,
    };

    match operation {
        CoreOperation::Profile(ProfileOperation::Validate) => "profile.validate",
        CoreOperation::Profile(ProfileOperation::Apply) => "profile.apply",
        CoreOperation::Profile(ProfileOperation::ApplyFeature) => "profile.apply_feature",
        CoreOperation::Dns(DnsOperation::ReadConfiguration) => "dns.read_configuration",
        CoreOperation::Dns(DnsOperation::ApplyConfiguration) => "dns.apply_configuration",
        CoreOperation::Dns(DnsOperation::ReadHosts) => "dns.read_hosts",
        CoreOperation::Dns(DnsOperation::WriteHosts) => "dns.write_hosts",
        CoreOperation::Dns(DnsOperation::Query) => "dns.query",
        CoreOperation::Dns(DnsOperation::FlushCache) => "dns.flush_cache",
        CoreOperation::Rules(RulesOperation::Trace) => "rules.trace",
        CoreOperation::Rules(RulesOperation::Apply) => "rules.apply",
        CoreOperation::Subscription(SubscriptionOperation::Refresh) => "subscription.refresh",
        CoreOperation::Subscription(SubscriptionOperation::Import) => "subscription.import",
        CoreOperation::Subscription(SubscriptionOperation::Update) => "subscription.update",
        CoreOperation::Provider(ProviderOperation::ListProxy) => "provider.list_proxy",
        CoreOperation::Provider(ProviderOperation::UpdateProxy) => "provider.update_proxy",
        CoreOperation::Provider(ProviderOperation::HealthcheckProxy) => {
            "provider.healthcheck_proxy"
        }
        CoreOperation::Provider(ProviderOperation::ListRule) => "provider.list_rule",
        CoreOperation::Provider(ProviderOperation::UpdateRule) => "provider.update_rule",
        CoreOperation::Outbound(OutboundOperation::List) => "outbound.list",
        CoreOperation::Outbound(OutboundOperation::Select) => "outbound.select",
        CoreOperation::Outbound(OutboundOperation::Probe) => "outbound.probe",
        CoreOperation::Connection(ConnectionOperation::List) => "connection.list",
        CoreOperation::Connection(ConnectionOperation::Close) => "connection.close",
        CoreOperation::Logs(LogsOperation::Read) => "logs.read",
        CoreOperation::Logs(LogsOperation::Stream) => "logs.stream",
        CoreOperation::Logs(LogsOperation::SetLevel) => "logs.set_level",
        CoreOperation::Geo(GeoOperation::UpdateDatabases) => "geo.update_databases",
        CoreOperation::FakeIpCache(FakeIpCacheOperation::Read) => "fake_ip_cache.read",
        CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush) => "fake_ip_cache.flush",
        CoreOperation::Tun(TunOperation::Inspect) => "tun.inspect",
        CoreOperation::Tun(TunOperation::Configure) => "tun.configure",
        CoreOperation::Tun(TunOperation::Start) => "tun.start",
        CoreOperation::Tun(TunOperation::Stop) => "tun.stop",
        CoreOperation::Lifecycle(CoreLifecycleOperation::Start) => "lifecycle.start",
        CoreOperation::Lifecycle(CoreLifecycleOperation::Stop) => "lifecycle.stop",
        CoreOperation::Lifecycle(CoreLifecycleOperation::Restart) => "lifecycle.restart",
        CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent) => {
            "routing.get_intent"
        }
        CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent) => {
            "routing.set_intent"
        }
        CoreOperation::CoreGarbageCollection(CoreGarbageCollectionOperation::Trigger) => {
            "core_garbage_collection.trigger"
        }
        CoreOperation::Version(VersionOperation::Inspect) => "version.inspect",
        CoreOperation::Version(VersionOperation::Install) => "version.install",
        CoreOperation::Version(VersionOperation::Activate) => "version.activate",
        CoreOperation::Version(VersionOperation::Uninstall) => "version.uninstall",
        CoreOperation::Version(VersionOperation::Rollback) => "version.rollback",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdapterOperationDeclarationError {
    EmptyEvidenceId,
    EmptyEvidenceLockAssets,
    InvalidEvidenceAdapterRevision {
        version: String,
        target: EngineBinaryTarget,
    },
    UnregisteredEvidenceTarget {
        version: String,
    },
    DuplicateEvidenceAssetScope {
        version: String,
        target: EngineBinaryTarget,
        adapter_revision: u64,
    },
    UnsupportedSchemaVersion(u32),
    EngineMismatch {
        expected: String,
        actual: String,
    },
    EvidenceLockEngineMismatch {
        declaration_engine: String,
        lock_engine: String,
    },
    OperationNotInContract(CoreOperation),
    DuplicateOperation(CoreOperation),
    MissingOperation(CoreOperation),
    EvidenceIdOperationMismatch {
        operation: CoreOperation,
        expected: String,
        actual: String,
    },
    EvidenceNotRegistered {
        operation: CoreOperation,
        evidence_id: String,
    },
    OwnerManagedOperationNotAllowed(CoreOperation),
    OwnerManagedOperationMismatch {
        operation: CoreOperation,
        expected: AdapterOperationOwner,
        actual: AdapterOperationOwner,
    },
    EmptyReason(CoreOperation),
}

impl Display for AdapterOperationDeclarationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl Error for AdapterOperationDeclarationError {}
