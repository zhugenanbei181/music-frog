use async_trait::async_trait;
use infiltrator_contract::android_control::{
    AndroidCoreLifecycleSnapshot, AndroidCoreProcessObservation, AndroidCoreProcessRecoveryReceipt,
    AndroidHostProcessRole, CoreLaunchConfigurationIdentity, CoreLaunchSpec, VpnAuthorizationState,
    VpnOwnerSnapshot,
};
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnConfigurationApplyOutcome, VpnOwnerStopReceipt, VpnStartOutcome,
    VpnStopTarget,
};
use infiltrator_ports::core_process::CoreProcessStartContext;
use infiltrator_ports::error::PortError;
use mihomo_api::error::{MihomoError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

/// One bounded request for the Android `:vpn` owner to validate a staged
/// Mihomo profile with the packaged same-ABI executable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AndroidCoreConfigurationValidationRequest {
    pub request_id: u64,
    pub timeout_millis: u64,
    pub configuration_identity: CoreLaunchConfigurationIdentity,
    pub candidate_path: PathBuf,
    pub candidate_digest: String,
    pub working_directory: PathBuf,
}

/// Exhaustive terminal result from Android's native stopped-Core validator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidCoreConfigurationValidationOutcome {
    Validated,
    ConfigurationRejected { exit_code: i32 },
    CoreActive,
    BinaryUnavailable,
    CandidateMissing,
    CandidatePathRejected,
    CandidateDigestMismatch,
    CandidateIdentityMismatch,
    DeadlineExpired,
    DeadlineTooFar,
    TimedOut,
    TimeoutCleanupFailed,
    Interrupted,
    InterruptedCleanupFailed,
    ProcessCleanupFailed,
    ValidationCleanupPending,
    CoreProcessObservationUnavailable,
    ProcessLaunchFailed,
    NotVpnOwner,
}

/// A request echo binds the terminal response to its candidate and caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AndroidCoreConfigurationValidationReply {
    Receipt {
        request: AndroidCoreConfigurationValidationRequest,
        outcome: AndroidCoreConfigurationValidationOutcome,
    },
    InvalidRequest {
        reason: String,
    },
}

/// Core start receipt returned by the Android host JNI boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreProcessStartReceipt {
    Started,
    AlreadyRunning,
    BinaryUnavailable,
    LaunchSpecRejected,
    SpawnFailed,
    NotVpnOwner,
    OwnerContextMismatch,
    PreviousChildExitPending,
    OwnershipRecordUnavailable,
}

impl CoreProcessStartReceipt {
    pub const fn wire_code(self) -> i32 {
        match self {
            Self::Started => 0,
            Self::AlreadyRunning => 1,
            Self::BinaryUnavailable => 2,
            Self::LaunchSpecRejected => 3,
            Self::SpawnFailed => 4,
            Self::NotVpnOwner => 5,
            Self::OwnerContextMismatch => 6,
            Self::PreviousChildExitPending => 7,
            Self::OwnershipRecordUnavailable => 8,
        }
    }

    pub const fn from_wire_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Started),
            1 => Some(Self::AlreadyRunning),
            2 => Some(Self::BinaryUnavailable),
            3 => Some(Self::LaunchSpecRejected),
            4 => Some(Self::SpawnFailed),
            5 => Some(Self::NotVpnOwner),
            6 => Some(Self::OwnerContextMismatch),
            7 => Some(Self::PreviousChildExitPending),
            8 => Some(Self::OwnershipRecordUnavailable),
            _ => None,
        }
    }

    pub fn into_port_result(self) -> std::result::Result<(), PortError> {
        match self {
            Self::Started | Self::AlreadyRunning => Ok(()),
            Self::BinaryUnavailable => Err(PortError::NotFound(
                "Android Core executable is unavailable".to_owned(),
            )),
            Self::LaunchSpecRejected => Err(PortError::Failed(
                "Android Core launch specification was rejected".to_owned(),
            )),
            Self::SpawnFailed => Err(PortError::Failed(
                "Android Core process could not be started".to_owned(),
            )),
            Self::NotVpnOwner => Err(PortError::Failed(
                "Android UI process cannot start the core locally".to_owned(),
            )),
            Self::OwnerContextMismatch => Err(PortError::Failed(
                "Android Mihomo child belongs to a different Core session".to_owned(),
            )),
            Self::PreviousChildExitPending => Err(PortError::Failed(
                "Android Core child exit awaits owner recovery".to_owned(),
            )),
            Self::OwnershipRecordUnavailable => Err(PortError::Failed(
                "Android Core process ownership record is unavailable".to_owned(),
            )),
        }
    }
}

