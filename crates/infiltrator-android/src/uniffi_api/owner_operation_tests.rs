use super::super::rules;
use super::super::settings;
use super::wire::{
    android_dispatch_owner_operation_wire_json, android_execute_owner_operation, execute_operation,
    owner_data_fence_matches, validate_owner_operation_reply,
};
use super::*;
use crate::host_support::build_routing_application;
use crate::uniffi_api::{VpnTunStack, VpnTunStackUpdate};
use infiltrator_application::vpn_preparation::VpnConfigurationActivationStatus;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_domain::app_routing::AppRoutingMode;
use infiltrator_domain::app_routing::AppRoutingRule;
use infiltrator_platform::TEST_LOCK;
use infiltrator_platform::android_bridge::{
    AndroidBridge, clear_android_bridge, get_android_bridge, set_android_bridge,
};
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_platform::paths::{clear_home_dir_override, set_home_dir_override};
use infiltrator_ports::profile_store::ProfileStore;
use mihomo_config::manager::ConfigManager;
use std::collections::BTreeMap;
use std::env::temp_dir;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_home() -> PathBuf {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    let home = temp_dir().join(format!("android-owner-operation-{id}"));
    fs::create_dir_all(&home).expect("create owner operation test home");
    home
}

async fn select_profile(manager: &ConfigManager<DefaultCredentialStore>, profile: &str) {
    let expected = ProfileStore::read_active_profile_reference(manager.engine_profile_store())
        .await
        .expect("read selection before test CAS");
    let target = ActiveProfileReferenceTarget::selected(
        ConfigManager::engine_id(manager),
        ProfileId::new(profile).expect("valid test profile ID"),
    );
    match ProfileStore::compare_and_set_active_profile_reference(
        manager.engine_profile_store(),
        expected,
        target,
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .expect("revisioned test selection CAS")
    {
        ActiveProfileReferenceCasOutcome::Committed { .. } => {}
        ActiveProfileReferenceCasOutcome::Stale { expected, observed } => {
            panic!("test selection unexpectedly stale: {expected:?} -> {observed:?}")
        }
    }
}

#[test]
fn owner_mutations_are_explicitly_busy_while_vpn_configuration_lease_is_active() {
    let mutation = AndroidOwnerOperation::ProfileCreate {
        name: "new-profile".to_owned(),
        url: "https://profiles.example.test/config.yaml".to_owned(),
    };

    assert_eq!(
        owner_operation_mutation_admission(
            &mutation,
            VpnConfigurationActivationStatus::BuilderAdmitted,
        ),
        OwnerOperationMutationAdmission::VpnConfigurationBusy,
    );
    assert_eq!(
        owner_operation_mutation_admission(
            &mutation,
            VpnConfigurationActivationStatus::DataPlaneStarting,
        ),
        OwnerOperationMutationAdmission::VpnConfigurationBusy,
    );
    assert_eq!(
        owner_operation_mutation_admission(
            &AndroidOwnerOperation::ProfilesList,
            VpnConfigurationActivationStatus::BuilderAdmitted,
        ),
        OwnerOperationMutationAdmission::ReadOnly,
    );
    assert_eq!(
        owner_operation_mutation_admission(&mutation, VpnConfigurationActivationStatus::Available,),
        OwnerOperationMutationAdmission::WriteAllowed,
    );
}

#[test]
fn busy_owner_mutation_keeps_a_typed_ipc_terminal() {
    let result = rejected_result(
        test_context(19),
        7,
        19,
        FfiStatus::err(
            FfiErrorCode::NotReady,
            "VPN configuration activation holds the owner-data revision fence",
        ),
        AndroidOwnerOperationIpcState::VpnConfigurationBusy,
    );

    assert_eq!(
        result.state,
        AndroidOwnerOperationIpcState::VpnConfigurationBusy
    );
    assert_eq!(result.status_code, FfiErrorCode::NotReady);
    let reply: AndroidOwnerOperationReply =
        serde_json::from_str(&result.outcome_json).expect("typed busy owner reply JSON");
    assert!(matches!(
        reply.readback,
        AndroidOwnerOperationReadback::Rejected(FfiStatus {
            code: FfiErrorCode::NotReady,
            ..
        })
    ));
}

#[tokio::test]
async fn owner_write_reports_busy_instead_of_waiting_for_revision_fence() {
    let _test_guard = TEST_LOCK.lock().await;
    let _revision_fence = owner_data_revision_fence().await;

    assert!(matches!(
        try_owner_data_revision_fence(),
        OwnerDataRevisionFenceAcquire::Busy,
    ));
}

fn test_context(expected_data_revision: u64) -> AndroidOwnerOperationContext {
    test_context_for_generation(0, expected_data_revision)
}

fn test_context_for_generation(
    expected_core_generation: u64,
    expected_data_revision: u64,
) -> AndroidOwnerOperationContext {
    AndroidOwnerOperationContext {
        client_session_id: "client-test".to_owned(),
        expected_owner_session_id: "owner-test".to_owned(),
        owner_session_id: "owner-test".to_owned(),
        expected_core_generation,
        expected_data_revision,
    }
}

#[test]
fn vpn_routing_activation_lease_is_revision_fenced_and_typed() {
    let mut state = AppRoutingActivationState::Available;
    assert_eq!(
        state.begin(6, 7, 40),
        AndroidVpnRoutingActivationResult::Stale {
            observed_routing_revision: 7,
        },
    );
    assert_eq!(state, AppRoutingActivationState::Available);

    let lease = AndroidVpnRoutingActivationLease {
        lease_id: 41,
        routing_revision: 7,
    };
    assert_eq!(
        state.begin(7, 7, lease.lease_id),
        AndroidVpnRoutingActivationResult::Acquired { lease },
    );
    assert!(state.blocks_mutations());
    assert_eq!(
        state.begin(7, 7, 42),
        AndroidVpnRoutingActivationResult::Busy,
    );
    assert_eq!(
        state.finish(lease.lease_id + 1),
        AndroidVpnRoutingActivationReleaseResult::LeaseMismatch,
    );
    assert_eq!(
        state.finish(lease.lease_id),
        AndroidVpnRoutingActivationReleaseResult::Released,
    );
    assert!(!state.blocks_mutations());
    assert_eq!(
        state.finish(lease.lease_id),
        AndroidVpnRoutingActivationReleaseResult::LeaseMissing,
    );
}

struct TestOwnerEnvironment {
    home: PathBuf,
    previous_bridge: Option<Arc<dyn AndroidBridge>>,
}

impl Drop for TestOwnerEnvironment {
    fn drop(&mut self) {
        clear_home_dir_override();
        let _ = fs::remove_dir_all(&self.home);
        clear_android_bridge();
        if let Some(bridge) = self.previous_bridge.take() {
            set_android_bridge(bridge);
        }
    }
}

#[test]
fn owner_operation_json_is_a_versioned_exhaustive_rust_enum() {
    let request = AndroidOwnerOperationRequest {
        protocol: AndroidOwnerOperationProtocolVersion::V6,
        operation: AndroidOwnerOperation::ProfileSubscriptionSave {
            name: "main".to_owned(),
            url: "https://example.invalid/sub".to_owned(),
            auto_update_enabled: true,
            update_interval_hours: Some(12),
        },
    };
    let encoded = serde_json::to_string(&request).expect("typed request serializes");
    assert_eq!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(&encoded)
            .expect("typed request decodes"),
        request,
    );
    assert!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(
            r#"{"protocol":"V6","operation":{"Unknown":{"name":"main"}}}"#,
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(
            r#"{"protocol":"V6","operation":"ProfilesList","action":"save"}"#,
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(
            r#"{"protocol":"V1","operation":"ProfilesList"}"#,
        )
        .is_err(),
        "the replaced V1 operation protocol is retired",
    );
    assert!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(
            r#"{"protocol":"V2","operation":"ProfilesList"}"#,
        )
        .is_err(),
        "the replaced V2 operation protocol is retired",
    );
    assert!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(
            r#"{"protocol":"V3","operation":"ProfilesList"}"#,
        )
        .is_err(),
        "the replaced V3 operation protocol is retired",
    );
    assert!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(
            r#"{"protocol":"V4","operation":"ProfilesList"}"#,
        )
        .is_err(),
        "the replaced V4 operation protocol is retired",
    );
    assert!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(
            r#"{"protocol":"V5","operation":"ProfilesList"}"#,
        )
        .is_err(),
        "the replaced V5 operation protocol is retired",
    );
    assert!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(
            r#"{"protocol":"V6","operation":{"ProfileSelect":{"name":"main"}}}"#,
        )
        .is_err(),
        "the replaced name-only ProfileSelect operation is retired",
    );
    assert!(serde_json::from_str::<AndroidOwnerOperationRequest>(
        r#"{"protocol":"V6","operation":{"ProfileSelect":{"target":{"engine":"mihomo","profile_id":"main","legacy_name":"main"}}}}"#,
    )
    .is_err());
    let app_routing_request = AndroidOwnerOperationRequest {
        protocol: AndroidOwnerOperationProtocolVersion::V6,
        operation: AndroidOwnerOperation::AppRoutingPackagesSet {
            packages: vec!["com.example.app".to_owned()],
            selection: AndroidAppRoutingPackageSelection::Selected,
        },
    };
    let app_routing_json =
        serde_json::to_string(&app_routing_request).expect("typed app-routing request serializes");
    assert_eq!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(&app_routing_json)
            .expect("typed app-routing request decodes"),
        app_routing_request,
    );
    let dns_request = AndroidOwnerOperationRequest {
        protocol: AndroidOwnerOperationProtocolVersion::V6,
        operation: AndroidOwnerOperation::DnsSettingsSave {
            patch: settings::DnsSettingsPatch {
                enable: Some(false),
                ipv6: Some(true),
                enhanced_mode: None,
                nameserver: Some(vec!["1.1.1.1".to_owned()]),
                default_nameserver: None,
                fallback: None,
                fallback_filter: None,
            },
        },
    };
    let dns_json = serde_json::to_string(&dns_request).expect("DNS DTO serializes");
    assert_eq!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(&dns_json).expect("DNS DTO decodes"),
        dns_request,
    );
    assert!(serde_json::from_str::<AndroidOwnerOperationRequest>(
        r#"{"protocol":"V6","operation":{"DnsSettingsSave":{"patch":{"enable":true,"unknown":1}}}}"#,
    )
    .is_err());
    let tun_request = AndroidOwnerOperationRequest {
        protocol: AndroidOwnerOperationProtocolVersion::V6,
        operation: AndroidOwnerOperation::TunSettingsSave {
            patch: VpnTunSettingsPatch {
                mtu: Some(1420),
                auto_route: Some(true),
                strict_route: Some(false),
                stack_update: VpnTunStackUpdate::Set {
                    stack: VpnTunStack::System,
                },
                auto_detect_interface: Some(true),
            },
        },
    };
    let tun_json = serde_json::to_string(&tun_request).expect("TUN DTO serializes");
    assert_eq!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(&tun_json)
            .expect("typed TUN DTO decodes"),
        tun_request,
    );
    let clear_tun_request = AndroidOwnerOperationRequest {
        protocol: AndroidOwnerOperationProtocolVersion::V6,
        operation: AndroidOwnerOperation::TunSettingsSave {
            patch: VpnTunSettingsPatch {
                mtu: None,
                auto_route: None,
                strict_route: None,
                stack_update: VpnTunStackUpdate::Clear,
                auto_detect_interface: None,
            },
        },
    };
    let clear_tun_json =
        serde_json::to_string(&clear_tun_request).expect("typed clear TUN update serializes");
    assert_eq!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(&clear_tun_json)
            .expect("typed clear TUN update decodes"),
        clear_tun_request,
    );
    assert!(serde_json::from_str::<AndroidOwnerOperationRequest>(
        r#"{"protocol":"V6","operation":{"TunSettingsSave":{"patch":{"mtu":1420,"dns_servers":["1.1.1.1"]}}}}"#,
    )
    .is_err());
    let rules_request = AndroidOwnerOperationRequest {
        protocol: AndroidOwnerOperationProtocolVersion::V6,
        operation: AndroidOwnerOperation::RulesSave {
            rules: vec![AndroidRuleEntry {
                rule: "MATCH,DIRECT".to_owned(),
                enabled: true,
            }],
        },
    };
    let rules_json = serde_json::to_string(&rules_request).expect("typed Rules DTO serializes");
    assert_eq!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(&rules_json)
            .expect("typed Rules DTO decodes"),
        rules_request,
    );
    assert!(serde_json::from_str::<AndroidOwnerOperationRequest>(
        r#"{"protocol":"V6","operation":{"RulesSave":{"rules":[{"rule":"MATCH,DIRECT","enabled":true,"unexpected":1}]}}}"#,
    )
    .is_err());
    assert!(serde_json::from_str::<AndroidOwnerOperationRequest>(
        r#"{"protocol":"V6","operation":{"RuleProvidersSave":{"providers":{},"unexpected":1}}}"#,
    )
    .is_err());
    let providers_request = AndroidOwnerOperationRequest {
        protocol: AndroidOwnerOperationProtocolVersion::V6,
        operation: AndroidOwnerOperation::RuleProvidersSave {
            providers: vec![AndroidRuleProviderEntry {
                name: "remote".to_owned(),
                configuration: AndroidJsonValue::ObjectValue {
                    value: vec![AndroidJsonEntry {
                        key: "type".to_owned(),
                        value: AndroidJsonValue::StringValue {
                            value: "http".to_owned(),
                        },
                    }],
                },
            }],
        },
    };
    let providers_json = serde_json::to_string(&providers_request)
        .expect("provider operation serializes as typed JSON");
    assert_eq!(
        serde_json::from_str::<AndroidOwnerOperationRequest>(&providers_json)
            .expect("provider operation decodes exhaustively"),
        providers_request,
    );
    assert!(serde_json::from_str::<AndroidOwnerOperationRequest>(
        r#"{"protocol":"V6","operation":{"RuleProvidersSave":{"providers":[{"name":"remote","configuration":{"kind":"StringValue","value":"http","extra":1}}]}}}"#,
    )
    .is_err());

    let webdav = WebDavSettings {
        enabled: true,
        url: "https://dav.example.invalid".to_owned(),
        username: "alice".to_owned(),
        password: "secret".to_owned(),
        sync_interval_mins: 30,
        sync_on_startup: false,
    };
    for operation in [
        AndroidOwnerOperation::WebDavSettings,
        AndroidOwnerOperation::WebDavSettingsSave {
            settings: webdav.clone(),
        },
        AndroidOwnerOperation::WebDavTest {
            settings: webdav.clone(),
        },
        AndroidOwnerOperation::WebDavSyncNow,
    ] {
        let request = AndroidOwnerOperationRequest {
            protocol: AndroidOwnerOperationProtocolVersion::V6,
            operation,
        };
        let encoded = serde_json::to_string(&request).expect("WebDAV operation serializes");
        assert_eq!(
            serde_json::from_str::<AndroidOwnerOperationRequest>(&encoded)
                .expect("WebDAV operation decodes"),
            request,
        );
    }

    for target in [
        AndroidProfileSelectionTarget::Mihomo {
            profile_id: "main".to_owned(),
        },
        AndroidProfileSelectionTarget::SingBox {
            profile_id: "mobile".to_owned(),
        },
    ] {
        let request = AndroidOwnerOperationRequest {
            protocol: AndroidOwnerOperationProtocolVersion::V6,
            operation: AndroidOwnerOperation::ProfileSelect { target },
        };
        let encoded = serde_json::to_string(&request).expect("engine target serializes");
        assert_eq!(
            serde_json::from_str::<AndroidOwnerOperationRequest>(&encoded)
                .expect("engine target decodes"),
            request,
        );
    }
}

