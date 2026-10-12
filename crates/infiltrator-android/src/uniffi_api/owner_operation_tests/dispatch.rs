use super::*;
use crate::uniffi_api::{VpnTunSettings, VpnTunStack, VpnTunStackUpdate};

#[tokio::test]
async fn unknown_operation_is_rejected_before_owner_data_revision_changes() {
    let before = android_owner_data_revision();
    let result = android_dispatch_owner_operation_wire_json(
        r#"{"protocol":"V5","operation":{"Unknown":null}}"#.to_owned(),
        "client-a".to_owned(),
        "owner-a".to_owned(),
        "owner-a".to_owned(),
        0,
        before,
    )
    .await;

    assert_eq!(result.state, AndroidOwnerOperationIpcState::Failed);
    assert_eq!(result.status_code, FfiErrorCode::InvalidInput);
    assert_eq!(android_owner_data_revision(), before);
}

#[tokio::test]
async fn typed_client_fails_closed_without_messenger_instead_of_running_local_rules() {
    let _guard = TEST_LOCK.lock().await;
    let previous_bridge = get_android_bridge();
    clear_android_bridge();
    let home = test_home();
    let _environment = TestOwnerEnvironment {
        home: home.clone(),
        previous_bridge,
    };
    set_home_dir_override(home);

    let result = android_execute_owner_operation(AndroidOwnerOperation::RulesList).await;
    assert!(matches!(
        result,
        AndroidOwnerOperationClientResult::Failed { status }
            if status.code == FfiErrorCode::NotReady
    ));
}

