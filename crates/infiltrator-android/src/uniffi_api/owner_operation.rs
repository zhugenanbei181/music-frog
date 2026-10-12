//! Typed request/reply boundary for persisted Android product data owned by
//! the `:vpn` process. Product UI uses the exported Rust operation enum and
//! typed reply; JSON is only the service-side Messenger wire encoding.

#[cfg(test)]
use super::app_routing::AndroidAppRoutingSnapshot;
use super::app_routing::AndroidVpnRoutingActivationLease;
use super::app_routing::AndroidVpnRoutingActivationReleaseResult;
use super::app_routing::AndroidVpnRoutingActivationResult;
use super::app_routing::{
    AndroidAppRoutingMode, AndroidAppRoutingPackageSelection, AndroidAppRoutingResult,
    load_android_app_routing, set_android_app_routing_mode, set_android_app_routing_packages,
};
use super::control::android_core_lifecycle_status;
use super::profiles::read_owner_profiles;
use super::profiles::{ProfileDetailResult, ProfilesResult};
use super::rules::load_rule_providers_local;
use super::rules::load_rules_local;
use super::rules::save_rule_providers_local;
use super::rules::save_rules_local;
#[cfg(test)]
use super::rules::{AndroidJsonEntry, AndroidJsonValue};
use super::rules::{
    AndroidRuleEntry, AndroidRuleProviderEntry, AndroidRuleProvidersResult, AndroidRulesResult,
};
use super::settings::DnsSettingsPatch;
use super::settings::FakeIpSettingsPatch;
use super::settings::clear_fake_ip_cache_local;
use super::settings::dns_settings_save_local;
use super::settings::read_dns_settings_local;
use super::settings::read_fake_ip_settings_local;
use super::settings::save_fake_ip_settings_local;
#[cfg(test)]
use super::settings::{DnsSettings, FakeIpSettings};
use super::settings::{DnsSettingsResult, FakeIpSettingsResult};
use super::vpn::{VpnTunSettingsPatch, VpnTunSettingsResult};
use super::webdav::{WebDavSettings, WebDavSettingsResult, WebDavSyncResult};
use crate::ffi::FfiBoolResult;
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_support::get_runtime;
use infiltrator_contract::android_control::AndroidHostProcessRole;
use infiltrator_platform::android_bridge::get_android_bridge;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync;

#[path = "owner_operation/version.rs"]
mod version;
#[path = "owner_operation/wire.rs"]
pub mod wire;
pub use version::{
    AndroidCoreRelease, AndroidCoreReleaseSummary, AndroidCoreVersionInstallCancellation,
    AndroidCoreVersionInstallContext, AndroidCoreVersionInstallObservation,
    AndroidCoreVersionInstallState, AndroidCoreVersionManagementOperation,
    AndroidCoreVersionManagementReadback, AndroidEngineNeutralReleaseChannel,
    AndroidInstalledCoreVersion, AndroidVersionDownloadProgress,
};

static OWNER_OPERATION_SERIALIZER: sync::Mutex<()> = sync::Mutex::const_new(());
static APP_ROUTING_SERIALIZER: sync::Mutex<()> = sync::Mutex::const_new(());
static APP_ROUTING_REVISION: AtomicU64 = AtomicU64::new(1);
static NEXT_VPN_ROUTING_LEASE_ID: AtomicU64 = AtomicU64::new(1);
static VPN_ROUTING_ACTIVATION: Mutex<AppRoutingActivationState> =
    Mutex::new(AppRoutingActivationState::Available);

/// One serializer fences Core/TUN/DNS preparation reads and their owner writes.
/// VPN activation holds only the typed preparation lease; it never keeps this
/// async mutex across Android Builder work.
pub(super) async fn owner_data_revision_fence() -> sync::MutexGuard<'static, ()> {
    OWNER_OPERATION_SERIALIZER.lock().await
}