#[test]
fn android_profile_target_resolves_a_validated_engine_scoped_reference() {
    use infiltrator_contract::active_profile_reference::ActiveProfileReferenceTarget;

    let target = AndroidProfileSelectionTarget::SingBox {
        profile_id: "mobile".to_owned(),
    }
    .into_reference_target()
    .expect("valid sing-box target");
    assert_eq!(
        target,
        ActiveProfileReferenceTarget::selected(
            EngineId::sing_box(),
            ProfileId::new("mobile").expect("valid profile ID"),
        ),
    );

    let error = AndroidProfileSelectionTarget::Mihomo {
        profile_id: "../outside".to_owned(),
    }
    .into_reference_target()
    .expect_err("profile path characters are rejected before dispatch");
    assert_eq!(error.code, FfiErrorCode::Config);
}

fn profile_selection_reply(observed: AndroidProfileSelectionTarget) -> AndroidOwnerOperationReply {
    AndroidOwnerOperationReply {
        protocol: AndroidOwnerOperationProtocolVersion::V6,
        context: test_context_for_generation(7, 10),
        observed_core_generation: 7,
        data_revision: 11,
        data_domain: Some(AndroidOwnerDataDomain::Profiles),
        snapshot: Some(AndroidOwnerDataSnapshot {
            domain: AndroidOwnerDataDomain::Profiles,
            data: AndroidOwnerDataSnapshotData::Profiles(ProfilesResult {
                status: FfiStatus::ok(),
                profiles: Vec::new(),
            }),
        }),
        readback: AndroidOwnerOperationReadback::ProfileSelection(
            AndroidProfileSelectionReadback {
                operation_status: FfiStatus::ok(),
                active_reference: AndroidActiveProfileReferenceObservation::Selected {
                    target: observed,
                    revision: 4,
                },
            },
        ),
    }
}

