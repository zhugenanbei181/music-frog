use super::*;
use infiltrator_contract::android_control::core_launch::{
    CoreLaunchArguments, CoreLaunchBinary, CoreLaunchConfigurationIdentity, CoreLaunchPath,
};
use infiltrator_contract::android_control::{
    AndroidChildProcessIdentity, AndroidOwnedCoreProcess, VpnOwnerCoreReadback, VpnOwnerLifecycle,
    VpnOwnerReadiness,
};
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::{ProfileDocumentRevision, ProfileId};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_contract::vpn::{VpnConfiguration, VpnGeneration, VpnPreparationId, VpnRoute};
use lurker_core_contract::session::SessionToken;

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
        CoreLaunchArguments::Mihomo {
            configuration_path: CoreLaunchPath::new("/data/config.yaml".to_owned())
                .expect("absolute config path"),
        },
    )
    .expect("valid test launch spec")
}

fn start_context() -> CoreProcessStartContext {
    CoreProcessStartContext::managed(
        CoreIdentity::new(
            EngineId::new("mihomo").expect("test engine"),
            CoreInstanceId::new("android-test").expect("test instance"),
            None,
        ),
        CoreGeneration::new(4),
        SessionToken::new(9),
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
    ) -> AndroidBridgeResult<CoreProcessStartReceipt> {
        Ok(CoreProcessStartReceipt::Started)
    }

    async fn core_stop(&self) -> AndroidBridgeResult<CoreProcessStopReceipt> {
        Ok(CoreProcessStopReceipt::StoppedGracefully)
    }

    async fn core_process_observation(&self) -> AndroidBridgeResult<AndroidCoreProcessObservation> {
        Ok(AndroidCoreProcessObservation::Running {
            owned: AndroidOwnedCoreProcess {
                context: start_context(),
                child: AndroidChildProcessIdentity {
                    pid: 321,
                    start_time_ticks: 987_654,
                },
            },
        })
    }

    async fn credential_get(
        &self,
        _service: &str,
        _key: &str,
    ) -> AndroidBridgeResult<Option<String>> {
        Ok(Some("secret".to_string()))
    }

    async fn credential_set(
        &self,
        _service: &str,
        _key: &str,
        _value: &str,
    ) -> AndroidBridgeResult<()> {
        Ok(())
    }

    async fn credential_delete(&self, _service: &str, _key: &str) -> AndroidBridgeResult<()> {
        Ok(())
    }

    fn data_dir(&self) -> Option<PathBuf> {
        Some(PathBuf::from("data"))
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        Some(PathBuf::from("cache"))
    }

    async fn vpn_start(&self) -> AndroidBridgeResult<VpnStartOutcome> {
        Ok(VpnStartOutcome::Running)
    }

    async fn vpn_apply_configuration(
        &self,
        _prepared: &PreparedVpnConfiguration,
    ) -> AndroidBridgeResult<VpnConfigurationApplyOutcome> {
        Ok(VpnConfigurationApplyOutcome::Accepted)
    }

    async fn vpn_stop(&self, target: VpnStopTarget) -> AndroidBridgeResult<VpnOwnerStopReceipt> {
        let snapshot = self.vpn_owner_snapshot().await?;
        let failure = Failure::new(
            ErrorCode::InvalidState,
            "test stop readback remains running",
            true,
        );
        Ok(match target {
            VpnStopTarget::Current => VpnOwnerStopReceipt::FailedWithReadback { failure, snapshot },
            VpnStopTarget::Generation(_) => {
                VpnOwnerStopReceipt::SupersededWithReadback { failure, snapshot }
            }
        })
    }

    async fn vpn_owner_snapshot(&self) -> AndroidBridgeResult<VpnOwnerSnapshot> {
        Ok(VpnOwnerSnapshot {
            owner_session_id: "owner-session".to_owned(),
            revision: 3,
            vpn_generation: VpnGeneration::new(8),
            vpn_state: VpnOwnerLifecycle::Running,
            readiness: VpnOwnerReadiness::Ready,
            resources: Vec::new(),
            foreground: true,
            mtu: Some(1500),
            route_count: 1,
            dns_servers: vec!["1.1.1.1".to_owned()],
            ipv6: false,
            core: VpnOwnerCoreReadback::Available(CoreLifecycleSnapshot {
                identity: None,
                lifecycle: CoreLifecycle::Stopped,
                generation: 0,
                session_token: None,
                revision: 1,
                failure: None,
            }),
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
            owned: AndroidOwnedCoreProcess {
                context: start_context(),
                child: AndroidChildProcessIdentity {
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
                preparation_id: VpnPreparationId::new(1),
                vpn_generation: VpnGeneration::new(1),
                owner_data_revision: 1,
                core_scope: CoreScope {
                    identity: CoreIdentity::new(
                        EngineId::new("mihomo").expect("engine ID"),
                        CoreInstanceId::new("primary").expect("instance ID"),
                        None,
                    ),
                    generation: CoreGeneration::new(1),
                    session_token: Some(SessionToken::new(1)),
                },
                configuration: VpnConfiguration {
                    proxy_endpoint: "socks5://127.0.0.1:7891".to_owned(),
                    mtu: 1500,
                    routes: vec![VpnRoute {
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
            .vpn_stop(VpnStopTarget::Generation(VpnGeneration::new(17),))
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