pub(super) enum OwnerDataRevisionFenceAcquire {
    Acquired(sync::MutexGuard<'static, ()>),
    Busy,
}

pub(super) fn try_owner_data_revision_fence() -> OwnerDataRevisionFenceAcquire {
    match OWNER_OPERATION_SERIALIZER.try_lock() {
        Ok(guard) => OwnerDataRevisionFenceAcquire::Acquired(guard),
        Err(_) => OwnerDataRevisionFenceAcquire::Busy,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OwnerOperationMutationAdmission {
    ReadOnly,
    WriteAllowed,
    VpnConfigurationBusy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AndroidOwnerDataMutation {
    ReadOnly,
    CommitOnSuccess,
    CommitRegardlessOfOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AndroidOwnerRevisionTransition {
    Unchanged,
    Advance,
}

pub(super) fn owner_operation_mutation_admission(
    operation: &AndroidOwnerOperation,
    activation: infiltrator_application::vpn_preparation::VpnConfigurationActivationStatus,
) -> OwnerOperationMutationAdmission {
    use infiltrator_application::vpn_preparation::VpnConfigurationActivationStatus;

    match (operation.data_mutation(), activation) {
        (AndroidOwnerDataMutation::ReadOnly, _) => OwnerOperationMutationAdmission::ReadOnly,
        (
            AndroidOwnerDataMutation::CommitOnSuccess
            | AndroidOwnerDataMutation::CommitRegardlessOfOutcome,
            VpnConfigurationActivationStatus::Available,
        ) => OwnerOperationMutationAdmission::WriteAllowed,
        (
            AndroidOwnerDataMutation::CommitOnSuccess
            | AndroidOwnerDataMutation::CommitRegardlessOfOutcome,
            VpnConfigurationActivationStatus::BuilderAdmitted
            | VpnConfigurationActivationStatus::DataPlaneStarting,
        ) => OwnerOperationMutationAdmission::VpnConfigurationBusy,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppRoutingActivationState {
    Available,
    Active {
        lease: AndroidVpnRoutingActivationLease,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AppRoutingMutationPermission {
    Allowed,
    Blocked(FfiStatus),
}

impl AppRoutingActivationState {
    fn begin(
        &mut self,
        expected_routing_revision: u64,
        observed_routing_revision: u64,
        lease_id: u64,
    ) -> AndroidVpnRoutingActivationResult {
        match self {
            Self::Active { .. } => AndroidVpnRoutingActivationResult::Busy,
            Self::Available if expected_routing_revision != observed_routing_revision => {
                AndroidVpnRoutingActivationResult::Stale {
                    observed_routing_revision,
                }
            }
            Self::Available => {
                let lease = AndroidVpnRoutingActivationLease {
                    lease_id,
                    routing_revision: observed_routing_revision,
                };
                *self = Self::Active { lease };
                AndroidVpnRoutingActivationResult::Acquired { lease }
            }
        }
    }

    fn finish(&mut self, lease_id: u64) -> AndroidVpnRoutingActivationReleaseResult {
        match *self {
            Self::Available => AndroidVpnRoutingActivationReleaseResult::LeaseMissing,
            Self::Active { lease } if lease.lease_id != lease_id => {
                AndroidVpnRoutingActivationReleaseResult::LeaseMismatch
            }
            Self::Active { .. } => {
                *self = Self::Available;
                AndroidVpnRoutingActivationReleaseResult::Released
            }
        }
    }

    fn blocks_mutations(self) -> bool {
        matches!(self, Self::Active { .. })
    }
}

pub(super) fn begin_vpn_routing_activation(
    expected_routing_revision: u64,
) -> AndroidVpnRoutingActivationResult {
    let Ok(mut state) = VPN_ROUTING_ACTIVATION.lock() else {
        return AndroidVpnRoutingActivationResult::Failed {
            status: FfiStatus::err(
                FfiErrorCode::Unknown,
                "VPN routing activation state is unavailable",
            ),
        };
    };
    if state.blocks_mutations() {
        return AndroidVpnRoutingActivationResult::Busy;
    }
    let observed_routing_revision = app_routing_revision();
    if expected_routing_revision != observed_routing_revision {
        return AndroidVpnRoutingActivationResult::Stale {
            observed_routing_revision,
        };
    }
    let lease_id =
        NEXT_VPN_ROUTING_LEASE_ID.try_update(Ordering::SeqCst, Ordering::SeqCst, |next| {
            next.checked_add(1)
        });
    let Ok(lease_id) = lease_id else {
        return AndroidVpnRoutingActivationResult::Failed {
            status: FfiStatus::err(
                FfiErrorCode::InvalidState,
                "VPN routing activation lease identifiers are exhausted",
            ),
        };
    };
    state.begin(
        expected_routing_revision,
        observed_routing_revision,
        lease_id,
    )
}

pub(super) fn finish_vpn_routing_activation(
    lease_id: u64,
) -> AndroidVpnRoutingActivationReleaseResult {
    let Ok(mut state) = VPN_ROUTING_ACTIVATION.lock() else {
        return AndroidVpnRoutingActivationReleaseResult::Failed {
            status: FfiStatus::err(
                FfiErrorCode::Unknown,
                "VPN routing activation state is unavailable",
            ),
        };
    };
    state.finish(lease_id)
}

fn app_routing_mutation_permission() -> AppRoutingMutationPermission {
    match VPN_ROUTING_ACTIVATION.lock() {
        Ok(state) if !state.blocks_mutations() => AppRoutingMutationPermission::Allowed,
        Ok(_) => AppRoutingMutationPermission::Blocked(FfiStatus::err(
            FfiErrorCode::NotReady,
            "VPN routing cannot change while a tunnel activation lease is active",
        )),
        Err(_) => AppRoutingMutationPermission::Blocked(FfiStatus::err(
            FfiErrorCode::Unknown,
            "VPN routing activation state is unavailable; mutations are blocked",
        )),
    }
}

pub(super) fn try_lock_app_routing() -> Result<sync::MutexGuard<'static, ()>, ()> {
    APP_ROUTING_SERIALIZER.try_lock().map_err(|_| ())
}

pub(super) fn app_routing_revision() -> u64 {
    APP_ROUTING_REVISION.load(Ordering::SeqCst)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum AndroidOwnerOperationProtocolVersion {
    V5,
}

/// Request operations are exhaustive typed variants. Unknown enum variants,
/// versions, or fields fail deserialization before any owner code runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AndroidOwnerOperationRequest {
    pub protocol: AndroidOwnerOperationProtocolVersion,
    pub operation: AndroidOwnerOperation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(deny_unknown_fields)]
pub enum AndroidOwnerOperation {
    ProfilesList,
    ProfileCreate {
        name: String,
        url: String,
    },
    ProfileSelect {
        name: String,
    },
    ProfileUpdate {
        name: String,
    },
    ProfileDetail {
        name: String,
    },
    ProfileSave {
        name: String,
        content: String,
        activate: bool,
    },
    ProfileDelete {
        name: String,
    },
    ProfileSubscriptionSave {
        name: String,
        url: String,
        auto_update_enabled: bool,
        update_interval_hours: Option<u32>,
    },
    ProfileSubscriptionClear {
        name: String,
    },
    CoreVersionManagement {
        operation: AndroidCoreVersionManagementOperation,
    },
    DnsSettings,
    DnsSettingsSave {
        patch: DnsSettingsPatch,
    },
    TunSettings,
    TunSettingsSave {
        patch: VpnTunSettingsPatch,
    },
    FakeIpSettings,
    FakeIpSettingsSave {
        patch: FakeIpSettingsPatch,
    },
    FakeIpCacheClear,
    RulesList,
    RulesSave {
        rules: Vec<AndroidRuleEntry>,
    },
    RuleProviders,
    RuleProvidersSave {
        providers: Vec<AndroidRuleProviderEntry>,
    },
    AppRoutingSnapshot,
    AppRoutingModeSet {
        mode: AndroidAppRoutingMode,
    },
    AppRoutingPackagesSet {
        packages: Vec<String>,
        selection: AndroidAppRoutingPackageSelection,
    },
    WebDavSettings,
    WebDavSettingsSave {
        settings: WebDavSettings,
    },
    WebDavTest {
        settings: WebDavSettings,
    },
    WebDavSyncNow,
}

impl AndroidOwnerOperation {
    pub(super) fn data_mutation(&self) -> AndroidOwnerDataMutation {
        match self {
            Self::CoreVersionManagement { operation } => operation.data_mutation(),
            Self::ProfilesList
            | Self::ProfileDetail { .. }
            | Self::DnsSettings
            | Self::TunSettings
            | Self::FakeIpSettings
            | Self::RulesList
            | Self::RuleProviders
            | Self::AppRoutingSnapshot
            | Self::WebDavSettings
            | Self::WebDavTest { .. }
            | Self::WebDavSyncNow => AndroidOwnerDataMutation::ReadOnly,
            Self::ProfileCreate { .. }
            | Self::ProfileSelect { .. }
            | Self::ProfileUpdate { .. }
            | Self::ProfileSave { .. }
            | Self::ProfileDelete { .. }
            | Self::ProfileSubscriptionSave { .. }
            | Self::ProfileSubscriptionClear { .. }
            | Self::TunSettingsSave { .. }
            | Self::WebDavSettingsSave { .. }
            | Self::AppRoutingModeSet { .. }
            | Self::AppRoutingPackagesSet { .. } => {
                AndroidOwnerDataMutation::CommitRegardlessOfOutcome
            }
            Self::DnsSettingsSave { .. }
            | Self::FakeIpSettingsSave { .. }
            | Self::FakeIpCacheClear
            | Self::RulesSave { .. }
            | Self::RuleProvidersSave { .. } => AndroidOwnerDataMutation::CommitOnSuccess,
        }
    }

    pub(super) fn data_domain(&self) -> AndroidOwnerDataDomain {
        match self {
            Self::ProfilesList
            | Self::ProfileCreate { .. }
            | Self::ProfileSelect { .. }
            | Self::ProfileUpdate { .. }
            | Self::ProfileDetail { .. }
            | Self::ProfileSave { .. }
            | Self::ProfileDelete { .. }
            | Self::ProfileSubscriptionSave { .. }
            | Self::ProfileSubscriptionClear { .. } => AndroidOwnerDataDomain::Profiles,
            Self::CoreVersionManagement { .. } => AndroidOwnerDataDomain::CoreVersions,
            Self::DnsSettings | Self::DnsSettingsSave { .. } => AndroidOwnerDataDomain::DnsSettings,
            Self::TunSettings | Self::TunSettingsSave { .. } => AndroidOwnerDataDomain::TunSettings,
            Self::FakeIpSettings | Self::FakeIpSettingsSave { .. } => {
                AndroidOwnerDataDomain::FakeIpSettings
            }
            Self::FakeIpCacheClear => AndroidOwnerDataDomain::FakeIpCache,
            Self::RulesList | Self::RulesSave { .. } => AndroidOwnerDataDomain::Rules,
            Self::RuleProviders | Self::RuleProvidersSave { .. } => {
                AndroidOwnerDataDomain::RuleProviders
            }
            Self::AppRoutingSnapshot
            | Self::AppRoutingModeSet { .. }
            | Self::AppRoutingPackagesSet { .. } => AndroidOwnerDataDomain::AppRouting,
            Self::WebDavSettings
            | Self::WebDavSettingsSave { .. }
            | Self::WebDavTest { .. }
            | Self::WebDavSyncNow => AndroidOwnerDataDomain::WebDav,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum AndroidOwnerDataDomain {
    Profiles,
    CoreVersions,
    DnsSettings,
    TunSettings,
    FakeIpSettings,
    FakeIpCache,
    Rules,
    RuleProviders,
    AppRouting,
    WebDav,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum AndroidOwnerOperationIpcState {
    Completed,
    StaleOwnerData,
    VpnConfigurationBusy,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidOwnerOperationContext {
    pub client_session_id: String,
    pub expected_owner_session_id: String,
    pub owner_session_id: String,
    pub expected_core_generation: u64,
    pub expected_data_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidOwnerDataSnapshot {
    pub domain: AndroidOwnerDataDomain,
    pub data: AndroidOwnerDataSnapshotData,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(deny_unknown_fields)]
pub enum AndroidOwnerDataSnapshotData {
    Profiles(ProfilesResult),
    CoreVersions(AndroidCoreVersionManagementReadback),
    DnsSettings(DnsSettingsResult),
    TunSettings(VpnTunSettingsResult),
    FakeIpSettings(FakeIpSettingsResult),
    Rules(AndroidRulesResult),
    RuleProviders(AndroidRuleProvidersResult),
    AppRouting(AndroidAppRoutingResult),
    WebDavSettings(WebDavSettingsResult),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(deny_unknown_fields)]
pub enum AndroidOwnerOperationReadback {
    ProfilesList(ProfilesResult),
    ProfileDetail(ProfileDetailResult),
    ProfileMutation(FfiStatus),
    CoreVersionManagement(AndroidCoreVersionManagementReadback),
    DnsSettings(DnsSettingsResult),
    TunSettings(VpnTunSettingsResult),
    FakeIpSettings(FakeIpSettingsResult),
    Rules(AndroidRulesResult),
    RuleProviders(AndroidRuleProvidersResult),
    AppRouting(AndroidAppRoutingResult),
    WebDavSettings(WebDavSettingsResult),
    WebDavTest(FfiStatus),
    WebDavSyncNow(WebDavSyncResult),
    FakeIpCacheClear(FfiBoolResult),
    Rejected(FfiStatus),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidOwnerOperationReply {
    pub protocol: AndroidOwnerOperationProtocolVersion,
    pub context: AndroidOwnerOperationContext,
    pub observed_core_generation: u64,
    pub data_revision: u64,
    pub data_domain: Option<AndroidOwnerDataDomain>,
    pub snapshot: Option<AndroidOwnerDataSnapshot>,
    pub readback: AndroidOwnerOperationReadback,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidOwnerOperationClientResult {
    Completed {
        reply: Box<AndroidOwnerOperationReply>,
    },
    Failed {
        status: FfiStatus,
    },
}

/// The local branch is the `:vpn` Core owner only. Android UI clients either
/// receive an owner reply or a typed error; they never get permission to
/// construct a local configuration application.
pub(super) enum AndroidOwnerOperationRoute {
    CoreOwner,
    UiClient(Box<AndroidOwnerOperationReply>),
}

/// Stable UniFFI carrier used by the `:vpn` service. The full owner-authored
/// reply remains a versioned Rust DTO in `outcome_json`.
#[derive(Clone, Debug, uniffi::Record)]
pub struct AndroidOwnerOperationIpcResult {
    pub state: AndroidOwnerOperationIpcState,
    pub status_code: FfiErrorCode,
    pub status_message: Option<String>,
    pub core_generation: u64,
    pub data_revision: u64,
    pub data_domain: Option<AndroidOwnerDataDomain>,
    pub snapshot_json: Option<String>,
    pub outcome_json: String,
}

static OWNER_DATA_REVISION: AtomicU64 = AtomicU64::new(1);

#[uniffi::export]
pub fn android_owner_data_revision() -> u64 {
    OWNER_DATA_REVISION.load(Ordering::SeqCst)
}

pub(super) fn advance_owner_data_revision() -> Option<u64> {
    OWNER_DATA_REVISION
        .try_update(Ordering::SeqCst, Ordering::SeqCst, |revision| {
            revision.checked_add(1)
        })
        .ok()
        .and_then(|revision| revision.checked_add(1))
}

fn advance_app_routing_revision() -> Option<u64> {
    APP_ROUTING_REVISION
        .try_update(Ordering::SeqCst, Ordering::SeqCst, |revision| {
            revision.checked_add(1)
        })
        .ok()
        .and_then(|revision| revision.checked_add(1))
}

fn owner_revision_transition(
    mutation: AndroidOwnerDataMutation,
    readback: &AndroidOwnerOperationReadback,
    status: FfiErrorCode,
) -> AndroidOwnerRevisionTransition {
    if let AndroidOwnerOperationReadback::CoreVersionManagement(result) = readback {
        return result.revision_transition();
    }
    match mutation {
        AndroidOwnerDataMutation::ReadOnly => AndroidOwnerRevisionTransition::Unchanged,
        AndroidOwnerDataMutation::CommitRegardlessOfOutcome => {
            AndroidOwnerRevisionTransition::Advance
        }
        AndroidOwnerDataMutation::CommitOnSuccess => match readback {
            AndroidOwnerOperationReadback::FakeIpCacheClear(result)
                if result.status.code == FfiErrorCode::Ok && result.value =>
            {
                AndroidOwnerRevisionTransition::Advance
            }
            AndroidOwnerOperationReadback::FakeIpCacheClear(_) => {
                AndroidOwnerRevisionTransition::Unchanged
            }
            _ if status == FfiErrorCode::Ok => AndroidOwnerRevisionTransition::Advance,
            _ => AndroidOwnerRevisionTransition::Unchanged,
        },
    }
}

fn app_routing_mutation_readback(
    mutation_status: FfiStatus,
) -> (
    AndroidOwnerOperationReadback,
    Option<AndroidOwnerDataSnapshot>,
) {
    let current = load_android_app_routing();
    let readback = if mutation_status.code == FfiErrorCode::Ok {
        current.clone()
    } else {
        AndroidAppRoutingResult {
            status: mutation_status,
            snapshot: None,
        }
    };
    let snapshot = AndroidOwnerDataSnapshot {
        domain: AndroidOwnerDataDomain::AppRouting,
        data: AndroidOwnerDataSnapshotData::AppRouting(current),
    };
    (
        AndroidOwnerOperationReadback::AppRouting(readback),
        Some(snapshot),
    )
}

async fn profile_mutation_readback(
    status: FfiStatus,
) -> (
    AndroidOwnerOperationReadback,
    Option<AndroidOwnerDataSnapshot>,
) {
    let profiles = read_owner_profiles().await;
    let snapshot = AndroidOwnerDataSnapshot {
        domain: AndroidOwnerDataDomain::Profiles,
        data: AndroidOwnerDataSnapshotData::Profiles(profiles),
    };
    (
        AndroidOwnerOperationReadback::ProfileMutation(status),
        Some(snapshot),
    )
}

fn rejected_result(
    context: AndroidOwnerOperationContext,
    observed_generation: u64,
    data_revision: u64,
    status: FfiStatus,
    state: AndroidOwnerOperationIpcState,
) -> AndroidOwnerOperationIpcResult {
    let reply = AndroidOwnerOperationReply {
        protocol: AndroidOwnerOperationProtocolVersion::V5,
        context,
        observed_core_generation: observed_generation,
        data_revision,
        data_domain: None,
        snapshot: None,
        readback: AndroidOwnerOperationReadback::Rejected(status.clone()),
    };
    ipc_result_from_reply(state, status, observed_generation, data_revision, &reply)
}

fn ipc_result_from_reply(
    state: AndroidOwnerOperationIpcState,
    status: FfiStatus,
    generation: u64,
    data_revision: u64,
    reply: &AndroidOwnerOperationReply,
) -> AndroidOwnerOperationIpcResult {
    let snapshot_json = reply
        .snapshot
        .as_ref()
        .and_then(|snapshot| serde_json::to_string(snapshot).ok());
    let outcome_json = serde_json::to_string(reply).unwrap_or_else(|error| {
        log::error!("Android owner operation reply serialization failed: {error}");
        String::new()
    });
    AndroidOwnerOperationIpcResult {
        state,
        status_code: status.code,
        status_message: status.message,
        core_generation: generation,
        data_revision,
        data_domain: reply.data_domain,
        snapshot_json,
        outcome_json,
    }
}

fn status_from_readback(readback: &AndroidOwnerOperationReadback) -> FfiStatus {
    match readback {
        AndroidOwnerOperationReadback::ProfilesList(result) => result.status.clone(),
        AndroidOwnerOperationReadback::ProfileDetail(result) => result.status.clone(),
        AndroidOwnerOperationReadback::DnsSettings(result) => result.status.clone(),
        AndroidOwnerOperationReadback::TunSettings(result) => result.status.clone(),
        AndroidOwnerOperationReadback::FakeIpSettings(result) => result.status.clone(),
        AndroidOwnerOperationReadback::FakeIpCacheClear(result) => result.status.clone(),
        AndroidOwnerOperationReadback::Rules(result) => result.status.clone(),
        AndroidOwnerOperationReadback::RuleProviders(result) => result.status.clone(),
        AndroidOwnerOperationReadback::AppRouting(result) => result.status.clone(),
        AndroidOwnerOperationReadback::WebDavSettings(result) => result.status.clone(),
        AndroidOwnerOperationReadback::WebDavTest(status) => status.clone(),
        AndroidOwnerOperationReadback::WebDavSyncNow(result) => result.status.clone(),
        AndroidOwnerOperationReadback::ProfileMutation(status)
        | AndroidOwnerOperationReadback::Rejected(status) => status.clone(),
        AndroidOwnerOperationReadback::CoreVersionManagement(result) => result.status(),
    }
}

#[cfg(test)]
#[path = "owner_operation_tests.rs"]
mod tests;
