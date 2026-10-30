use super::*;

#[test]
fn owner_managed_lifecycle_still_requires_an_owner_composed_port() {
    let operation = CoreOperation::Lifecycle(CoreLifecycleOperation::Start);
    let provider = MihomoCoreCapabilityProvider::new_for_target(
        HostKind::Desktop,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        [],
    );
    let engine_id = EngineId::new("mihomo").expect("valid engine ID");
    let matrix = provider
        .matrix_for(&engine_id, None)
        .expect("Mihomo lifecycle capability matrix");
    let resolution = matrix.resolve(CoreCapabilityKey::new(matrix.scope.clone(), operation));

    assert_eq!(
        resolution.engine,
        Some(EngineCapabilityDecision::OwnerManaged {
            owner: AdapterOperationOwner::CoreLifecycle,
        })
    );
    assert!(matches!(
        resolution.adapter,
        Some(AdapterCapabilityDecision::AdapterMissing { .. })
    ));
    assert!(matches!(
        resolution.status,
        CoreCapabilityStatus::AdapterMissing { .. }
    ));
}

#[test]
fn linux_real_process_evidence_does_not_authorize_android_or_ios_operations() {
    let version = CoreVersion::new("v1.19.33").expect("valid version");
    for (host, target) in [
        (HostKind::Android, EngineBinaryTarget::Aarch64LinuxAndroid),
        (HostKind::Ios, EngineBinaryTarget::Aarch64AppleIos),
    ] {
        let provider = MihomoCoreCapabilityProvider::new_for_target(
            host,
            target,
            CoreOperation::all().iter().copied(),
        );
        let engine_id = EngineId::new("mihomo").expect("valid engine ID");
        let matrix = provider
            .matrix_for(&engine_id, Some(&version))
            .expect("Mihomo capability matrix");
        let resolution = matrix.resolve(CoreCapabilityKey::new(
            matrix.scope.clone(),
            CoreOperation::Outbound(OutboundOperation::Probe),
        ));
        assert!(
            matches!(
                resolution.status,
                CoreCapabilityStatus::HostUnavailable { .. }
            ),
            "{host:?} cannot inherit x86_64-unknown-linux-gnu process evidence"
        );
    }
}