#[tokio::test]
async fn dns_and_fake_ip_owner_dispatches_return_written_configuration_readback() {
    let _guard = TEST_LOCK.lock().await;
    let previous_bridge = get_android_bridge();
    clear_android_bridge();
    let home = test_home();
    let _environment = TestOwnerEnvironment {
        home: home.clone(),
        previous_bridge,
    };
    set_home_dir_override(home.clone());

    let manager =
        ConfigManager::with_home_and_store(home.clone(), DefaultCredentialStore::default())
            .expect("config manager uses the isolated owner home");
    manager
        .save_inactive_profile(
            "main",
            "mode: rule\ndns:\n  enable: true\n  enhanced-mode: redir-host\n  nameserver:\n    - 1.1.1.1\n  fake-ip-range: 198.18.0.1/16\n",
        )
        .await
        .expect("create the active profile for the owner store");
    super::select_profile(&manager, "main").await;

    let original_revision = android_owner_data_revision();
    let dns_result = execute_operation(
        AndroidOwnerOperation::DnsSettings,
        test_context(original_revision),
        0,
    )
    .await;
    let dns_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&dns_result.outcome_json).expect("typed DNS readback decodes");
    assert_eq!(
        dns_reply.data_domain,
        Some(AndroidOwnerDataDomain::DnsSettings)
    );
    assert!(matches!(
        dns_reply.readback,
        AndroidOwnerOperationReadback::DnsSettings(DnsSettingsResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            settings: Some(DnsSettings { ref nameserver, .. }),
        }) if nameserver == &["1.1.1.1".to_owned()]
    ));
    assert_eq!(dns_reply.data_revision, original_revision);

    let original_profile = manager
        .load("main")
        .await
        .expect("profile is readable before the invalid operation");
    let invalid_dns = execute_operation(
        AndroidOwnerOperation::DnsSettingsSave {
            patch: settings::DnsSettingsPatch {
                enable: None,
                ipv6: None,
                enhanced_mode: Some("not-a-mode".to_owned()),
                nameserver: None,
                default_nameserver: None,
                fallback: None,
                fallback_filter: None,
            },
        },
        test_context(original_revision),
        0,
    )
    .await;
    assert_eq!(android_owner_data_revision(), original_revision);
    assert_eq!(
        manager
            .load("main")
            .await
            .expect("profile remains readable"),
        original_profile,
        "invalid DNS writes must not alter the profile",
    );
    let invalid_dns_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&invalid_dns.outcome_json)
            .expect("failed DNS operation still has typed owner readback");
    assert!(matches!(
        invalid_dns_reply.readback,
        AndroidOwnerOperationReadback::DnsSettings(DnsSettingsResult {
            status: FfiStatus { code, .. },
            settings: None,
        })
        if code != FfiErrorCode::Ok
    ));
    assert!(matches!(
        invalid_dns_reply.snapshot,
        Some(AndroidOwnerDataSnapshot {
            data: AndroidOwnerDataSnapshotData::DnsSettings(DnsSettingsResult {
                status: FfiStatus { code: FfiErrorCode::Ok, .. },
                settings: Some(DnsSettings { ref nameserver, .. }),
            }),
            ..
        }) if nameserver == &["1.1.1.1".to_owned()]
    ));

    let dns_saved = execute_operation(
        AndroidOwnerOperation::DnsSettingsSave {
            patch: settings::DnsSettingsPatch {
                enable: None,
                ipv6: Some(false),
                enhanced_mode: None,
                nameserver: Some(vec!["8.8.8.8".to_owned()]),
                default_nameserver: None,
                fallback: None,
                fallback_filter: None,
            },
        },
        test_context(original_revision),
        0,
    )
    .await;
    let dns_saved_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&dns_saved.outcome_json).expect("DNS save readback decodes");
    assert_eq!(dns_saved_reply.data_revision, original_revision + 1);
    assert!(matches!(
        dns_saved_reply.readback,
        AndroidOwnerOperationReadback::DnsSettings(DnsSettingsResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            settings: Some(DnsSettings { ref nameserver, ipv6: Some(false), .. }),
        }) if nameserver == &["8.8.8.8".to_owned()]
    ));

    let fake_ip_revision = android_owner_data_revision();
    let fake_ip_read = execute_operation(
        AndroidOwnerOperation::FakeIpSettings,
        test_context(fake_ip_revision),
        0,
    )
    .await;
    let fake_ip_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&fake_ip_read.outcome_json).expect("typed Fake-IP readback decodes");
    assert_eq!(
        fake_ip_reply.data_domain,
        Some(AndroidOwnerDataDomain::FakeIpSettings)
    );
    assert!(matches!(
        fake_ip_reply.readback,
        AndroidOwnerOperationReadback::FakeIpSettings(FakeIpSettingsResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            settings: Some(FakeIpSettings { fake_ip_range: Some(ref range), .. }),
        }) if range == "198.18.0.1/16"
    ));

    let invalid_fake_ip = execute_operation(
        AndroidOwnerOperation::FakeIpSettingsSave {
            patch: settings::FakeIpSettingsPatch {
                fake_ip_range: Some("invalid-range".to_owned()),
                fake_ip_filter: None,
                store_fake_ip: None,
            },
        },
        test_context(fake_ip_revision),
        0,
    )
    .await;
    let invalid_fake_ip_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&invalid_fake_ip.outcome_json)
            .expect("failed Fake-IP operation keeps typed readback");
    assert_eq!(android_owner_data_revision(), fake_ip_revision);
    assert!(matches!(
        invalid_fake_ip_reply.readback,
        AndroidOwnerOperationReadback::FakeIpSettings(FakeIpSettingsResult {
            status: FfiStatus { code, .. },
            settings: None,
        }) if code != FfiErrorCode::Ok
    ));
    assert!(matches!(
        invalid_fake_ip_reply.snapshot,
        Some(AndroidOwnerDataSnapshot {
            data: AndroidOwnerDataSnapshotData::FakeIpSettings(FakeIpSettingsResult {
                status: FfiStatus { code: FfiErrorCode::Ok, .. },
                settings: Some(FakeIpSettings { fake_ip_range: Some(ref range), .. }),
            }),
            ..
        }) if range == "198.18.0.1/16"
    ));

    let fake_ip_saved = execute_operation(
        AndroidOwnerOperation::FakeIpSettingsSave {
            patch: settings::FakeIpSettingsPatch {
                fake_ip_range: Some("198.19.0.1/16".to_owned()),
                fake_ip_filter: None,
                store_fake_ip: None,
            },
        },
        test_context(fake_ip_revision),
        0,
    )
    .await;
    let fake_ip_saved_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&fake_ip_saved.outcome_json).expect("Fake-IP save readback decodes");
    assert_eq!(fake_ip_saved_reply.data_revision, fake_ip_revision + 1);
    assert!(matches!(
        fake_ip_saved_reply.readback,
        AndroidOwnerOperationReadback::FakeIpSettings(FakeIpSettingsResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            settings: Some(FakeIpSettings { fake_ip_range: Some(ref range), .. }),
        }) if range == "198.19.0.1/16"
    ));
}