/// Core stop receipt returned by the Android host JNI boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreProcessStopReceipt {
    StoppedGracefully,
    StoppedForcibly,
    AlreadyStopped,
    TimedOut,
    Interrupted,
    NotVpnOwner,
    OwnershipRecordUnavailable,
}

impl CoreProcessStopReceipt {
    pub const fn wire_code(self) -> i32 {
        match self {
            Self::StoppedGracefully => 0,
            Self::StoppedForcibly => 1,
            Self::AlreadyStopped => 2,
            Self::TimedOut => 3,
            Self::Interrupted => 4,
            Self::NotVpnOwner => 5,
            Self::OwnershipRecordUnavailable => 6,
        }
    }

    pub const fn from_wire_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::StoppedGracefully),
            1 => Some(Self::StoppedForcibly),
            2 => Some(Self::AlreadyStopped),
            3 => Some(Self::TimedOut),
            4 => Some(Self::Interrupted),
            5 => Some(Self::NotVpnOwner),
            6 => Some(Self::OwnershipRecordUnavailable),
            _ => None,
        }
    }

    pub fn into_port_result(self) -> std::result::Result<(), PortError> {
        match self {
            Self::StoppedGracefully | Self::StoppedForcibly | Self::AlreadyStopped => Ok(()),
            Self::TimedOut => Err(PortError::Failed(
                "Android Core process did not stop before the deadline".to_owned(),
            )),
            Self::Interrupted => Err(PortError::Failed(
                "Android Core process stop was interrupted".to_owned(),
            )),
            Self::NotVpnOwner => Err(PortError::Failed(
                "Android UI process cannot stop the core locally".to_owned(),
            )),
            Self::OwnershipRecordUnavailable => Err(PortError::Failed(
                "Android Core process ownership record is unavailable".to_owned(),
            )),
        }
    }
}

#[async_trait]
pub trait AndroidBridge: Send + Sync {
    /// Typed role of the process where this bridge was registered. Only
    /// `CoreOwner` may construct a `CoreApplication` session.
    fn process_role(&self) -> AndroidHostProcessRole {
        AndroidHostProcessRole::Unsupported
    }

    /// Core process result receipt; operation failures remain typed values.
    async fn core_start(
        &self,
        launch_spec: &CoreLaunchSpec,
        context: &CoreProcessStartContext,
    ) -> Result<CoreProcessStartReceipt>;
    /// Core process result receipt; operation failures remain typed values.
    async fn core_stop(&self) -> Result<CoreProcessStopReceipt>;
    async fn core_process_observation(&self) -> Result<AndroidCoreProcessObservation>;
    /// Consume a matching exited-child record only after the shared Rust owner
    /// proof has been recovered from that exact managed context.
    async fn core_process_recovery_completed(
        &self,
        _context: &CoreProcessStartContext,
    ) -> Result<AndroidCoreProcessRecoveryReceipt> {
        Err(MihomoError::Service(
            "Android Core process recovery acknowledgement is unavailable on this bridge"
                .to_owned(),
        ))
    }
    /// Validate one frozen candidate on the process that owns the packaged
    /// Android Core binary. Implementations without that host capability fail closed.
    async fn core_validate_configuration(
        &self,
        _request: &AndroidCoreConfigurationValidationRequest,
    ) -> Result<AndroidCoreConfigurationValidationOutcome> {
        Err(MihomoError::Service(
            "Android stopped-Core configuration validation is unavailable on this bridge"
                .to_owned(),
        ))
    }

    /// Read the owner-authored lifecycle, including its fencing metadata.
    async fn core_lifecycle_snapshot(&self) -> Result<AndroidCoreLifecycleSnapshot> {
        Err(MihomoError::Service(
            "Android core lifecycle IPC is unavailable on this bridge".to_owned(),
        ))
    }

    /// Submit a Serde-tagged shared Rust product command through the owner.
    async fn execute_product_command_json(&self, _command_json: &str) -> Result<String> {
        Err(MihomoError::Service(
            "Android product command IPC is unavailable on this bridge".to_owned(),
        ))
    }