#[test]
fn mismatched_host_target_and_adapter_revision_fail_closed() {
    let version = CoreVersion::new("v1.19.33").expect("valid version");
    let cases = [
        (
            HostKind::Android,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(2),
        ),
        (
            HostKind::Desktop,
            EngineBinaryTarget::Aarch64LinuxAndroid,
            AdapterRevision::new(2),
        ),
        (
            HostKind::Desktop,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        (
            HostKind::Desktop,
            EngineBinaryTarget::Aarch64UnknownLinuxGnu,
            AdapterRevision::new(2),
        ),
    ];
    for (host, target, revision) in cases {
        let provider = MihomoCoreCapabilityProvider::new_with_scope(
            host,
            target,
            revision,
            CoreOperation::all().iter().copied(),
        );
        let engine_id = EngineId::new("mihomo").expect("valid engine ID");
        let matrix = provider
            .matrix_for(&engine_id, Some(&version))
            .expect("Mihomo capability matrix");
        assert_eq!(matrix.scope.binary_target, target);
        assert_eq!(matrix.scope.adapter_revision, revision);
        let resolution = matrix.resolve(CoreCapabilityKey::new(
            matrix.scope.clone(),
            CoreOperation::Outbound(OutboundOperation::List),
        ));
        assert!(
            matches!(
                resolution.status,
                CoreCapabilityStatus::HostUnavailable { .. }
            ),
            "{host:?}/{target:?}/revision {} must not reuse x86_64-unknown-linux-gnu revision 2 evidence",
            revision.value()
        );
    }
}

#[test]
fn capability_matrix_keeps_only_composition_audited_operations_available() {
    #[derive(Clone, Copy)]
    enum ExpectedStatus {
        Available,
        Unsupported,
        VersionUnavailable,
    }

    let provider = MihomoCoreCapabilityProvider::new_for_target(
        HostKind::Desktop,
        EngineBinaryTarget::X86_64UnknownLinuxGnu,
        CoreOperation::all().iter().copied(),
    );
    let engine_id = EngineId::new("mihomo").expect("valid engine ID");
    let version = CoreVersion::new("v1.19.33").expect("valid version");
    let matrix = provider
        .matrix_for(&engine_id, Some(&version))
        .expect("Mihomo capability matrix");
    let cases = [
        (
            CoreOperation::Outbound(OutboundOperation::List),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Outbound(OutboundOperation::Select),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Outbound(OutboundOperation::Probe),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Logs(LogsOperation::Stream),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Logs(LogsOperation::SetLevel),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
            ExpectedStatus::Available,
        ),
        (
            CoreOperation::Profile(ProfileOperation::Validate),
            ExpectedStatus::Unsupported,
        ),
        (
            CoreOperation::Profile(ProfileOperation::Apply),
            ExpectedStatus::Unsupported,
        ),
        (
            CoreOperation::Profile(ProfileOperation::ApplyFeature),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Dns(DnsOperation::Query),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Dns(DnsOperation::ApplyConfiguration),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Rules(RulesOperation::Trace),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Rules(RulesOperation::Apply),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Subscription(SubscriptionOperation::Refresh),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Subscription(SubscriptionOperation::Import),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Subscription(SubscriptionOperation::Update),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Geo(GeoOperation::UpdateDatabases),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Connection(ConnectionOperation::Close),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Provider(ProviderOperation::ListRule),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Provider(ProviderOperation::UpdateRule),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::CoreGarbageCollection(CoreGarbageCollectionOperation::Trigger),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Connection(ConnectionOperation::List),
            ExpectedStatus::Unsupported,
        ),
        (
            CoreOperation::Tun(TunOperation::Configure),
            ExpectedStatus::VersionUnavailable,
        ),
        (
            CoreOperation::Version(VersionOperation::Install),
            ExpectedStatus::Unsupported,
        ),
    ];

    for (operation, expected) in cases {
        let resolution = matrix.resolve(CoreCapabilityKey::new(matrix.scope.clone(), operation));
        match expected {
            ExpectedStatus::Available => assert!(
                matches!(resolution.status, CoreCapabilityStatus::Available { .. }),
                "{operation:?}"
            ),
            ExpectedStatus::Unsupported => assert!(
                matches!(resolution.status, CoreCapabilityStatus::Unsupported { .. }),
                "{operation:?}"
            ),
            ExpectedStatus::VersionUnavailable => assert!(
                matches!(
                    resolution.status,
                    CoreCapabilityStatus::VersionUnavailable { .. }
                ),
                "{operation:?}"
            ),
        }
    }
}

#[test]
fn active_version_independent_engine_operations_still_require_exact_target_evidence() {
    let operation = CoreOperation::Version(VersionOperation::Install);
    let declarations = version_install_declarations(AdapterOperationSupport::Supported {
        evidence_id: OperationEvidenceId::new("version.install").expect("evidence ID"),
    });

    assert_eq!(
        declarations.engine_decision_for_scope(
            operation,
            None,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        EngineCapabilityDecision::Supported
    );
    for (target, revision) in [
        (
            EngineBinaryTarget::Aarch64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        (
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(2),
        ),
    ] {
        assert!(matches!(
            declarations.engine_decision_for_scope(operation, None, target, revision),
            EngineCapabilityDecision::VersionUnavailable { .. }
        ));
    }
}

#[test]
fn evidence_open_is_never_available_for_a_version_independent_operation() {
    let operation = CoreOperation::Version(VersionOperation::Install);
    let declarations = version_install_declarations(AdapterOperationSupport::EvidenceOpen {
        reason: "candidate engine behavior is not verified".to_owned(),
    });

    assert!(matches!(
        declarations.engine_decision_for_scope(
            operation,
            None,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(1),
        ),
        EngineCapabilityDecision::VersionUnavailable { .. }
    ));
}

#[test]
fn locked_historical_observations_do_not_promote_evidence_open_operations() {
    let declarations = operation_declarations().expect("validated Mihomo declarations");
    let locked = locked_versions_and_operations();
    for (version, target, revision, operation_ids) in locked {
        assert_eq!(target, "x86_64-unknown-linux-gnu", "{version}");
        assert_eq!(revision, 2, "{version}");
        assert_eq!(operation_ids.len(), 24, "{version}");
        let core_version = CoreVersion::new(version.clone()).expect("locked version");
        let tun_operation = CoreOperation::Tun(TunOperation::Configure);
        assert!(
            operation_ids
                .iter()
                .any(|registered| registered == "tun.configure")
        );
        assert!(!declarations.has_verified_support_for_scope(
            tun_operation,
            &core_version,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(revision),
        ));
        let rule_trace_operation = CoreOperation::Rules(RulesOperation::Trace);
        assert!(
            operation_ids
                .iter()
                .any(|registered| registered == "rules.trace")
        );
        assert!(!declarations.has_verified_support_for_scope(
            rule_trace_operation,
            &core_version,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(revision),
        ));
        let profile_feature_operation = CoreOperation::Profile(ProfileOperation::ApplyFeature);
        assert!(
            operation_ids
                .iter()
                .any(|registered| registered == "profile.apply_feature")
        );
        assert!(!declarations.has_verified_support_for_scope(
            profile_feature_operation,
            &core_version,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(revision),
        ));
        for (operation, evidence_id) in [
            (
                CoreOperation::Subscription(SubscriptionOperation::Import),
                "subscription.import",
            ),
            (
                CoreOperation::Subscription(SubscriptionOperation::Update),
                "subscription.update",
            ),
            (
                CoreOperation::Subscription(SubscriptionOperation::Refresh),
                "subscription.refresh",
            ),
        ] {
            assert!(
                operation_ids
                    .iter()
                    .any(|registered| registered == evidence_id),
                "{version} must register {evidence_id}"
            );
            assert!(!declarations.has_verified_support_for_scope(
                operation,
                &core_version,
                EngineBinaryTarget::X86_64UnknownLinuxGnu,
                AdapterRevision::new(revision),
            ));
        }
        assert!(
            operation_ids
                .iter()
                .any(|registered| registered == "geo.update_databases"),
            "{version} must register geo.update_databases"
        );
        assert!(!declarations.has_verified_support_for_scope(
            CoreOperation::Geo(GeoOperation::UpdateDatabases),
            &core_version,
            EngineBinaryTarget::X86_64UnknownLinuxGnu,
            AdapterRevision::new(revision),
        ));
        for (operation, evidence_id) in [
            (
                CoreOperation::Profile(ProfileOperation::Validate),
                "profile.validate",
            ),
            (
                CoreOperation::Profile(ProfileOperation::Apply),
                "profile.apply",
            ),
        ] {
            assert!(
                operation_ids
                    .iter()
                    .any(|registered| registered == evidence_id)
            );
            assert!(!declarations.has_verified_support_for_scope(
                operation,
                &core_version,
                EngineBinaryTarget::X86_64UnknownLinuxGnu,
                AdapterRevision::new(revision),
            ));
            assert!(matches!(
                declarations.support(operation),
                AdapterOperationSupport::Unsupported { reason }
                    if reason.contains("CORE-013.4")
            ));
        }
    }
}

#[test]
fn canonical_mihomo_versions_require_exact_release_syntax() {
    for (version, _, _, _) in locked_versions_and_operations() {
        assert!(is_canonical_mihomo_version(&version), "{version}");
        assert!(!is_canonical_mihomo_version(&format!("{version}-rc.1")));
        assert!(!is_canonical_mihomo_version(&format!("{version}+build.1")));
        let components = version
            .strip_prefix('v')
            .expect("canonical version")
            .split('.')
            .collect::<Vec<_>>();
        assert!(!is_canonical_mihomo_version(&version[1..]));
        assert!(!is_canonical_mihomo_version(&format!(
            "v0{}.{}.{}",
            components[0], components[1], components[2]
        )));
        assert!(!is_canonical_mihomo_version(&format!(
            "v{}.{}.0{}",
            components[0], components[1], components[2]
        )));
        assert!(!is_canonical_mihomo_version(&format!(
            "v{}.{}",
            components[0], components[1]
        )));
        assert!(!is_canonical_mihomo_version(&format!("{version}.1")));
    }
}