#[test]
fn profile_selection_success_requires_exact_engine_scoped_owner_readback() {
    let target = AndroidProfileSelectionTarget::SingBox {
        profile_id: "mobile".to_owned(),
    };
    let operation = AndroidOwnerOperation::ProfileSelect {
        target: target.clone(),
    };
    let matching = profile_selection_reply(target.clone());
    let accepted = validate_owner_operation_reply(&operation, matching)
        .expect("exact target owner readback is accepted");
    assert_eq!(
        accepted.readback,
        AndroidOwnerOperationReadback::ProfileSelection(AndroidProfileSelectionReadback {
            operation_status: FfiStatus::ok(),
            active_reference: AndroidActiveProfileReferenceObservation::Selected {
                target: target.clone(),
                revision: 4,
            },
        }),
    );

    let mismatching = profile_selection_reply(AndroidProfileSelectionTarget::Mihomo {
        profile_id: "mobile".to_owned(),
    });
    assert_eq!(
        validate_owner_operation_reply(&operation, mismatching)
            .expect_err("a different active engine cannot confirm selection")
            .code,
        FfiErrorCode::NotReady,
    );
}

#[test]
fn read_operations_do_not_advance_the_persisted_data_revision() {
    assert_eq!(
        AndroidOwnerOperation::ProfilesList.data_mutation(),
        AndroidOwnerDataMutation::ReadOnly
    );
    assert!(
        AndroidOwnerOperation::ProfileDetail {
            name: "main".to_owned()
        }
        .data_mutation()
            == AndroidOwnerDataMutation::ReadOnly
    );
    assert_eq!(
        AndroidOwnerOperation::ProfileSelect {
            target: AndroidProfileSelectionTarget::Mihomo {
                profile_id: "main".to_owned(),
            }
        }
        .data_mutation(),
        AndroidOwnerDataMutation::CommitRegardlessOfOutcome
    );
    assert_eq!(
        AndroidOwnerOperation::DnsSettings.data_mutation(),
        AndroidOwnerDataMutation::ReadOnly
    );
    assert_eq!(
        AndroidOwnerOperation::WebDavSettings.data_mutation(),
        AndroidOwnerDataMutation::ReadOnly
    );
    assert_eq!(
        AndroidOwnerOperation::WebDavTest {
            settings: WebDavSettings {
                enabled: false,
                url: String::new(),
                username: String::new(),
                password: String::new(),
                sync_interval_mins: 60,
                sync_on_startup: false,
            }
        }
        .data_mutation(),
        AndroidOwnerDataMutation::ReadOnly
    );
    assert_eq!(
        AndroidOwnerOperation::WebDavSyncNow.data_mutation(),
        AndroidOwnerDataMutation::ReadOnly
    );
    let webdav_save = AndroidOwnerOperation::WebDavSettingsSave {
        settings: WebDavSettings {
            enabled: false,
            url: String::new(),
            username: String::new(),
            password: String::new(),
            sync_interval_mins: 60,
            sync_on_startup: false,
        },
    };
    assert_eq!(
        webdav_save.data_mutation(),
        AndroidOwnerDataMutation::CommitRegardlessOfOutcome
    );
    assert_eq!(webdav_save.data_domain(), AndroidOwnerDataDomain::WebDav);
    let failed_webdav_write = FfiStatus::err(FfiErrorCode::InvalidInput, "invalid WebDAV settings");
    assert_eq!(
        owner_revision_transition(
            webdav_save.data_mutation(),
            &AndroidOwnerOperationReadback::WebDavSettings(WebDavSettingsResult {
                status: failed_webdav_write.clone(),
                settings: None,
            }),
            failed_webdav_write.code,
        ),
        AndroidOwnerRevisionTransition::Advance
    );
    assert_eq!(
        AndroidOwnerOperation::RulesList.data_mutation(),
        AndroidOwnerDataMutation::ReadOnly
    );
    assert_eq!(
        AndroidOwnerOperation::RuleProviders.data_mutation(),
        AndroidOwnerDataMutation::ReadOnly
    );
    assert_eq!(
        AndroidOwnerOperation::DnsSettingsSave {
            patch: settings::DnsSettingsPatch {
                enable: Some(false),
                ipv6: None,
                enhanced_mode: None,
                nameserver: None,
                default_nameserver: None,
                fallback: None,
                fallback_filter: None,
            },
        }
        .data_mutation(),
        AndroidOwnerDataMutation::CommitOnSuccess
    );
}