    /// Read the owner-authored shared page snapshot through typed IPC.
    async fn surface_snapshot_json(&self) -> Result<String> {
        Err(MihomoError::Service(
            "Android surface snapshot IPC is unavailable on this bridge".to_owned(),
        ))
    }

    /// Execute a typed Android configuration operation through the `:vpn`
    /// owner and return its Serde-tagged outcome.
    async fn execute_owner_operation_json(&self, _operation_json: &str) -> Result<String> {
        Err(MihomoError::Service(
            "Android owner operation IPC is unavailable on this bridge".to_owned(),
        ))
    }

    async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>>;
    async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()>;
    async fn credential_delete(&self, service: &str, key: &str) -> Result<()>;

    fn data_dir(&self) -> Option<PathBuf>;
    fn cache_dir(&self) -> Option<PathBuf>;
    /// Active physical-link MTU, when the native Android host exposes it.
    fn physical_mtu(&self) -> Option<u32> {
        None
    }

    /// Native code may report its local packaged-core/config preflight. The
    /// conservative default keeps older bridges compatible and never claims
    /// offline readiness without host evidence.
    fn offline_startup_snapshot(&self) -> OfflineStartupSnapshot {
        OfflineStartupSnapshot::default()
    }

    /// Ask the native host to run VpnService.prepare/consent handling and
    /// promote the service to the foreground. The TUN FD is returned to Rust
    /// later through the start_vpn callback.
    async fn vpn_start(&self) -> Result<VpnStartOutcome>;
    async fn vpn_authorization_state(&self) -> Result<VpnAuthorizationState> {
        Ok(VpnAuthorizationState::Required)
    }
    /// Apply the typed, Core-scoped prepared VPN transaction before handing
    /// the established TUN FD back to Rust. Every bridge must implement the
    /// Builder operation.
    async fn vpn_apply_configuration(
        &self,
        prepared: &PreparedVpnConfiguration,
    ) -> Result<VpnConfigurationApplyOutcome>;
    async fn vpn_stop(&self, target: VpnStopTarget) -> Result<VpnOwnerStopReceipt>;
    /// Read one owner-authored snapshot binding VPN, Core, resource and
    /// readiness facts to a single owner session and revision.
    async fn vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot>;
}

#[async_trait]
impl AndroidBridge for Box<dyn AndroidBridge> {
    fn process_role(&self) -> AndroidHostProcessRole {
        self.as_ref().process_role()
    }

    async fn core_start(
        &self,
        launch_spec: &CoreLaunchSpec,
        context: &CoreProcessStartContext,
    ) -> Result<CoreProcessStartReceipt> {
        self.as_ref().core_start(launch_spec, context).await
    }

    async fn core_stop(&self) -> Result<CoreProcessStopReceipt> {
        self.as_ref().core_stop().await
    }

    async fn core_process_observation(&self) -> Result<AndroidCoreProcessObservation> {
        self.as_ref().core_process_observation().await
    }

    async fn core_process_recovery_completed(
        &self,
        context: &CoreProcessStartContext,
    ) -> Result<AndroidCoreProcessRecoveryReceipt> {
        self.as_ref().core_process_recovery_completed(context).await
    }

    async fn core_validate_configuration(
        &self,
        request: &AndroidCoreConfigurationValidationRequest,
    ) -> Result<AndroidCoreConfigurationValidationOutcome> {
        self.as_ref().core_validate_configuration(request).await
    }

    async fn core_lifecycle_snapshot(&self) -> Result<AndroidCoreLifecycleSnapshot> {
        self.as_ref().core_lifecycle_snapshot().await
    }

    async fn execute_product_command_json(&self, command_json: &str) -> Result<String> {
        self.as_ref()
            .execute_product_command_json(command_json)
            .await
    }

    async fn surface_snapshot_json(&self) -> Result<String> {
        self.as_ref().surface_snapshot_json().await
    }

    async fn execute_owner_operation_json(&self, operation_json: &str) -> Result<String> {
        self.as_ref()
            .execute_owner_operation_json(operation_json)
            .await
    }

    async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>> {
        self.as_ref().credential_get(service, key).await
    }

