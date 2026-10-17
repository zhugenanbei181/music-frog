//! Engine-neutral, operation-level capability declarations.

use crate::adapter_operation_declaration::AdapterOperationOwner;
use crate::core_identity::{CoreVersion, EngineId};
use crate::surface::HostKind;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    tag = "category",
    content = "operation",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum CoreOperation {
    Profile(ProfileOperation),
    Dns(DnsOperation),
    Rules(RulesOperation),
    Subscription(SubscriptionOperation),
    Provider(ProviderOperation),
    Outbound(OutboundOperation),
    Connection(ConnectionOperation),
    Logs(LogsOperation),
    Geo(GeoOperation),
    FakeIpCache(FakeIpCacheOperation),
    Tun(TunOperation),
    Lifecycle(CoreLifecycleOperation),
    ProxyControl(ProxyControlOperation),
    CoreGarbageCollection(CoreGarbageCollectionOperation),
    Version(VersionOperation),
}

#[path = "core_capabilities/operations.rs"]
mod operations;

#[path = "core_capabilities/matrix_resolution.rs"]
mod matrix_resolution;
use matrix_resolution::*;

pub const CORE_OPERATIONS: &[CoreOperation] = &[
    CoreOperation::Profile(ProfileOperation::Validate),
    CoreOperation::Profile(ProfileOperation::Apply),
    CoreOperation::Profile(ProfileOperation::ApplyFeature),
    CoreOperation::Dns(DnsOperation::ReadConfiguration),
    CoreOperation::Dns(DnsOperation::ApplyConfiguration),
    CoreOperation::Dns(DnsOperation::ReadHosts),
    CoreOperation::Dns(DnsOperation::WriteHosts),
    CoreOperation::Dns(DnsOperation::Query),
    CoreOperation::Dns(DnsOperation::FlushCache),
    CoreOperation::Rules(RulesOperation::Trace),
    CoreOperation::Rules(RulesOperation::Apply),
    CoreOperation::Subscription(SubscriptionOperation::Refresh),
    CoreOperation::Subscription(SubscriptionOperation::Import),
    CoreOperation::Subscription(SubscriptionOperation::Update),
    CoreOperation::Provider(ProviderOperation::ListProxy),
    CoreOperation::Provider(ProviderOperation::UpdateProxy),
    CoreOperation::Provider(ProviderOperation::HealthcheckProxy),
    CoreOperation::Provider(ProviderOperation::ListRule),
    CoreOperation::Provider(ProviderOperation::UpdateRule),
    CoreOperation::Outbound(OutboundOperation::List),
    CoreOperation::Outbound(OutboundOperation::Select),
    CoreOperation::Outbound(OutboundOperation::Probe),
    CoreOperation::Connection(ConnectionOperation::List),
    CoreOperation::Connection(ConnectionOperation::Close),
    CoreOperation::Logs(LogsOperation::Read),
    CoreOperation::Logs(LogsOperation::Stream),
    CoreOperation::Logs(LogsOperation::SetLevel),
    CoreOperation::Geo(GeoOperation::UpdateDatabases),
    CoreOperation::FakeIpCache(FakeIpCacheOperation::Read),
    CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush),
    CoreOperation::Tun(TunOperation::Inspect),
    CoreOperation::Tun(TunOperation::Configure),
    CoreOperation::Tun(TunOperation::Start),
    CoreOperation::Tun(TunOperation::Stop),
    CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
    CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
    CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
    CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
    CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
    CoreOperation::CoreGarbageCollection(CoreGarbageCollectionOperation::Trigger),
    CoreOperation::Version(VersionOperation::Inspect),
    CoreOperation::Version(VersionOperation::Install),
    CoreOperation::Version(VersionOperation::Activate),
    CoreOperation::Version(VersionOperation::Uninstall),
    CoreOperation::Version(VersionOperation::Rollback),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileOperation {
    Validate,
    Apply,
    ApplyFeature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsOperation {
    ReadConfiguration,
    ApplyConfiguration,
    ReadHosts,
    WriteHosts,
    Query,
    FlushCache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RulesOperation {
    Trace,
    Apply,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionOperation {
    Refresh,
    Import,
    Update,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOperation {
    ListProxy,
    UpdateProxy,
    HealthcheckProxy,
    ListRule,
    UpdateRule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboundOperation {
    List,
    Select,
    Probe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionOperation {
    List,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogsOperation {
    Read,
    Stream,
    SetLevel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeoOperation {
    UpdateDatabases,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FakeIpCacheOperation {
    Read,
    Flush,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TunOperation {
    Inspect,
    Configure,
    Start,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreLifecycleOperation {
    Start,
    Stop,
    Restart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyControlOperation {
    GetRoutingIntent,
    SetRoutingIntent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreGarbageCollectionOperation {
    Trigger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionOperation {
    Inspect,
    Install,
    Activate,
    Uninstall,
    Rollback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreVersionRequirement {
    RequiresActiveVersion,
    IndependentOfActiveVersion,
}

/// Adapter implementation revision used to scope capability answers.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct AdapterRevision(u64);

impl AdapterRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Concrete binary platform a backend adapter was built and exercised on.
/// Unknown build targets fail closed and never inherit another target's evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineBinaryTarget {
    #[serde(rename = "x86_64-unknown-linux-gnu")]
    X86_64UnknownLinuxGnu,
    #[serde(rename = "aarch64-unknown-linux-gnu")]
    Aarch64UnknownLinuxGnu,
    #[serde(rename = "x86_64-unknown-linux-musl")]
    X86_64UnknownLinuxMusl,
    #[serde(rename = "aarch64-unknown-linux-musl")]
    Aarch64UnknownLinuxMusl,
    #[serde(rename = "x86_64-pc-windows-msvc")]
    X86_64PcWindowsMsvc,
    #[serde(rename = "aarch64-pc-windows-msvc")]
    Aarch64PcWindowsMsvc,
    #[serde(rename = "x86_64-pc-windows-gnu")]
    X86_64PcWindowsGnu,
    #[serde(rename = "x86_64-pc-windows-gnullvm")]
    X86_64PcWindowsGnullvm,
    #[serde(rename = "aarch64-pc-windows-gnullvm")]
    Aarch64PcWindowsGnullvm,
    #[serde(rename = "x86_64-apple-darwin")]
    X86_64AppleDarwin,
    #[serde(rename = "aarch64-apple-darwin")]
    Aarch64AppleDarwin,
    #[serde(rename = "x86_64-linux-android")]
    X86_64LinuxAndroid,
    #[serde(rename = "aarch64-linux-android")]
    Aarch64LinuxAndroid,
    #[serde(rename = "x86_64-apple-ios")]
    X86_64AppleIos,
    #[serde(rename = "aarch64-apple-ios")]
    Aarch64AppleIos,
    #[serde(rename = "aarch64-apple-ios-sim")]
    Aarch64AppleIosSim,
    #[serde(rename = "aarch64-apple-ios-macabi")]
    Aarch64AppleIosMacabi,
    #[serde(rename = "unregistered")]
    Unregistered,
}

/// The exact engine/version/host/binary-target/adapter tuple for a capability lookup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreCapabilityScope {
    pub engine_id: EngineId,
    pub core_version: Option<CoreVersion>,
    pub host: HostKind,
    pub binary_target: EngineBinaryTarget,
    pub adapter_revision: AdapterRevision,
}

impl CoreCapabilityScope {
    /// Builds a scope for this process's compiled binary target.
    pub fn new(
        engine_id: EngineId,
        core_version: Option<CoreVersion>,
        host: HostKind,
        adapter_revision: AdapterRevision,
    ) -> Self {
        Self::new_for_target(
            engine_id,
            core_version,
            host,
            EngineBinaryTarget::current(),
            adapter_revision,
        )
    }

    pub fn new_for_target(
        engine_id: EngineId,
        core_version: Option<CoreVersion>,
        host: HostKind,
        binary_target: EngineBinaryTarget,
        adapter_revision: AdapterRevision,
    ) -> Self {
        Self {
            engine_id,
            core_version,
            host,
            binary_target,
            adapter_revision,
        }
    }
}

/// Full operation identity. A result from another scope cannot be reused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreCapabilityKey {
    pub scope: CoreCapabilityScope,
    pub operation: CoreOperation,
}

impl CoreCapabilityKey {
    pub fn new(scope: CoreCapabilityScope, operation: CoreOperation) -> Self {
        Self { scope, operation }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineCapabilityDecision {
    Supported,
    OwnerManaged { owner: AdapterOperationOwner },
    ReadOnly,
    Experimental,
    Unsupported { reason: String },
    VersionUnavailable { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostCapabilityDecision {
    Supported,
    ReadOnly,
    Experimental,
    Unsupported { reason: String },
    HostUnavailable { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdapterCapabilityDecision {
    Supported,
    ReadOnly,
    Experimental,
    Unsupported { reason: String },
    AdapterMissing { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreCapabilityEntry {
    pub operation: CoreOperation,
    pub engine: EngineCapabilityDecision,
    pub host: HostCapabilityDecision,
    pub adapter: AdapterCapabilityDecision,
}

impl CoreCapabilityEntry {
    pub fn new(
        operation: CoreOperation,
        engine: EngineCapabilityDecision,
        host: HostCapabilityDecision,
        adapter: AdapterCapabilityDecision,
    ) -> Self {
        Self {
            operation,
            engine,
            host,
            adapter,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "blocker", rename_all = "snake_case")]
pub enum CapabilityBlocker {
    Unsupported {
        layer: CapabilityLayer,
        reason: String,
    },
    VersionUnavailable {
        reason: String,
    },
    HostUnavailable {
        reason: String,
    },
    AdapterMissing {
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityLayer {
    Engine,
    Host,
    Adapter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAccess {
    Supported,
    ReadOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityMaturity {
    Stable,
    Experimental,
}

/// Fail-closed effective result after combining all three independent judgements.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CoreCapabilityStatus {
    Available {
        access: CapabilityAccess,
        maturity: CapabilityMaturity,
    },
    Unsupported {
        blockers: Vec<CapabilityBlocker>,
    },
    VersionUnavailable {
        blockers: Vec<CapabilityBlocker>,
    },
    HostUnavailable {
        blockers: Vec<CapabilityBlocker>,
    },
    AdapterMissing {
        blockers: Vec<CapabilityBlocker>,
    },
    Unregistered,
    ScopeMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreCapabilityResolution {
    pub key: CoreCapabilityKey,
    pub engine: Option<EngineCapabilityDecision>,
    pub host: Option<HostCapabilityDecision>,
    pub adapter: Option<AdapterCapabilityDecision>,
    pub status: CoreCapabilityStatus,
}

impl CoreCapabilityResolution {
    fn unavailable(key: CoreCapabilityKey, status: CoreCapabilityStatus) -> Self {
        Self {
            key,
            engine: None,
            host: None,
            adapter: None,
            status,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreCapabilityMatrixError {
    DuplicateOperation(CoreOperation),
    OwnerManagedOperationNotAllowed(CoreOperation),
    OwnerManagedOperationMismatch {
        operation: CoreOperation,
        expected: AdapterOperationOwner,
        actual: AdapterOperationOwner,
    },
}

impl Display for CoreCapabilityMatrixError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateOperation(operation) => {
                write!(
                    formatter,
                    "duplicate core capability operation: {operation:?}"
                )
            }
            Self::OwnerManagedOperationNotAllowed(operation) => {
                write!(
                    formatter,
                    "owner-managed capability is not allowed for {operation:?}"
                )
            }
            Self::OwnerManagedOperationMismatch {
                operation,
                expected,
                actual,
            } => write!(
                formatter,
                "owner-managed capability for {operation:?} requires {expected:?}, received {actual:?}"
            ),
        }
    }
}

impl Error for CoreCapabilityMatrixError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CoreCapabilityMatrix {
    pub scope: CoreCapabilityScope,
    pub entries: Vec<CoreCapabilityEntry>,
}

impl CoreCapabilityMatrix {
    pub fn new(
        scope: CoreCapabilityScope,
        entries: Vec<CoreCapabilityEntry>,
    ) -> Result<Self, CoreCapabilityMatrixError> {
        Self::validate_entries(&entries)?;
        Ok(Self { scope, entries })
    }

    /// Revalidate the public matrix contents at trust boundaries. Providers
    /// may construct a valid matrix and then mutate its public entries.
    pub fn validate(&self) -> Result<(), CoreCapabilityMatrixError> {
        Self::validate_entries(&self.entries)
    }

    fn validate_entries(entries: &[CoreCapabilityEntry]) -> Result<(), CoreCapabilityMatrixError> {
        let mut operations = HashSet::with_capacity(entries.len());
        for entry in entries {
            if !operations.insert(entry.operation) {
                return Err(CoreCapabilityMatrixError::DuplicateOperation(
                    entry.operation,
                ));
            }
            if let EngineCapabilityDecision::OwnerManaged { owner } = &entry.engine {
                let Some(expected) = entry.operation.owner_managed_owner() else {
                    return Err(CoreCapabilityMatrixError::OwnerManagedOperationNotAllowed(
                        entry.operation,
                    ));
                };
                if *owner != expected {
                    return Err(CoreCapabilityMatrixError::OwnerManagedOperationMismatch {
                        operation: entry.operation,
                        expected,
                        actual: *owner,
                    });
                }
            }
        }
        Ok(())
    }

    pub fn resolve(&self, key: CoreCapabilityKey) -> CoreCapabilityResolution {
        if key.scope != self.scope {
            return CoreCapabilityResolution::unavailable(key, CoreCapabilityStatus::ScopeMismatch);
        }

        let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.operation == key.operation)
        else {
            return CoreCapabilityResolution::unavailable(key, CoreCapabilityStatus::Unregistered);
        };

        let status = self.effective_status(entry);
        CoreCapabilityResolution {
            key,
            engine: Some(entry.engine.clone()),
            host: Some(entry.host.clone()),
            adapter: Some(entry.adapter.clone()),
            status,
        }
    }

    fn effective_status(&self, entry: &CoreCapabilityEntry) -> CoreCapabilityStatus {
        let mut blockers = Vec::new();
        let mut access = CapabilityAccess::Supported;
        let mut maturity = CapabilityMaturity::Stable;

        apply_engine_decision(&entry.engine, &mut access, &mut maturity, &mut blockers);
        apply_host_decision(&entry.host, &mut access, &mut maturity, &mut blockers);
        apply_adapter_decision(&entry.adapter, &mut access, &mut maturity, &mut blockers);

        let version_is_unavailable = blockers
            .iter()
            .any(|blocker| matches!(blocker, CapabilityBlocker::VersionUnavailable { .. }));
        let version_is_required = matches!(
            entry.operation.version_requirement(),
            CoreVersionRequirement::RequiresActiveVersion
        );
        if self.scope.core_version.is_none() && version_is_required && !version_is_unavailable {
            blockers.push(CapabilityBlocker::VersionUnavailable {
                reason: "active core version is unavailable".to_string(),
            });
        }

        if blockers.is_empty() {
            return CoreCapabilityStatus::Available { access, maturity };
        }

        status_from_blockers(blockers)
    }
}

impl<'de> Deserialize<'de> for CoreCapabilityMatrix {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct MatrixWire {
            scope: CoreCapabilityScope,
            entries: Vec<CoreCapabilityEntry>,
        }

        let wire = MatrixWire::deserialize(deserializer)?;
        Self::new(wire.scope, wire.entries).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CoreCapabilitySnapshot {
    pub scope: CoreCapabilityScope,
    pub resolutions: Vec<CoreCapabilityResolution>,
}

impl CoreCapabilitySnapshot {
    pub fn from_matrix(matrix: &CoreCapabilityMatrix) -> Self {
        let resolutions = CoreOperation::all()
            .iter()
            .copied()
            .map(|operation| {
                matrix.resolve(CoreCapabilityKey::new(matrix.scope.clone(), operation))
            })
            .collect();
        Self {
            scope: matrix.scope.clone(),
            resolutions,
        }
    }

    /// Check that every owner-authored resolution belongs to this exact scope
    /// and that the snapshot covers the complete operation catalogue.
    pub fn validate(&self) -> Result<(), CoreCapabilitySnapshotError> {
        validate_capability_resolutions(&self.scope, &self.resolutions)
    }

    pub fn resolution(&self, operation: CoreOperation) -> Option<&CoreCapabilityResolution> {
        let mut matches = self
            .resolutions
            .iter()
            .filter(|resolution| resolution.key.operation == operation);
        let resolution = matches.next()?;
        if resolution.key.scope != self.scope || matches.next().is_some() {
            return None;
        }
        Some(resolution)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreCapabilitySnapshotError {
    ScopeMismatch,
    DuplicateOperation(CoreOperation),
    MissingOperation(CoreOperation),
    OwnerManagedOperationNotAllowed(CoreOperation),
    OwnerManagedOperationMismatch {
        operation: CoreOperation,
        expected: AdapterOperationOwner,
        actual: AdapterOperationOwner,
    },
}

impl Display for CoreCapabilitySnapshotError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScopeMismatch => {
                formatter.write_str("core capability resolution has a different scope")
            }
            Self::DuplicateOperation(operation) => {
                write!(
                    formatter,
                    "duplicate capability resolution for {operation:?}"
                )
            }
            Self::MissingOperation(operation) => {
                write!(formatter, "missing capability resolution for {operation:?}")
            }
            Self::OwnerManagedOperationNotAllowed(operation) => {
                write!(
                    formatter,
                    "owner-managed capability is not allowed for {operation:?}"
                )
            }
            Self::OwnerManagedOperationMismatch {
                operation,
                expected,
                actual,
            } => write!(
                formatter,
                "owner-managed capability for {operation:?} requires {expected:?}, received {actual:?}"
            ),
        }
    }
}

impl Error for CoreCapabilitySnapshotError {}

fn validate_capability_resolutions(
    scope: &CoreCapabilityScope,
    resolutions: &[CoreCapabilityResolution],
) -> Result<(), CoreCapabilitySnapshotError> {
    let mut operations = HashSet::with_capacity(resolutions.len());
    for resolution in resolutions {
        if resolution.key.scope != *scope {
            return Err(CoreCapabilitySnapshotError::ScopeMismatch);
        }
        if !operations.insert(resolution.key.operation) {
            return Err(CoreCapabilitySnapshotError::DuplicateOperation(
                resolution.key.operation,
            ));
        }
        if let Some(EngineCapabilityDecision::OwnerManaged { owner }) = resolution.engine.as_ref() {
            let operation = resolution.key.operation;
            let Some(expected) = operation.owner_managed_owner() else {
                return Err(
                    CoreCapabilitySnapshotError::OwnerManagedOperationNotAllowed(operation),
                );
            };
            if *owner != expected {
                return Err(CoreCapabilitySnapshotError::OwnerManagedOperationMismatch {
                    operation,
                    expected,
                    actual: *owner,
                });
            }
        }
    }
    if let Some(operation) = CoreOperation::all()
        .iter()
        .find(|operation| !operations.contains(operation))
    {
        return Err(CoreCapabilitySnapshotError::MissingOperation(*operation));
    }
    Ok(())
}

#[derive(Deserialize)]
struct CoreCapabilitySnapshotFields {
    scope: CoreCapabilityScope,
    resolutions: Vec<CoreCapabilityResolution>,
}

impl<'de> Deserialize<'de> for CoreCapabilitySnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = CoreCapabilitySnapshotFields::deserialize(deserializer)?;
        validate_capability_resolutions(&fields.scope, &fields.resolutions)
            .map_err(D::Error::custom)?;
        Ok(Self {
            scope: fields.scope,
            resolutions: fields.resolutions,
        })
    }
}

#[cfg(test)]
#[path = "core_capabilities_tests.rs"]
mod tests;