#[tokio::test]
async fn tun_settings_are_read_and_written_only_through_owner_revisioned_operations() {
    let _guard = TEST_LOCK.lock().await;
    let previous_bridge = get_android_bridge();
    clear_android_bridge();
    let home = test_home();
    let _environment = TestOwnerEnvironment {
        home: home.clone(),
        previous_bridge,
    };
    set_home_dir_override(home.clone());

    let manager =
        ConfigManager::with_home_and_store(home.clone(), DefaultCredentialStore::default())
            .expect("config manager uses the isolated owner home");
    manager
        .save_inactive_profile(
            "main",
            "mode: rule\ntun:\n  enable: true\n  stack: system\n  auto-route: true\n  strict-route: false\n  auto-detect-interface: true\n  mtu: 1500\n",
        )
        .await
        .expect("create an active profile with TUN settings");
    super::select_profile(&manager, "main").await;

    let original_revision = android_owner_data_revision();
    let read_operation = AndroidOwnerOperation::TunSettings;
    let read = execute_operation(read_operation.clone(), test_context(original_revision), 0).await;
    let read_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&read.outcome_json).expect("TUN owner readback decodes");
    let read_reply = validate_owner_operation_reply(&read_operation, read_reply)
        .expect("owner TUN readback carries its revisioned snapshot");
    assert_eq!(
        read_reply.data_domain,
        Some(AndroidOwnerDataDomain::TunSettings)
    );
    assert_eq!(read_reply.data_revision, original_revision);
    assert!(matches!(
        read_reply.readback,
        AndroidOwnerOperationReadback::TunSettings(VpnTunSettingsResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            settings: Some(VpnTunSettings { mtu: Some(1500), stack: Some(ref stack), .. }),
        }) if stack == "system"
    ));

    let write_operation = AndroidOwnerOperation::TunSettingsSave {
        patch: VpnTunSettingsPatch {
            mtu: Some(1420),
            auto_route: Some(false),
            strict_route: Some(true),
            stack_update: VpnTunStackUpdate::Set {
                stack: VpnTunStack::Gvisor,
            },
            auto_detect_interface: Some(false),
        },
    };
    let write =
        execute_operation(write_operation.clone(), test_context(original_revision), 0).await;
    let write_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&write.outcome_json).expect("TUN owner writeback decodes");
    let write_reply = validate_owner_operation_reply(&write_operation, write_reply)
        .expect("owner TUN writeback matches its typed snapshot");
    assert_eq!(write_reply.data_revision, original_revision + 1);
    assert!(matches!(
        write_reply.readback,
        AndroidOwnerOperationReadback::TunSettings(VpnTunSettingsResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            settings: Some(VpnTunSettings {
                mtu: Some(1420),
                auto_route: Some(false),
                strict_route: Some(true),
                stack: Some(ref stack),
                auto_detect_interface: Some(false),
            }),
        }) if stack == "gvisor"
    ));
}