    async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        self.as_ref().credential_set(service, key, value).await
    }

    async fn credential_delete(&self, service: &str, key: &str) -> Result<()> {
        self.as_ref().credential_delete(service, key).await
    }

    fn data_dir(&self) -> Option<PathBuf> {
        self.as_ref().data_dir()
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        self.as_ref().cache_dir()
    }

    fn offline_startup_snapshot(&self) -> OfflineStartupSnapshot {
        self.as_ref().offline_startup_snapshot()
    }

    fn physical_mtu(&self) -> Option<u32> {
        self.as_ref().physical_mtu()
    }

    async fn vpn_start(&self) -> Result<VpnStartOutcome> {
        self.as_ref().vpn_start().await
    }

    async fn vpn_authorization_state(&self) -> Result<VpnAuthorizationState> {
        self.as_ref().vpn_authorization_state().await
    }

    async fn vpn_apply_configuration(
        &self,
        prepared: &PreparedVpnConfiguration,
    ) -> Result<VpnConfigurationApplyOutcome> {
        self.as_ref().vpn_apply_configuration(prepared).await
    }

    async fn vpn_stop(&self, target: VpnStopTarget) -> Result<VpnOwnerStopReceipt> {
        self.as_ref().vpn_stop(target).await
    }

    async fn vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot> {
        self.as_ref().vpn_owner_snapshot().await
    }
}

#[async_trait]
impl AndroidBridge for Arc<dyn AndroidBridge> {
    fn process_role(&self) -> AndroidHostProcessRole {
        self.as_ref().process_role()
    }

    async fn core_start(
        &self,
        launch_spec: &CoreLaunchSpec,
        context: &CoreProcessStartContext,
    ) -> Result<CoreProcessStartReceipt> {
        self.as_ref().core_start(launch_spec, context).await
    }

    async fn core_stop(&self) -> Result<CoreProcessStopReceipt> {
        self.as_ref().core_stop().await
    }

    async fn core_process_observation(&self) -> Result<AndroidCoreProcessObservation> {
        self.as_ref().core_process_observation().await
    }

    async fn core_process_recovery_completed(
        &self,
        context: &CoreProcessStartContext,
    ) -> Result<AndroidCoreProcessRecoveryReceipt> {
        self.as_ref().core_process_recovery_completed(context).await
    }

    async fn core_validate_configuration(
        &self,
        request: &AndroidCoreConfigurationValidationRequest,
    ) -> Result<AndroidCoreConfigurationValidationOutcome> {
        self.as_ref().core_validate_configuration(request).await
    }

    async fn core_lifecycle_snapshot(&self) -> Result<AndroidCoreLifecycleSnapshot> {
        self.as_ref().core_lifecycle_snapshot().await
    }

    async fn execute_product_command_json(&self, command_json: &str) -> Result<String> {
        self.as_ref()
            .execute_product_command_json(command_json)
            .await
    }

    async fn surface_snapshot_json(&self) -> Result<String> {
        self.as_ref().surface_snapshot_json().await
    }

    async fn execute_owner_operation_json(&self, operation_json: &str) -> Result<String> {
        self.as_ref()
            .execute_owner_operation_json(operation_json)
            .await
    }

    async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>> {
        self.as_ref().credential_get(service, key).await
    }

    async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        self.as_ref().credential_set(service, key, value).await
    }

    async fn credential_delete(&self, service: &str, key: &str) -> Result<()> {
        self.as_ref().credential_delete(service, key).await
    }

    fn data_dir(&self) -> Option<PathBuf> {
        self.as_ref().data_dir()
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        self.as_ref().cache_dir()
    }

    fn offline_startup_snapshot(&self) -> OfflineStartupSnapshot {
        self.as_ref().offline_startup_snapshot()
    }

    fn physical_mtu(&self) -> Option<u32> {
        self.as_ref().physical_mtu()
    }

    async fn vpn_start(&self) -> Result<VpnStartOutcome> {
        self.as_ref().vpn_start().await
    }

    async fn vpn_authorization_state(&self) -> Result<VpnAuthorizationState> {
        self.as_ref().vpn_authorization_state().await
    }

    async fn vpn_apply_configuration(
        &self,
        prepared: &PreparedVpnConfiguration,
    ) -> Result<VpnConfigurationApplyOutcome> {
        self.as_ref().vpn_apply_configuration(prepared).await
    }

    async fn vpn_stop(&self, target: VpnStopTarget) -> Result<VpnOwnerStopReceipt> {
        self.as_ref().vpn_stop(target).await
    }

    async fn vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot> {
        self.as_ref().vpn_owner_snapshot().await
    }
}

pub fn set_android_bridge(bridge: Arc<dyn AndroidBridge>) {
    let mut guard = android_bridge_state()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = Some(bridge);
}