#[test]
fn rules_requests_are_fenced_by_core_generation_and_profile_data_revision() {
    let context = test_context_for_generation(4, 17);
    assert!(owner_data_fence_matches(&context, 4, 17));
    assert!(!owner_data_fence_matches(&context, 5, 17));
    assert!(!owner_data_fence_matches(&context, 4, 18));
}

#[test]
fn failed_dns_or_fake_ip_mutations_leave_the_data_revision_unchanged() {
    let failure = FfiStatus::err(FfiErrorCode::InvalidInput, "invalid patch");
    assert_eq!(
        owner_revision_transition(
            AndroidOwnerDataMutation::CommitOnSuccess,
            &AndroidOwnerOperationReadback::DnsSettings(settings::DnsSettingsResult {
                status: failure.clone(),
                settings: None,
            }),
            failure.code,
        ),
        AndroidOwnerRevisionTransition::Unchanged
    );
    assert_eq!(
        owner_revision_transition(
            AndroidOwnerDataMutation::CommitOnSuccess,
            &AndroidOwnerOperationReadback::FakeIpSettings(settings::FakeIpSettingsResult {
                status: failure.clone(),
                settings: None,
            },),
            failure.code,
        ),
        AndroidOwnerRevisionTransition::Unchanged
    );
    assert_eq!(
        owner_revision_transition(
            AndroidOwnerDataMutation::CommitOnSuccess,
            &AndroidOwnerOperationReadback::FakeIpCacheClear(FfiBoolResult {
                status: FfiStatus::ok(),
                value: false,
            }),
            FfiErrorCode::Ok,
        ),
        AndroidOwnerRevisionTransition::Unchanged
    );
    assert_eq!(
        owner_revision_transition(
            AndroidOwnerDataMutation::CommitOnSuccess,
            &AndroidOwnerOperationReadback::FakeIpCacheClear(FfiBoolResult {
                status: FfiStatus::ok(),
                value: true,
            }),
            FfiErrorCode::Ok,
        ),
        AndroidOwnerRevisionTransition::Advance
    );
}

#[path = "owner_operation_tests/dispatch.rs"]
mod dispatch;