#[tokio::test]
async fn app_routing_owner_operations_return_revisioned_readback_and_preserve_rules() {
    let _guard = TEST_LOCK.lock().await;
    let previous_bridge = get_android_bridge();
    clear_android_bridge();
    let home = test_home();
    let _environment = TestOwnerEnvironment {
        home: home.clone(),
        previous_bridge,
    };
    set_home_dir_override(home);

    let application =
        build_routing_application().expect("routing application uses the isolated owner home");
    application
        .set_rule("preserved.rule", AppRoutingRule::Direct)
        .expect("seed an independent explicit routing rule");

    let original_revision = android_owner_data_revision();
    let original_routing_revision = app_routing_revision();
    let read_operation = AndroidOwnerOperation::AppRoutingSnapshot;
    let read = execute_operation(read_operation.clone(), test_context(original_revision), 0).await;
    let read_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&read.outcome_json).expect("app-routing snapshot decodes");
    let read_reply = validate_owner_operation_reply(&read_operation, read_reply)
        .expect("owner readback matches the requested app-routing domain");
    assert_eq!(read_reply.data_revision, original_revision);
    assert_eq!(app_routing_revision(), original_routing_revision);
    assert!(matches!(
        read_reply.readback,
        AndroidOwnerOperationReadback::AppRouting(AndroidAppRoutingResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            snapshot: Some(AndroidAppRoutingSnapshot {
                mode: AndroidAppRoutingMode::ProxyAll,
                packages,
            }),
        }) if packages.is_empty()
    ));

    let mode_operation = AndroidOwnerOperation::AppRoutingModeSet {
        mode: AndroidAppRoutingMode::ProxySelected,
    };
    let mode_write =
        execute_operation(mode_operation.clone(), test_context(original_revision), 0).await;
    let mode_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&mode_write.outcome_json).expect("mode readback decodes");
    let mode_reply = validate_owner_operation_reply(&mode_operation, mode_reply)
        .expect("mode mutation carries an owner-authored snapshot");
    assert_eq!(mode_reply.data_revision, original_revision + 1);
    assert_eq!(app_routing_revision(), original_routing_revision + 1);
    assert!(matches!(
        mode_reply.readback,
        AndroidOwnerOperationReadback::AppRouting(AndroidAppRoutingResult {
            status: FfiStatus {
                code: FfiErrorCode::Ok,
                ..
            },
            snapshot: Some(AndroidAppRoutingSnapshot {
                mode: AndroidAppRoutingMode::ProxySelected,
                ..
            }),
        })
    ));

    let packages_operation = AndroidOwnerOperation::AppRoutingPackagesSet {
        packages: vec![
            "com.zeta.app".to_owned(),
            "com.alpha.app".to_owned(),
            "com.alpha.app".to_owned(),
        ],
        selection: AndroidAppRoutingPackageSelection::Selected,
    };
    let packages_write = execute_operation(
        packages_operation.clone(),
        test_context(original_revision + 1),
        0,
    )
    .await;
    let packages_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&packages_write.outcome_json)
            .expect("package selection readback decodes");
    let packages_reply = validate_owner_operation_reply(&packages_operation, packages_reply)
        .expect("package selection returns its canonical owner snapshot");
    assert_eq!(packages_reply.data_revision, original_revision + 2);
    assert_eq!(app_routing_revision(), original_routing_revision + 2);
    assert!(matches!(
        packages_reply.readback,
        AndroidOwnerOperationReadback::AppRouting(AndroidAppRoutingResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            snapshot: Some(AndroidAppRoutingSnapshot { packages, .. }),
        }) if packages == ["com.alpha.app", "com.zeta.app"]
    ));
    let saved_config = application.load().expect("selected packages persist");
    assert_eq!(
        saved_config.rules.get("preserved.rule"),
        Some(&AppRoutingRule::Direct),
    );

    let unselect_operation = AndroidOwnerOperation::AppRoutingPackagesSet {
        packages: vec!["com.alpha.app".to_owned()],
        selection: AndroidAppRoutingPackageSelection::Unselected,
    };
    let unselect_write = execute_operation(
        unselect_operation.clone(),
        test_context(original_revision + 2),
        0,
    )
    .await;
    let unselect_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&unselect_write.outcome_json).expect("unselection readback decodes");
    let unselect_reply = validate_owner_operation_reply(&unselect_operation, unselect_reply)
        .expect("unselection reports its persisted state");
    assert_eq!(unselect_reply.data_revision, original_revision + 3);
    assert_eq!(app_routing_revision(), original_routing_revision + 3);
    assert!(matches!(
        unselect_reply.readback,
        AndroidOwnerOperationReadback::AppRouting(AndroidAppRoutingResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            snapshot: Some(AndroidAppRoutingSnapshot { packages, .. }),
        }) if packages == ["com.zeta.app"]
    ));

    let lease = match begin_vpn_routing_activation(app_routing_revision()) {
        AndroidVpnRoutingActivationResult::Acquired { lease } => lease,
        result => panic!("expected current routing revision lease, got {result:?}"),
    };
    let blocked_revision = android_owner_data_revision();
    let route_revision_while_leased = app_routing_revision();
    let blocked_operation = AndroidOwnerOperation::AppRoutingModeSet {
        mode: AndroidAppRoutingMode::BypassSelected,
    };
    let blocked_reply =
        execute_operation(blocked_operation.clone(), test_context(blocked_revision), 0).await;
    let blocked_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&blocked_reply.outcome_json)
            .expect("blocked route mutation has a typed readback");
    assert!(matches!(
        blocked_reply.readback,
        AndroidOwnerOperationReadback::AppRouting(AndroidAppRoutingResult {
            status: FfiStatus {
                code: FfiErrorCode::NotReady,
                ..
            },
            snapshot: None,
        })
    ));
    assert_eq!(app_routing_revision(), route_revision_while_leased);
    assert_eq!(
        application
            .load()
            .expect("leased routing remains readable")
            .mode,
        AppRoutingMode::ProxySelected,
    );
    assert_eq!(
        finish_vpn_routing_activation(lease.lease_id),
        AndroidVpnRoutingActivationReleaseResult::Released,
    );
}