pub fn clear_android_bridge() {
    let mut guard = android_bridge_state()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = None;
}

pub fn get_android_bridge() -> Option<Arc<dyn AndroidBridge>> {
    android_bridge_state()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn android_bridge_state() -> &'static RwLock<Option<Arc<dyn AndroidBridge>>> {
    static ANDROID_BRIDGE: OnceLock<RwLock<Option<Arc<dyn AndroidBridge>>>> = OnceLock::new();
    ANDROID_BRIDGE.get_or_init(|| RwLock::new(None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::android_control::{
        CoreLaunchBinary, CoreLaunchConfigurationIdentity,
    };
    use infiltrator_contract::core_identity::EngineId;
    use infiltrator_contract::profile_document_identity::{ProfileDocumentRevision, ProfileId};

    fn launch_spec() -> CoreLaunchSpec {
        CoreLaunchSpec::new(
            CoreLaunchConfigurationIdentity {
                engine_id: EngineId::new("mihomo").expect("engine ID"),
                profile_id: ProfileId::new("default").expect("profile ID"),
                document_revision: ProfileDocumentRevision::new("sha256:test")
                    .expect("document revision"),
            },
            CoreLaunchBinary::MihomoBundled,
            PathBuf::from("/data"),
            vec!["-d".to_owned(), "/data".to_owned()],
        )
        .expect("valid test launch spec")
    }

    fn start_context() -> CoreProcessStartContext {
        CoreProcessStartContext::managed(
            infiltrator_contract::core_identity::CoreIdentity::new(
                infiltrator_contract::core_identity::EngineId::new("mihomo").expect("test engine"),
                infiltrator_contract::core_identity::CoreInstanceId::new("android-test")
                    .expect("test instance"),
                None,
            ),
            infiltrator_contract::core_identity::CoreGeneration::new(4),
            infiltrator_contract::session::SessionToken::new(9),
        )
    }

    #[test]
    fn native_configuration_validation_wire_matches_kotlin_receipt_shape() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let request = AndroidCoreConfigurationValidationRequest {
            request_id: 41,
            timeout_millis: 30_000,
            configuration_identity: CoreLaunchConfigurationIdentity {
                engine_id: EngineId::new("mihomo").expect("engine ID"),
                profile_id: ProfileId::new("primary").expect("profile ID"),
                document_revision: ProfileDocumentRevision::new("a".repeat(64))
                    .expect("candidate revision"),
            },
            candidate_path: PathBuf::from(
                "/data/user/0/com.musicfrog.infiltrator/files/cloud/profiles/engines/mihomo/.primary.yaml.tmp-1-41",
            ),
            candidate_digest: digest.clone(),
            working_directory: PathBuf::from("/data/user/0/com.musicfrog.infiltrator/files"),
        };
        let encoded = serde_json::to_value(&request).expect("request serializes");
        assert_eq!(encoded["request_id"], 41);
        assert_eq!(encoded["timeout_millis"], 30_000);
        assert_eq!(encoded["candidate_digest"], digest);

        let response = serde_json::json!({
            "Receipt": {
                "request": encoded,
                "outcome": "Validated"
            }
        });
        let decoded: AndroidCoreConfigurationValidationReply =
            serde_json::from_value(response).expect("Kotlin receipt deserializes");
        assert_eq!(
            decoded,
            AndroidCoreConfigurationValidationReply::Receipt {
                request,
                outcome: AndroidCoreConfigurationValidationOutcome::Validated,
            }
        );
    }

    struct TestBridge;

    #[async_trait]
    impl AndroidBridge for TestBridge {
        async fn core_start(
            &self,
            _launch_spec: &CoreLaunchSpec,
            _context: &CoreProcessStartContext,
        ) -> Result<CoreProcessStartReceipt> {
            Ok(CoreProcessStartReceipt::Started)
        }

        async fn core_stop(&self) -> Result<CoreProcessStopReceipt> {
            Ok(CoreProcessStopReceipt::StoppedGracefully)
        }

        async fn core_process_observation(&self) -> Result<AndroidCoreProcessObservation> {
            Ok(AndroidCoreProcessObservation::Running {
                owned: infiltrator_contract::android_control::AndroidOwnedCoreProcess {
                    context: start_context(),
                    child: infiltrator_contract::android_control::AndroidChildProcessIdentity {
                        pid: 321,
                        start_time_ticks: 987_654,
                    },
                },
            })
        }

        async fn credential_get(&self, _service: &str, _key: &str) -> Result<Option<String>> {
            Ok(Some("secret".to_string()))
        }

        async fn credential_set(&self, _service: &str, _key: &str, _value: &str) -> Result<()> {
            Ok(())
        }

        async fn credential_delete(&self, _service: &str, _key: &str) -> Result<()> {
            Ok(())
        }

        fn data_dir(&self) -> Option<PathBuf> {
            Some(PathBuf::from("data"))
        }

        fn cache_dir(&self) -> Option<PathBuf> {
            Some(PathBuf::from("cache"))
        }

        async fn vpn_start(&self) -> Result<VpnStartOutcome> {
            Ok(VpnStartOutcome::Running)
        }

        async fn vpn_apply_configuration(
            &self,
            _prepared: &PreparedVpnConfiguration,
        ) -> Result<VpnConfigurationApplyOutcome> {
            Ok(VpnConfigurationApplyOutcome::Accepted)
        }

        async fn vpn_stop(&self, target: VpnStopTarget) -> Result<VpnOwnerStopReceipt> {
            let snapshot = self.vpn_owner_snapshot().await?;
            let failure = infiltrator_contract::error::Failure::new(
                infiltrator_contract::error::ErrorCode::InvalidState,
                "test stop readback remains running",
                true,
            );
            Ok(match target {
                VpnStopTarget::Current => {
                    VpnOwnerStopReceipt::FailedWithReadback { failure, snapshot }
                }
                VpnStopTarget::Generation(_) => {
                    VpnOwnerStopReceipt::SupersededWithReadback { failure, snapshot }
                }
            })
        }

        async fn vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot> {
            Ok(VpnOwnerSnapshot {
                owner_session_id: "owner-session".to_owned(),
                revision: 3,
                vpn_generation: infiltrator_contract::vpn::VpnGeneration::new(8),
                vpn_state: infiltrator_contract::android_control::VpnOwnerLifecycle::Running,
                readiness: infiltrator_contract::android_control::VpnOwnerReadiness::Ready,
                resources: Vec::new(),
                foreground: true,
                mtu: Some(1500),
                route_count: 1,
                dns_servers: vec!["1.1.1.1".to_owned()],
                ipv6: false,
                core: infiltrator_contract::android_control::VpnOwnerCoreReadback::Available(
                    infiltrator_contract::snapshot::CoreLifecycleSnapshot {
                        identity: None,
                        lifecycle: infiltrator_contract::snapshot::CoreLifecycle::Stopped,
                        generation: 0,
                        session_token: None,
                        revision: 1,
                        failure: None,
                    },
                ),
            })
        }
    }

    #[tokio::test]
    async fn test_android_bridge_registry() {
        clear_android_bridge();
        set_android_bridge(Arc::new(TestBridge));
        let bridge = get_android_bridge().expect("bridge");

        // Exhaustive check of proxied methods via Arc
        assert_eq!(
            bridge
                .core_start(&launch_spec(), &start_context())
                .await
                .unwrap(),
            CoreProcessStartReceipt::Started
        );
        assert_eq!(
            bridge.core_stop().await.unwrap(),
            CoreProcessStopReceipt::StoppedGracefully
        );
        assert_eq!(
            bridge.core_process_observation().await.unwrap(),
            AndroidCoreProcessObservation::Running {
                owned: infiltrator_contract::android_control::AndroidOwnedCoreProcess {
                    context: start_context(),
                    child: infiltrator_contract::android_control::AndroidChildProcessIdentity {
                        pid: 321,
                        start_time_ticks: 987_654,
                    },
                },
            }
        );
        assert_eq!(
            bridge.credential_get("s", "k").await.unwrap(),
            Some("secret".to_string())
        );
        assert!(bridge.credential_set("s", "k", "v").await.is_ok());
        assert!(bridge.credential_delete("s", "k").await.is_ok());

        assert_eq!(bridge.data_dir().unwrap(), PathBuf::from("data"));
        assert_eq!(bridge.cache_dir().unwrap(), PathBuf::from("cache"));

        assert_eq!(bridge.vpn_start().await.unwrap(), VpnStartOutcome::Running);
        assert_eq!(
            bridge
                .vpn_apply_configuration(&PreparedVpnConfiguration {
                    preparation_id: infiltrator_contract::vpn::VpnPreparationId::new(1),
                    vpn_generation: infiltrator_contract::vpn::VpnGeneration::new(1),
                    owner_data_revision: 1,
                    core_scope: infiltrator_contract::core_identity::CoreScope {
                        identity: infiltrator_contract::core_identity::CoreIdentity::new(
                            infiltrator_contract::core_identity::EngineId::new("mihomo")
                                .expect("engine ID"),
                            infiltrator_contract::core_identity::CoreInstanceId::new("primary")
                                .expect("instance ID"),
                            None,
                        ),
                        generation: infiltrator_contract::core_identity::CoreGeneration::new(1),
                        session_token: Some(infiltrator_contract::session::SessionToken::new(1)),
                    },
                    configuration: infiltrator_contract::vpn::VpnConfiguration {
                        proxy_endpoint: "socks5://127.0.0.1:7891".to_owned(),
                        mtu: 1500,
                        routes: vec![infiltrator_contract::vpn::VpnRoute {
                            address: "0.0.0.0".to_owned(),
                            prefix: 0,
                            exclude: false,
                        }],
                        dns_servers: vec!["1.1.1.1".to_owned()],
                        ipv6: true,
                        foreground_requested: true,
                    },
                })
                .await
                .unwrap(),
            VpnConfigurationApplyOutcome::Accepted
        );
        assert!(matches!(
            bridge.vpn_stop(VpnStopTarget::Current).await.unwrap(),
            VpnOwnerStopReceipt::FailedWithReadback { .. }
        ));
        assert!(matches!(
            bridge
                .vpn_stop(VpnStopTarget::Generation(
                    infiltrator_contract::vpn::VpnGeneration::new(17),
                ))
                .await
                .unwrap(),
            VpnOwnerStopReceipt::SupersededWithReadback { .. }
        ));
        assert_eq!(bridge.vpn_owner_snapshot().await.unwrap().revision, 3);

        // Test Box proxy
        let boxed: Box<dyn AndroidBridge> = Box::new(TestBridge);
        assert_eq!(
            boxed
                .core_start(&launch_spec(), &start_context())
                .await
                .unwrap(),
            CoreProcessStartReceipt::Started
        );
        assert_eq!(boxed.data_dir().unwrap(), PathBuf::from("data"));

        clear_android_bridge();
        assert!(get_android_bridge().is_none());
    }

    #[test]
    fn core_start_receipt_wire_codes_are_exhaustive_and_fail_closed() {
        for (receipt, code) in [
            (CoreProcessStartReceipt::Started, 0),
            (CoreProcessStartReceipt::AlreadyRunning, 1),
            (CoreProcessStartReceipt::BinaryUnavailable, 2),
            (CoreProcessStartReceipt::LaunchSpecRejected, 3),
            (CoreProcessStartReceipt::SpawnFailed, 4),
            (CoreProcessStartReceipt::NotVpnOwner, 5),
            (CoreProcessStartReceipt::OwnerContextMismatch, 6),
            (CoreProcessStartReceipt::PreviousChildExitPending, 7),
            (CoreProcessStartReceipt::OwnershipRecordUnavailable, 8),
        ] {
            assert_eq!(receipt.wire_code(), code);
            assert_eq!(CoreProcessStartReceipt::from_wire_code(code), Some(receipt));
        }
        assert_eq!(CoreProcessStartReceipt::from_wire_code(-1), None);
        assert_eq!(CoreProcessStartReceipt::from_wire_code(9), None);
    }

    #[test]
    fn core_stop_receipt_wire_codes_are_exhaustive_and_fail_closed() {
        for (receipt, code) in [
            (CoreProcessStopReceipt::StoppedGracefully, 0),
            (CoreProcessStopReceipt::StoppedForcibly, 1),
            (CoreProcessStopReceipt::AlreadyStopped, 2),
            (CoreProcessStopReceipt::TimedOut, 3),
            (CoreProcessStopReceipt::Interrupted, 4),
            (CoreProcessStopReceipt::NotVpnOwner, 5),
            (CoreProcessStopReceipt::OwnershipRecordUnavailable, 6),
        ] {
            assert_eq!(receipt.wire_code(), code);
            assert_eq!(CoreProcessStopReceipt::from_wire_code(code), Some(receipt));
        }
        assert_eq!(CoreProcessStopReceipt::from_wire_code(-1), None);
        assert_eq!(CoreProcessStopReceipt::from_wire_code(7), None);
    }
}
