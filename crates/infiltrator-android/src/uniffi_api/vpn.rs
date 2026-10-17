//! VPN/TUN surface: tun2proxy worker lifecycle (`start_vpn`/`stop_vpn_worker`), live TUN
//! status via the Android bridge, and owner-only persisted TUN configuration.

use crate::ffi::{FfiErrorCode, FfiStatus};
#[cfg(target_os = "android")]
use crate::host_session::shared_core;
#[cfg(target_os = "android")]
use crate::host_support::get_runtime;
use crate::host_support::{build_configuration_application, map_application_failure};
#[cfg(target_os = "android")]
use crate::uniffi_api::owner_operation::{android_owner_data_revision, owner_data_revision_fence};
#[cfg(target_os = "android")]
use crate::vpn_route::VpnRouteConfig;
#[cfg(target_os = "android")]
use crate::vpn_service::AndroidVpnServicePort;
#[cfg(target_os = "android")]
use crate::vpn_service::stop_data_plane_worker;
#[cfg(any(target_os = "android", test))]
use crate::vpn_service::worker::WorkerStopOutcome;
#[cfg(target_os = "android")]
use infiltrator_application::vpn_application::VpnServiceApplication;
#[cfg(target_os = "android")]
use infiltrator_application::vpn_preparation::VpnConfigurationActivationReleaseOutcome;
use infiltrator_application::vpn_preparation::VpnConfigurationActivationStatus;
#[cfg(target_os = "android")]
use infiltrator_application::vpn_preparation::{
    VpnConfigurationActivationBeginOutcome, VpnConfigurationActivationConsumeOutcome,
    VpnPreparationInstallOutcome, VpnPreparationReadyOutcome, VpnPreparationStore,
};
#[cfg(target_os = "android")]
use infiltrator_contract::android_control::AndroidHostProcessRole;
#[cfg(target_os = "android")]
use infiltrator_contract::core_identity::CoreScope;
#[cfg(target_os = "android")]
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::tun::{TunStack, TunStackUpdate};
use infiltrator_contract::vpn::VpnConfigurationActivationLease;
#[cfg(target_os = "android")]
use infiltrator_contract::vpn::VpnSessionState;
#[cfg(target_os = "android")]
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnConfiguration, VpnGeneration, VpnPreparationId, VpnRoute,
    VpnStartRequest,
};
use infiltrator_domain::tun;
#[cfg(target_os = "android")]
use infiltrator_platform::android_bridge::get_android_bridge;
#[cfg(target_os = "android")]
use std::net::IpAddr;
#[cfg(target_os = "android")]
use std::sync::Arc;
#[cfg(target_os = "android")]
use std::sync::OnceLock;
#[cfg(target_os = "android")]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_os = "android")]
static VPN_PREPARATION_STORE: OnceLock<VpnPreparationStore> = OnceLock::new();
#[cfg(target_os = "android")]
static NEXT_VPN_PREPARATION_ID: AtomicU64 = AtomicU64::new(1);
#[cfg(target_os = "android")]
static NEXT_VPN_CONFIGURATION_ACTIVATION_LEASE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, uniffi::Record)]
pub struct VpnPreparationResult {
    pub status: FfiStatus,
    pub preparation_id: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct AndroidVpnConfigurationActivationLease {
    pub lease_id: u64,
    pub preparation_id: u64,
    pub vpn_generation: u64,
    pub owner_data_revision: u64,
}

impl From<VpnConfigurationActivationLease> for AndroidVpnConfigurationActivationLease {
    fn from(lease: VpnConfigurationActivationLease) -> Self {
        Self {
            lease_id: lease.lease_id,
            preparation_id: lease.preparation_id.value(),
            vpn_generation: lease.vpn_generation.value(),
            owner_data_revision: lease.owner_data_revision,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidVpnConfigurationActivationResult {
    Acquired {
        lease: AndroidVpnConfigurationActivationLease,
    },
    StaleOwnerData {
        expected_revision: u64,
        observed_revision: u64,
    },
    PreparedRevisionMismatch {
        prepared_revision: u64,
        requested_revision: u64,
    },
    Busy,
    Missing,
    NotReady,
    TransactionMismatch,
    StaleGeneration,
    Failed {
        status: FfiStatus,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidVpnConfigurationActivationReleaseResult {
    ReleasedBeforeConsume,
    ReleasedAfterConsume,
    LeaseMissing,
    LeaseMismatch,
}

/// Start tun2proxy from the one prepared configuration owned by this VPN and
/// Core activation. The owner-data revision is rechecked and the preparation
/// consumed atomically while the activation lease still blocks owner writes.
#[uniffi::export]
pub fn start_vpn(
    fd: i32,
    vpn_generation: u64,
    preparation_id: u64,
    activation_lease_id: u64,
) -> FfiStatus {
    log::info!("Rust received VPN File Descriptor: {}", fd);

    #[cfg(target_os = "android")]
    {
        if fd <= 0 || vpn_generation == 0 || preparation_id == 0 || activation_lease_id == 0 {
            return FfiStatus::err(
                FfiErrorCode::InvalidInput,
                "VpnService supplied an invalid descriptor, transaction, or activation lease identity",
            );
        }
        let generation = VpnGeneration::new(vpn_generation);
        get_runtime().block_on(async {
            let request = {
                let _owner_data_fence = owner_data_revision_fence().await;
                let core_scope = match active_core_scope().await {
                    Ok(scope) => scope,
                    Err(status) => return status,
                };
                let observed_revision = android_owner_data_revision();
                match vpn_preparation_store().consume_activation(
                    activation_lease_id,
                    VpnPreparationId::new(preparation_id),
                    generation,
                    &core_scope,
                    observed_revision,
                    fd,
                ) {
                    VpnConfigurationActivationConsumeOutcome::Consumed(request) => request,
                    VpnConfigurationActivationConsumeOutcome::LeaseMissing => {
                        return FfiStatus::err(
                            FfiErrorCode::NotReady,
                            "VPN configuration activation lease is no longer active",
                        );
                    }
                    VpnConfigurationActivationConsumeOutcome::LeaseMismatch => {
                        return FfiStatus::err(
                            FfiErrorCode::InvalidInput,
                            "VPN start request does not match its activation lease",
                        );
                    }
                    VpnConfigurationActivationConsumeOutcome::StaleOwnerData {
                        expected_revision,
                        observed_revision,
                    } => {
                        return FfiStatus::err(
                            FfiErrorCode::NotReady,
                            format!(
                                "VPN frozen owner-data revision is stale (expected {expected_revision}, observed {observed_revision})"
                            ),
                        );
                    }
                    VpnConfigurationActivationConsumeOutcome::CoreScopeChanged => {
                        return FfiStatus::err(
                            FfiErrorCode::InvalidState,
                            "Core identity, generation, or session changed after VPN preparation",
                        );
                    }
                    VpnConfigurationActivationConsumeOutcome::InvalidTunnelDescriptor => {
                        return FfiStatus::err(
                            FfiErrorCode::InvalidInput,
                            "VpnService supplied an invalid tunnel descriptor",
                        );
                    }
                }
            };
            start_prepared_vpn(request).await
        })
    }

    #[cfg(not(target_os = "android"))]
    {
        let _android_transaction = (fd, vpn_generation, preparation_id, activation_lease_id);
        FfiStatus::err(
            FfiErrorCode::NotSupported,
            "Android VpnService is unavailable on this target",
        )
    }
}

/// Compare the frozen revision under the same serializer used by owner writes
/// and admit exactly one Builder activation lease.
#[uniffi::export]
pub fn android_begin_vpn_configuration_activation(
    preparation_id: u64,
    vpn_generation: u64,
    requested_owner_data_revision: u64,
) -> AndroidVpnConfigurationActivationResult {
    #[cfg(target_os = "android")]
    {
        if preparation_id == 0 || vpn_generation == 0 {
            return AndroidVpnConfigurationActivationResult::Failed {
                status: FfiStatus::err(
                    FfiErrorCode::InvalidInput,
                    "VPN activation requires non-zero preparation and generation identities",
                ),
            };
        }
        get_runtime().block_on(async {
            let _owner_data_fence = owner_data_revision_fence().await;
            let observed_revision = android_owner_data_revision();
            let lease_id = NEXT_VPN_CONFIGURATION_ACTIVATION_LEASE_ID.try_update(
                Ordering::SeqCst,
                Ordering::SeqCst,
                |next| next.checked_add(1),
            );
            let Ok(lease_id) = lease_id else {
                return AndroidVpnConfigurationActivationResult::Failed {
                    status: FfiStatus::err(
                        FfiErrorCode::InvalidState,
                        "VPN configuration activation lease identifiers are exhausted",
                    ),
                };
            };
            match vpn_preparation_store().begin_activation(
                VpnPreparationId::new(preparation_id),
                VpnGeneration::new(vpn_generation),
                requested_owner_data_revision,
                observed_revision,
                lease_id,
            ) {
                VpnConfigurationActivationBeginOutcome::Acquired(lease) => {
                    AndroidVpnConfigurationActivationResult::Acquired {
                        lease: lease.into(),
                    }
                }
                VpnConfigurationActivationBeginOutcome::Busy => {
                    AndroidVpnConfigurationActivationResult::Busy
                }
                VpnConfigurationActivationBeginOutcome::Missing => {
                    AndroidVpnConfigurationActivationResult::Missing
                }
                VpnConfigurationActivationBeginOutcome::NotReady => {
                    AndroidVpnConfigurationActivationResult::NotReady
                }
                VpnConfigurationActivationBeginOutcome::TransactionMismatch => {
                    AndroidVpnConfigurationActivationResult::TransactionMismatch
                }
                VpnConfigurationActivationBeginOutcome::StaleGeneration => {
                    AndroidVpnConfigurationActivationResult::StaleGeneration
                }
                VpnConfigurationActivationBeginOutcome::PreparedRevisionMismatch {
                    prepared_revision,
                    requested_revision,
                } => AndroidVpnConfigurationActivationResult::PreparedRevisionMismatch {
                    prepared_revision,
                    requested_revision,
                },
                VpnConfigurationActivationBeginOutcome::StaleOwnerData {
                    expected_revision,
                    observed_revision,
                } => AndroidVpnConfigurationActivationResult::StaleOwnerData {
                    expected_revision,
                    observed_revision,
                },
                VpnConfigurationActivationBeginOutcome::InvalidLeaseId => {
                    AndroidVpnConfigurationActivationResult::Failed {
                        status: FfiStatus::err(
                            FfiErrorCode::InvalidState,
                            "VPN configuration activation lease identifier is invalid",
                        ),
                    }
                }
            }
        })
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (
            preparation_id,
            vpn_generation,
            requested_owner_data_revision,
        );
        AndroidVpnConfigurationActivationResult::Failed {
            status: FfiStatus::err(
                FfiErrorCode::NotSupported,
                "Android VpnService is unavailable on this target",
            ),
        }
    }
}

/// Release the activation admission after Kotlin has published its terminal
/// Start receipt or completed failure/cancellation cleanup.
#[uniffi::export]
pub fn android_finish_vpn_configuration_activation(
    activation_lease_id: u64,
) -> AndroidVpnConfigurationActivationReleaseResult {
    #[cfg(target_os = "android")]
    {
        if activation_lease_id == 0 {
            return AndroidVpnConfigurationActivationReleaseResult::LeaseMismatch;
        }
        get_runtime().block_on(async {
            let _owner_data_fence = owner_data_revision_fence().await;
            map_activation_release(vpn_preparation_store().finish_activation(activation_lease_id))
        })
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = activation_lease_id;
        AndroidVpnConfigurationActivationReleaseResult::LeaseMissing
    }
}

#[cfg(target_os = "android")]
fn map_activation_release(
    outcome: VpnConfigurationActivationReleaseOutcome,
) -> AndroidVpnConfigurationActivationReleaseResult {
    match outcome {
        VpnConfigurationActivationReleaseOutcome::ReleasedBeforeConsume => {
            AndroidVpnConfigurationActivationReleaseResult::ReleasedBeforeConsume
        }
        VpnConfigurationActivationReleaseOutcome::ReleasedAfterConsume => {
            AndroidVpnConfigurationActivationReleaseResult::ReleasedAfterConsume
        }
        VpnConfigurationActivationReleaseOutcome::LeaseMissing => {
            AndroidVpnConfigurationActivationReleaseResult::LeaseMissing
        }
        VpnConfigurationActivationReleaseOutcome::LeaseMismatch => {
            AndroidVpnConfigurationActivationReleaseResult::LeaseMismatch
        }
    }
}

/// Freeze and apply the exact configuration that Kotlin's Builder and the
/// later Rust worker will consume for this VPN generation.
#[uniffi::export]
pub fn prepare_vpn(vpn_generation: u64) -> VpnPreparationResult {
    #[cfg(target_os = "android")]
    {
        if vpn_generation == 0 {
            return VpnPreparationResult {
                status: FfiStatus::err(
                    FfiErrorCode::InvalidInput,
                    "VPN preparation requires a non-zero service generation",
                ),
                preparation_id: None,
            };
        }
        get_runtime().block_on(async move {
            let _owner_data_fence = owner_data_revision_fence().await;
            if vpn_configuration_activation_status() != VpnConfigurationActivationStatus::Available {
                return VpnPreparationResult {
                    status: FfiStatus::err(
                        FfiErrorCode::NotReady,
                        "VPN configuration activation is still holding the prepared revision",
                    ),
                    preparation_id: None,
                };
            }
            let owner_data_revision = android_owner_data_revision();
            let core_scope = match active_core_scope().await {
                Ok(scope) => scope,
                Err(status) => {
                    return VpnPreparationResult {
                        status,
                        preparation_id: None,
                    };
                }
            };
            let proxy_endpoint = match resolve_proxy_url().await {
                Ok(Some(endpoint)) => endpoint,
                Ok(None) => default_proxy_url(),
                Err(status) => {
                    return VpnPreparationResult {
                        status,
                        preparation_id: None,
                    };
                }
            };
            let observed_core_scope = match active_core_scope().await {
                Ok(scope) => scope,
                Err(status) => {
                    return VpnPreparationResult {
                        status,
                        preparation_id: None,
                    };
                }
            };
            if observed_core_scope != core_scope {
                return VpnPreparationResult {
                    status: FfiStatus::err(
                        FfiErrorCode::NotReady,
                        "Core identity, generation, or session changed during VPN preparation",
                    ),
                    preparation_id: None,
                };
            }
            let configuration = match build_vpn_configuration(proxy_endpoint).await {
                Ok(configuration) => configuration,
                Err(status) => {
                    return VpnPreparationResult {
                        status,
                        preparation_id: None,
                    };
                }
            };
            let observed_owner_data_revision =
                android_owner_data_revision();
            if observed_owner_data_revision != owner_data_revision {
                return VpnPreparationResult {
                    status: FfiStatus::err(
                        FfiErrorCode::NotReady,
                        format!(
                            "owner data changed during VPN preparation (expected {owner_data_revision}, observed {observed_owner_data_revision})"
                        ),
                    ),
                    preparation_id: None,
                };
            }
            match active_core_scope().await {
                Ok(observed) if observed == core_scope => {}
                Ok(_) => {
                    return VpnPreparationResult {
                        status: FfiStatus::err(
                            FfiErrorCode::NotReady,
                            "Core identity, generation, or session changed while VPN settings were read",
                        ),
                        preparation_id: None,
                    };
                }
                Err(status) => {
                    return VpnPreparationResult {
                        status,
                        preparation_id: None,
                    };
                }
            }
            let Some(preparation_id) = next_vpn_preparation_id() else {
                return VpnPreparationResult {
                    status: FfiStatus::err(
                        FfiErrorCode::InvalidState,
                        "VPN preparation ID space is exhausted",
                    ),
                    preparation_id: None,
                };
            };
            let prepared = PreparedVpnConfiguration {
                preparation_id,
                vpn_generation: VpnGeneration::new(vpn_generation),
                owner_data_revision,
                core_scope,
                configuration,
            };
            match vpn_preparation_store().install(prepared.clone()) {
                VpnPreparationInstallOutcome::Installed
                | VpnPreparationInstallOutcome::ReplacedOlder(_) => {}
                VpnPreparationInstallOutcome::RejectedCurrent(current_id) => {
                    return VpnPreparationResult {
                        status: FfiStatus::err(
                            FfiErrorCode::InvalidState,
                            format!(
                                "VPN generation already has prepared transaction {}",
                                current_id.value()
                            ),
                        ),
                        preparation_id: None,
                    };
                }
            }
            let application = VpnServiceApplication::new(Arc::new(AndroidVpnServicePort::shared()));
            match application.prepare(prepared.clone()).await {
                Ok(snapshot) if snapshot.foreground => match active_core_scope().await {
                    Ok(current_scope)
                        if current_scope == prepared.core_scope
                            && android_owner_data_revision()
                                == prepared.owner_data_revision =>
                    {
                        match vpn_preparation_store().mark_prepared(preparation_id) {
                            VpnPreparationReadyOutcome::Ready => VpnPreparationResult {
                                status: FfiStatus::ok(),
                                preparation_id: Some(preparation_id.value()),
                            },
                            VpnPreparationReadyOutcome::Missing
                            | VpnPreparationReadyOutcome::Stale => {
                                let _ = vpn_preparation_store().clear_transaction(preparation_id);
                                VpnPreparationResult {
                                    status: FfiStatus::err(
                                        FfiErrorCode::NotReady,
                                        "VPN preparation was superseded before Builder readback",
                                    ),
                                    preparation_id: None,
                                }
                            }
                        }
                    }
                    Ok(_) => {
                        let _ = vpn_preparation_store().clear_transaction(preparation_id);
                        VpnPreparationResult {
                            status: FfiStatus::err(FfiErrorCode::NotReady, "Core scope or owner-data revision changed during Builder preparation"),
                            preparation_id: None,
                        }
                    }
                    Err(status) => {
                        let _ = vpn_preparation_store().clear_transaction(preparation_id);
                        VpnPreparationResult {
                            status,
                            preparation_id: None,
                        }
                    }
                },
                Ok(snapshot) => {
                    let _ = vpn_preparation_store().clear_transaction(preparation_id);
                    VpnPreparationResult {
                        status: FfiStatus::err(
                            FfiErrorCode::NotReady,
                            format!(
                                "VPN Builder configuration is not foreground-protected: {:?}",
                                snapshot.state
                            ),
                        ),
                        preparation_id: None,
                    }
                }
                Err(failure) => {
                    let _ = vpn_preparation_store().clear_transaction(preparation_id);
                    VpnPreparationResult {
                        status: map_application_failure(failure),
                        preparation_id: None,
                    }
                }
            }
        })
    }

    #[cfg(not(target_os = "android"))]
    {
        let _android_generation = vpn_generation;
        VpnPreparationResult {
            status: FfiStatus::err(
                FfiErrorCode::NotSupported,
                "Android VpnService is unavailable on this target",
            ),
            preparation_id: None,
        }
    }
}

/// Clear any prepared transaction at or before the stopping/revoked owner
/// generation. A late cleanup cannot erase a newer activation.
#[uniffi::export]
pub fn cancel_vpn_preparation(vpn_generation: u64) -> FfiStatus {
    #[cfg(target_os = "android")]
    {
        if vpn_generation == 0 {
            return FfiStatus::err(
                FfiErrorCode::InvalidInput,
                "VPN preparation cleanup requires a non-zero service generation",
            );
        }
        get_runtime().block_on(async {
            let _owner_data_fence = owner_data_revision_fence().await;
            let _ = vpn_preparation_store().clear_through(VpnGeneration::new(vpn_generation));
        });
        FfiStatus::ok()
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = vpn_generation;
        FfiStatus::err(
            FfiErrorCode::NotSupported,
            "Android VpnService is unavailable on this target",
        )
    }
}

#[cfg(target_os = "android")]
fn vpn_preparation_store() -> &'static VpnPreparationStore {
    VPN_PREPARATION_STORE.get_or_init(VpnPreparationStore::default)
}

#[cfg(target_os = "android")]
pub(super) fn vpn_configuration_activation_status() -> VpnConfigurationActivationStatus {
    vpn_preparation_store().activation_status()
}

#[cfg(not(target_os = "android"))]
pub(super) fn vpn_configuration_activation_status() -> VpnConfigurationActivationStatus {
    VpnConfigurationActivationStatus::Available
}

#[cfg(target_os = "android")]
fn next_vpn_preparation_id() -> Option<VpnPreparationId> {
    NEXT_VPN_PREPARATION_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .ok()
        .map(VpnPreparationId::new)
}

#[cfg(target_os = "android")]
async fn active_core_scope() -> Result<CoreScope, FfiStatus> {
    let core = shared_core().await?;
    let snapshot = core.application.snapshot();
    if snapshot.lifecycle != CoreLifecycle::Running {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            format!(
                "VPN configuration requires a Running Core, observed {:?}",
                snapshot.lifecycle
            ),
        ));
    }
    let scope = snapshot.scope().ok_or_else(|| {
        FfiStatus::err(
            FfiErrorCode::NotReady,
            "VPN configuration requires an observed Core identity",
        )
    })?;
    if !scope.session_token.is_some_and(|token| token.is_valid()) {
        return Err(FfiStatus::err(
            FfiErrorCode::NotReady,
            "VPN configuration requires an active Core session token",
        ));
    }
    Ok(scope)
}

#[path = "vpn/tun_config.rs"]
mod tun_config;
use tun_config::*;

#[uniffi::export]
pub fn stop_vpn_worker() -> FfiStatus {
    #[cfg(target_os = "android")]
    {
        match get_android_bridge().map(|bridge| bridge.process_role()) {
            Some(AndroidHostProcessRole::CoreOwner) => worker_stop_status(stop_data_plane_worker()),
            Some(AndroidHostProcessRole::UiClient) => FfiStatus::err(
                FfiErrorCode::NotReady,
                "only the :vpn owner may stop its local tun2proxy worker",
            ),
            Some(AndroidHostProcessRole::Unsupported) | None => FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android VPN worker lifecycle owner is unavailable",
            ),
        }
    }

    #[cfg(not(target_os = "android"))]
    FfiStatus::err(
        FfiErrorCode::NotSupported,
        "Android tun2proxy worker is unavailable on this target",
    )
}

#[cfg(any(target_os = "android", test))]
fn worker_stop_status(outcome: WorkerStopOutcome) -> FfiStatus {
    match outcome {
        WorkerStopOutcome::Stopped => FfiStatus::ok(),
        WorkerStopOutcome::TimedOut => FfiStatus::err(
            FfiErrorCode::NotReady,
            "timed out waiting for Android tun2proxy worker exit",
        ),
        WorkerStopOutcome::Unavailable => FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android tun2proxy worker lifecycle is unavailable",
        ),
    }
}

#[uniffi::export]
pub fn revoke_vpn() -> FfiStatus {
    #[cfg(target_os = "android")]
    {
        match get_android_bridge().map(|bridge| bridge.process_role()) {
            Some(AndroidHostProcessRole::CoreOwner) => {
                let application =
                    VpnServiceApplication::new(Arc::new(AndroidVpnServicePort::shared()));
                match get_runtime().block_on(application.revoke()) {
                    Ok(snapshot) if snapshot.state == VpnSessionState::Revoked => FfiStatus::ok(),
                    Ok(snapshot) => FfiStatus::err(
                        FfiErrorCode::InvalidState,
                        format!("VPN revoke did not settle: {:?}", snapshot.state),
                    ),
                    Err(failure) => map_application_failure(failure),
                }
            }
            Some(AndroidHostProcessRole::UiClient) => FfiStatus::err(
                FfiErrorCode::NotReady,
                "VPN revocation must be finalized by the :vpn owner",
            ),
            Some(AndroidHostProcessRole::Unsupported) | None => FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android VPN lifecycle owner is unavailable",
            ),
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        FfiStatus::err(
            FfiErrorCode::NotSupported,
            "Android VpnService is unavailable on this target",
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct VpnTunSettings {
    pub mtu: Option<u32>,
    pub auto_route: Option<bool>,
    pub strict_route: Option<bool>,
    pub stack: Option<String>,
    pub auto_detect_interface: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Enum)]
#[serde(rename_all = "lowercase")]
pub enum VpnTunStack {
    Gvisor,
    System,
    Mixed,
    Lwip,
}

impl From<VpnTunStack> for TunStack {
    fn from(stack: VpnTunStack) -> Self {
        match stack {
            VpnTunStack::Gvisor => Self::Gvisor,
            VpnTunStack::System => Self::System,
            VpnTunStack::Mixed => Self::Mixed,
            VpnTunStack::Lwip => Self::Lwip,
        }
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Enum,
)]
pub enum VpnTunStackUpdate {
    #[default]
    Keep,
    Clear,
    Set {
        stack: VpnTunStack,
    },
}

impl From<VpnTunStackUpdate> for TunStackUpdate {
    fn from(update: VpnTunStackUpdate) -> Self {
        match update {
            VpnTunStackUpdate::Keep => Self::Keep,
            VpnTunStackUpdate::Clear => Self::Clear,
            VpnTunStackUpdate::Set { stack } => Self::Set(stack.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct VpnTunSettingsPatch {
    pub mtu: Option<u32>,
    pub auto_route: Option<bool>,
    pub strict_route: Option<bool>,
    pub stack_update: VpnTunStackUpdate,
    pub auto_detect_interface: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct VpnTunSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<VpnTunSettings>,
}

pub(super) async fn owner_tun_settings_read() -> VpnTunSettingsResult {
    match load_vpn_tun_settings().await {
        Ok(settings) => VpnTunSettingsResult {
            status: FfiStatus::ok(),
            settings: Some(settings),
        },
        Err(status) => VpnTunSettingsResult {
            status,
            settings: None,
        },
    }
}

pub(super) async fn owner_tun_settings_save(patch: VpnTunSettingsPatch) -> VpnTunSettingsResult {
    match save_vpn_tun_settings(patch).await {
        Ok(settings) => VpnTunSettingsResult {
            status: FfiStatus::ok(),
            settings: Some(settings),
        },
        Err(status) => VpnTunSettingsResult {
            status,
            settings: None,
        },
    }
}

pub(super) fn build_tun_patch(patch: &VpnTunSettingsPatch) -> tun::TunConfigPatch {
    let mut core_patch = tun::TunConfigPatch::default();
    if let Some(value) = patch.mtu {
        core_patch.mtu = Some(value);
    }
    if let Some(value) = patch.auto_route {
        core_patch.auto_route = Some(value);
    }
    if let Some(value) = patch.strict_route {
        core_patch.strict_route = Some(value);
    }
    core_patch.stack = patch.stack_update.into();
    if let Some(value) = patch.auto_detect_interface {
        core_patch.auto_detect_interface = Some(value);
    }
    core_patch
}

#[cfg(test)]
#[path = "vpn/worker_stop_tests.rs"]
mod worker_stop_tests;