#[tokio::test]
async fn rules_owner_dispatches_read_write_and_reject_invalid_data_without_side_effects() {
    let _guard = TEST_LOCK.lock().await;
    let previous_bridge = get_android_bridge();
    clear_android_bridge();
    let home = test_home();
    let _environment = TestOwnerEnvironment {
        home: home.clone(),
        previous_bridge,
    };
    set_home_dir_override(home.clone());

    let manager =
        ConfigManager::with_home_and_store(home.clone(), DefaultCredentialStore::default())
            .expect("config manager uses the isolated owner home");
    manager
        .save_inactive_profile(
            "main",
            "mode: rule\nrules:\n  - DOMAIN-SUFFIX,example.com,DIRECT\n  - MATCH,DIRECT\nrule-providers:\n  remote:\n    type: http\n    behavior: domain\n    url: https://example.invalid/rules.yaml\n    path: ./rules/remote.yaml\n    interval: 86400\n",
        )
        .await
        .expect("create the active profile for the owner store");
    super::select_profile(&manager, "main").await;

    let initial_revision = android_owner_data_revision();
    let rules_operation = AndroidOwnerOperation::RulesList;
    let rules_read =
        execute_operation(rules_operation.clone(), test_context(initial_revision), 0).await;
    let rules_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&rules_read.outcome_json).expect("Rules readback decodes");
    let rules_reply = validate_owner_operation_reply(&rules_operation, rules_reply)
        .expect("typed client accepts a fenced Rules readback");
    assert_eq!(rules_reply.data_domain, Some(AndroidOwnerDataDomain::Rules));
    assert_eq!(rules_reply.data_revision, initial_revision);
    assert!(matches!(
        rules_reply.readback,
        AndroidOwnerOperationReadback::Rules(AndroidRulesResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            ref rules,
        }) if rules.len() == 2 && rules[0].rule == "DOMAIN-SUFFIX,example.com,DIRECT"
    ));

    let original_profile = manager
        .load("main")
        .await
        .expect("profile is readable before the invalid Rules operation");
    let invalid_rules_operation = AndroidOwnerOperation::RulesSave {
        rules: vec![AndroidRuleEntry {
            rule: "   ".to_owned(),
            enabled: true,
        }],
    };
    let invalid_rules = execute_operation(
        invalid_rules_operation.clone(),
        test_context(initial_revision),
        0,
    )
    .await;
    let invalid_rules_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&invalid_rules.outcome_json)
            .expect("failed Rules operation keeps a typed readback");
    let invalid_rules_reply =
        validate_owner_operation_reply(&invalid_rules_operation, invalid_rules_reply)
            .expect("typed client preserves a failed but well-fenced Rules result");
    assert_eq!(android_owner_data_revision(), initial_revision);
    assert_eq!(
        manager
            .load("main")
            .await
            .expect("profile remains readable"),
        original_profile,
        "invalid Rules writes must not alter the profile",
    );
    assert!(matches!(
        invalid_rules_reply.readback,
        AndroidOwnerOperationReadback::Rules(AndroidRulesResult {
            status: FfiStatus { code, .. },
            rules,
        }) if code != FfiErrorCode::Ok && rules.is_empty()
    ));
    assert!(matches!(
        invalid_rules_reply.snapshot,
        Some(AndroidOwnerDataSnapshot {
            data: AndroidOwnerDataSnapshotData::Rules(AndroidRulesResult {
                status: FfiStatus { code: FfiErrorCode::Ok, .. },
                rules,
            }),
            ..
        }) if rules.len() == 2
    ));

    let replacement = vec![
        AndroidRuleEntry {
            rule: "DOMAIN,blocked.example,DIRECT".to_owned(),
            enabled: false,
        },
        AndroidRuleEntry {
            rule: "MATCH,DIRECT".to_owned(),
            enabled: true,
        },
    ];
    let rules_save_operation = AndroidOwnerOperation::RulesSave {
        rules: replacement.clone(),
    };
    let rules_saved = execute_operation(
        rules_save_operation.clone(),
        test_context(initial_revision),
        0,
    )
    .await;
    let rules_saved_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&rules_saved.outcome_json).expect("Rules save readback decodes");
    let rules_saved_reply =
        validate_owner_operation_reply(&rules_save_operation, rules_saved_reply)
            .expect("typed client accepts a fenced Rules save readback");
    assert_eq!(rules_saved_reply.data_revision, initial_revision + 1);
    assert!(matches!(
        rules_saved_reply.readback,
        AndroidOwnerOperationReadback::Rules(AndroidRulesResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            rules,
        }) if rules == replacement
    ));
    assert!(matches!(
        rules_saved_reply.snapshot,
        Some(AndroidOwnerDataSnapshot {
            domain: AndroidOwnerDataDomain::Rules,
            data: AndroidOwnerDataSnapshotData::Rules(AndroidRulesResult {
                status: FfiStatus { code: FfiErrorCode::Ok, .. },
                rules,
            }),
        }) if rules == replacement
    ));

    let provider_revision = android_owner_data_revision();
    let providers_read = execute_operation(
        AndroidOwnerOperation::RuleProviders,
        test_context(provider_revision),
        0,
    )
    .await;
    let providers_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&providers_read.outcome_json)
            .expect("rule providers readback decodes");
    assert_eq!(
        providers_reply.data_domain,
        Some(AndroidOwnerDataDomain::RuleProviders)
    );
    let profile_before_invalid_provider = manager
        .load("main")
        .await
        .expect("profile is readable before invalid provider input");
    let invalid_provider_operation = AndroidOwnerOperation::RuleProvidersSave {
        providers: vec![AndroidRuleProviderEntry {
            name: "remote".to_owned(),
            configuration: AndroidJsonValue::NumberValue {
                value: "not-a-number".to_owned(),
            },
        }],
    };
    let invalid_provider = execute_operation(
        invalid_provider_operation.clone(),
        test_context(provider_revision),
        0,
    )
    .await;
    let invalid_provider_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&invalid_provider.outcome_json)
            .expect("invalid provider result keeps a typed owner readback");
    let invalid_provider_reply =
        validate_owner_operation_reply(&invalid_provider_operation, invalid_provider_reply)
            .expect("typed client preserves a failed provider result without mutation");
    assert_eq!(android_owner_data_revision(), provider_revision);
    assert_eq!(
        manager
            .load("main")
            .await
            .expect("profile remains readable"),
        profile_before_invalid_provider,
        "invalid provider data must not alter the profile",
    );
    assert!(matches!(
        invalid_provider_reply.readback,
        AndroidOwnerOperationReadback::RuleProviders(AndroidRuleProvidersResult {
            status: FfiStatus { code: FfiErrorCode::InvalidInput, .. },
            providers,
        }) if providers.is_empty()
    ));
    assert!(matches!(
        invalid_provider_reply.snapshot,
        Some(AndroidOwnerDataSnapshot {
            data: AndroidOwnerDataSnapshotData::RuleProviders(AndroidRuleProvidersResult {
                status: FfiStatus { code: FfiErrorCode::Ok, .. },
                providers,
            }),
            ..
        }) if !providers.is_empty()
    ));
    let providers_value = serde_json::json!({
        "remote": {
            "type": "http",
            "behavior": "domain",
            "url": "https://example.invalid/new.yaml",
            "path": "./rules/new.yaml",
            "interval": 43200
        }
    });
    let provider_map =
        serde_json::from_value::<BTreeMap<String, serde_json::Value>>(providers_value)
            .expect("provider fixture is a name-to-record map");
    let providers = rules::rule_provider_map_to_entries(provider_map);
    let providers_save_operation = AndroidOwnerOperation::RuleProvidersSave {
        providers: providers.clone(),
    };
    let providers_saved = execute_operation(
        providers_save_operation.clone(),
        test_context(provider_revision),
        0,
    )
    .await;
    let providers_saved_reply: AndroidOwnerOperationReply =
        serde_json::from_str(&providers_saved.outcome_json)
            .expect("provider save readback decodes");
    let providers_saved_reply =
        validate_owner_operation_reply(&providers_save_operation, providers_saved_reply)
            .expect("typed client accepts a fenced provider save readback");
    assert_eq!(providers_saved_reply.data_revision, provider_revision + 1);
    assert!(matches!(
        providers_saved_reply.readback,
        AndroidOwnerOperationReadback::RuleProviders(AndroidRuleProvidersResult {
            status: FfiStatus { code: FfiErrorCode::Ok, .. },
            providers: ref actual,
        }) if actual == &providers
    ));
    assert!(matches!(
        providers_saved_reply.snapshot,
        Some(AndroidOwnerDataSnapshot {
            domain: AndroidOwnerDataDomain::RuleProviders,
            data: AndroidOwnerDataSnapshotData::RuleProviders(AndroidRuleProvidersResult {
                status: FfiStatus { code: FfiErrorCode::Ok, .. },
                providers: actual,
            }),
        }) if actual == providers
    ));
}
