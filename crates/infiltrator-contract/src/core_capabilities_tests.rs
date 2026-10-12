use super::{
    AdapterCapabilityDecision, AdapterRevision, CapabilityAccess, CapabilityBlocker,
    CapabilityLayer, CapabilityMaturity, ConnectionOperation, CoreCapabilityEntry,
    CoreCapabilityKey, CoreCapabilityMatrix, CoreCapabilityMatrixError, CoreCapabilityScope,
    CoreCapabilitySnapshot, CoreCapabilityStatus, CoreGarbageCollectionOperation,
    CoreLifecycleOperation, CoreOperation, CoreVersionRequirement, DnsOperation,
    EngineBinaryTarget, EngineCapabilityDecision, FakeIpCacheOperation, GeoOperation,
    HostCapabilityDecision, LogsOperation, OutboundOperation, ProfileOperation, ProviderOperation,
    ProxyControlOperation, RulesOperation, SubscriptionOperation, TunOperation, VersionOperation,
};
use crate::adapter_operation_declaration::AdapterOperationOwner;
use crate::core_identity::{CoreVersion, EngineId};
use crate::surface::HostKind;

fn scope_for(engine_id: &str, version: Option<&str>) -> CoreCapabilityScope {
    CoreCapabilityScope::new_for_target(
        EngineId::new(engine_id).expect("valid engine identifier"),
        version.map(|value| CoreVersion::new(value).expect("valid core version")),
        HostKind::Android,
        EngineBinaryTarget::Aarch64LinuxAndroid,
        AdapterRevision::new(3),
    )
}

fn scope(version: Option<&str>) -> CoreCapabilityScope {
    scope_for("mihomo", version)
}

fn entry(
    operation: CoreOperation,
    engine: EngineCapabilityDecision,
    host: HostCapabilityDecision,
    adapter: AdapterCapabilityDecision,
) -> CoreCapabilityEntry {
    CoreCapabilityEntry::new(operation, engine, host, adapter)
}

fn key(scope: CoreCapabilityScope, operation: CoreOperation) -> CoreCapabilityKey {
    CoreCapabilityKey::new(scope, operation)
}

#[test]
fn binary_target_is_part_of_the_serialized_capability_scope() {
    let target_scope = CoreCapabilityScope::new_for_target(
        EngineId::new("mihomo").expect("valid engine id"),
        Some(CoreVersion::new("v1.19.32").expect("valid version")),
        HostKind::Android,
        EngineBinaryTarget::Aarch64LinuxAndroid,
        AdapterRevision::new(7),
    );
    let value = serde_json::to_value(&target_scope).expect("serialize typed target scope");
    assert_eq!(value["binary_target"], "aarch64-linux-android");
    assert_eq!(
        serde_json::from_value::<CoreCapabilityScope>(value).expect("roundtrip typed scope"),
        target_scope
    );
    assert_ne!(
        target_scope,
        CoreCapabilityScope::new_for_target(
            EngineId::new("mihomo").expect("valid engine id"),
            Some(CoreVersion::new("v1.19.32").expect("valid version")),
            HostKind::Android,
            EngineBinaryTarget::X86_64LinuxAndroid,
            AdapterRevision::new(7),
        ),
    );
}

#[test]
fn current_binary_target_uses_the_exact_cargo_target_triple() {
    let expected = EngineBinaryTarget::from_target_triple(env!("INFILTRATOR_BUILD_TARGET"))
        .unwrap_or(EngineBinaryTarget::Unregistered);
    assert_eq!(EngineBinaryTarget::current(), expected);
}

fn supported_entry(operation: CoreOperation) -> CoreCapabilityEntry {
    entry(
        operation,
        EngineCapabilityDecision::Supported,
        HostCapabilityDecision::Supported,
        AdapterCapabilityDecision::Supported,
    )
}

#[test]
fn core_operation_catalogue_covers_each_required_backend_area() {
    let operations = CoreOperation::all();
    assert!(operations.contains(&CoreOperation::Profile(ProfileOperation::Validate)));
    assert!(operations.contains(&CoreOperation::Dns(DnsOperation::Query)));
    assert!(operations.contains(&CoreOperation::Rules(RulesOperation::Trace)));
    assert!(operations.contains(&CoreOperation::Subscription(SubscriptionOperation::Refresh)));
    assert!(operations.contains(&CoreOperation::Provider(ProviderOperation::UpdateProxy)));
    assert!(operations.contains(&CoreOperation::Outbound(OutboundOperation::Probe)));
    assert!(operations.contains(&CoreOperation::Connection(ConnectionOperation::Close)));
    assert!(operations.contains(&CoreOperation::Logs(LogsOperation::Stream)));
    assert!(operations.contains(&CoreOperation::Geo(GeoOperation::UpdateDatabases)));
    assert!(operations.contains(&CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush)));
    assert!(operations.contains(&CoreOperation::Tun(TunOperation::Configure)));
    assert!(operations.contains(&CoreOperation::Lifecycle(CoreLifecycleOperation::Start)));
    assert!(operations.contains(&CoreOperation::Lifecycle(CoreLifecycleOperation::Stop)));
    assert!(operations.contains(&CoreOperation::Lifecycle(CoreLifecycleOperation::Restart)));
    assert!(operations.contains(&CoreOperation::ProxyControl(
        ProxyControlOperation::SetRoutingIntent
    )));
    assert!(operations.contains(&CoreOperation::CoreGarbageCollection(
        CoreGarbageCollectionOperation::Trigger
    )));
    assert!(operations.contains(&CoreOperation::Version(VersionOperation::Install)));
    assert!(operations.contains(&CoreOperation::Version(VersionOperation::Activate)));
    assert!(operations.contains(&CoreOperation::Version(VersionOperation::Uninstall)));
}

#[test]
fn proxy_control_wire_accepts_only_routing_intent_operations() {
    for retired_operation in ["start", "stop"] {
        let wire = serde_json::json!({
            "category": "proxy_control",
            "operation": retired_operation,
        });
        assert!(serde_json::from_value::<CoreOperation>(wire).is_err());
    }
}

#[test]
fn version_management_operations_do_not_require_an_active_core_version() {
    let operations = [
        CoreOperation::Version(VersionOperation::Inspect),
        CoreOperation::Version(VersionOperation::Install),
        CoreOperation::Version(VersionOperation::Activate),
        CoreOperation::Version(VersionOperation::Uninstall),
        CoreOperation::Version(VersionOperation::Rollback),
    ];
    let scope = scope(None);
    let entries = operations
        .into_iter()
        .map(supported_entry)
        .collect::<Vec<_>>();
    let matrix =
        CoreCapabilityMatrix::new(scope.clone(), entries).expect("version operations are unique");

    for operation in operations {
        assert_eq!(
            operation.version_requirement(),
            CoreVersionRequirement::IndependentOfActiveVersion
        );
        assert_eq!(
            matrix.resolve(key(scope.clone(), operation)).status,
            CoreCapabilityStatus::Available {
                access: CapabilityAccess::Supported,
                maturity: CapabilityMaturity::Stable,
            }
        );
    }
}

#[test]
fn engine_operations_require_an_active_version_by_default() {
    assert_eq!(
        CoreOperation::Profile(ProfileOperation::Validate).version_requirement(),
        CoreVersionRequirement::RequiresActiveVersion
    );
    assert_eq!(
        CoreOperation::FakeIpCache(FakeIpCacheOperation::Flush).version_requirement(),
        CoreVersionRequirement::RequiresActiveVersion
    );
    assert_eq!(
        CoreOperation::Lifecycle(CoreLifecycleOperation::Start).version_requirement(),
        CoreVersionRequirement::IndependentOfActiveVersion
    );
    assert_eq!(
        CoreOperation::Lifecycle(CoreLifecycleOperation::Stop).version_requirement(),
        CoreVersionRequirement::IndependentOfActiveVersion
    );
    assert_eq!(
        CoreOperation::Lifecycle(CoreLifecycleOperation::Restart).version_requirement(),
        CoreVersionRequirement::IndependentOfActiveVersion
    );
    assert_eq!(
        CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent).version_requirement(),
        CoreVersionRequirement::RequiresActiveVersion
    );
    assert_eq!(
        CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent).version_requirement(),
        CoreVersionRequirement::RequiresActiveVersion
    );
}

#[test]
fn lifecycle_stop_and_restart_remain_available_without_version() {
    let stop = CoreOperation::Lifecycle(CoreLifecycleOperation::Stop);
    let restart = CoreOperation::Lifecycle(CoreLifecycleOperation::Restart);
    let scope = scope(None);
    let matrix = CoreCapabilityMatrix::new(
        scope.clone(),
        vec![supported_entry(stop), supported_entry(restart)],
    )
    .expect("unique lifecycle operations");

    for operation in [stop, restart] {
        assert_eq!(
            matrix.resolve(key(scope.clone(), operation)).status,
            CoreCapabilityStatus::Available {
                access: CapabilityAccess::Supported,
                maturity: CapabilityMaturity::Stable,
            }
        );
    }
}

#[test]
fn routing_intent_requires_owner_reported_active_version() {
    let operations = [
        CoreOperation::ProxyControl(ProxyControlOperation::GetRoutingIntent),
        CoreOperation::ProxyControl(ProxyControlOperation::SetRoutingIntent),
    ];
    let scope = scope(None);
    let matrix = CoreCapabilityMatrix::new(
        scope.clone(),
        operations.into_iter().map(supported_entry).collect(),
    )
    .expect("unique proxy operations");

    for operation in operations {
        assert!(matches!(
            matrix.resolve(key(scope.clone(), operation)).status,
            CoreCapabilityStatus::VersionUnavailable { blockers }
                if blockers.iter().any(|blocker| matches!(
                    blocker,
                    CapabilityBlocker::VersionUnavailable { .. }
                ))
        ));
    }
}

#[test]
fn supported_decisions_resolve_to_stable_supported_access() {
    let operation = CoreOperation::Outbound(OutboundOperation::List);
    let scope = scope(Some("1.19.0"));
    let matrix = CoreCapabilityMatrix::new(scope.clone(), vec![supported_entry(operation)])
        .expect("unique operation");

    let resolution = matrix.resolve(key(scope, operation));

    assert_eq!(
        resolution.status,
        CoreCapabilityStatus::Available {
            access: CapabilityAccess::Supported,
            maturity: CapabilityMaturity::Stable,
        }
    );
    assert!(resolution.engine.is_some());
    assert!(resolution.host.is_some());
    assert!(resolution.adapter.is_some());
}

#[test]
fn owner_managed_decisions_keep_the_authoritative_owner_in_the_resolution() {
    let operations = [
        (
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            AdapterOperationOwner::CoreLifecycle,
        ),
        (
            CoreOperation::Version(VersionOperation::Inspect),
            AdapterOperationOwner::VersionManagement,
        ),
        (
            CoreOperation::Version(VersionOperation::Rollback),
            AdapterOperationOwner::VersionManagement,
        ),
        (
            CoreOperation::Version(VersionOperation::Install),
            AdapterOperationOwner::VersionManagement,
        ),
        (
            CoreOperation::Version(VersionOperation::Activate),
            AdapterOperationOwner::VersionManagement,
        ),
        (
            CoreOperation::Version(VersionOperation::Uninstall),
            AdapterOperationOwner::VersionManagement,
        ),
    ];

    for (operation, owner) in operations {
        let scope = scope(None);
        let matrix = CoreCapabilityMatrix::new(
            scope.clone(),
            vec![entry(
                operation,
                EngineCapabilityDecision::OwnerManaged { owner },
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )],
        )
        .expect("explicit owner matches its Core operation");
        let resolution = matrix.resolve(key(scope, operation));

        assert_eq!(
            resolution.engine,
            Some(EngineCapabilityDecision::OwnerManaged { owner })
        );
        assert_eq!(
            resolution.status,
            CoreCapabilityStatus::Available {
                access: CapabilityAccess::Supported,
                maturity: CapabilityMaturity::Stable,
            }
        );
    }
}

#[test]
fn owner_managed_authority_cannot_be_assigned_to_engine_operations() {
    let operation = CoreOperation::Profile(ProfileOperation::Apply);
    let result = CoreCapabilityMatrix::new(
        scope(None),
        vec![entry(
            operation,
            EngineCapabilityDecision::OwnerManaged {
                owner: AdapterOperationOwner::VersionManagement,
            },
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::Supported,
        )],
    );

    assert!(matches!(
        result,
        Err(
            super::CoreCapabilityMatrixError::OwnerManagedOperationNotAllowed(
                CoreOperation::Profile(ProfileOperation::Apply)
            )
        )
    ));
}

#[test]
fn owner_managed_authority_must_match_its_lifecycle_or_version_owner() {
    let operation = CoreOperation::Lifecycle(CoreLifecycleOperation::Stop);
    let result = CoreCapabilityMatrix::new(
        scope(None),
        vec![entry(
            operation,
            EngineCapabilityDecision::OwnerManaged {
                owner: AdapterOperationOwner::VersionManagement,
            },
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::Supported,
        )],
    );

    assert!(matches!(
        result,
        Err(
            super::CoreCapabilityMatrixError::OwnerManagedOperationMismatch {
                operation: CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
                expected: AdapterOperationOwner::CoreLifecycle,
                actual: AdapterOperationOwner::VersionManagement,
            }
        )
    ));
}

#[test]
fn published_capability_snapshot_contains_owner_resolved_status_for_every_operation() {
    let operation = CoreOperation::Outbound(OutboundOperation::Select);
    let scope = scope(Some("1.19.0"));
    let matrix = CoreCapabilityMatrix::new(scope, vec![supported_entry(operation)])
        .expect("unique operation");

    let snapshot = CoreCapabilitySnapshot::from_matrix(&matrix);
    assert_eq!(snapshot.resolutions.len(), CoreOperation::all().len());
    assert_eq!(
        snapshot
            .resolution(operation)
            .expect("registered operation")
            .status,
        CoreCapabilityStatus::Available {
            access: CapabilityAccess::Supported,
            maturity: CapabilityMaturity::Stable,
        }
    );
    assert_eq!(
        snapshot
            .resolution(CoreOperation::Outbound(OutboundOperation::Probe))
            .expect("unregistered operation is explicit")
            .status,
        CoreCapabilityStatus::Unregistered
    );
    let encoded = serde_json::to_string(&snapshot).expect("serialize snapshot");
    assert_eq!(
        serde_json::from_str::<CoreCapabilitySnapshot>(&encoded).expect("validated snapshot"),
        snapshot
    );

    let mut stale_resolution = snapshot.clone();
    let selected_resolution = stale_resolution
        .resolutions
        .iter_mut()
        .find(|resolution| resolution.key.operation == operation)
        .expect("selected operation has a resolution");
    selected_resolution.key.scope.adapter_revision = AdapterRevision::new(2);
    assert!(matches!(
        stale_resolution.validate(),
        Err(super::CoreCapabilitySnapshotError::ScopeMismatch)
    ));
    assert!(
        stale_resolution.resolution(operation).is_none(),
        "an operation resolution from another adapter revision must not be reused"
    );
    assert!(
        serde_json::from_value::<CoreCapabilitySnapshot>(
            serde_json::to_value(&stale_resolution).expect("serialize stale snapshot")
        )
        .is_err()
    );

    let mut forged_owner = snapshot.clone();
    let selected_resolution = forged_owner
        .resolutions
        .iter_mut()
        .find(|resolution| resolution.key.operation == operation)
        .expect("selected operation has a resolution");
    selected_resolution.engine = Some(EngineCapabilityDecision::OwnerManaged {
        owner: AdapterOperationOwner::CoreLifecycle,
    });
    assert!(matches!(
        forged_owner.validate(),
        Err(
            super::CoreCapabilitySnapshotError::OwnerManagedOperationNotAllowed(
                CoreOperation::Outbound(OutboundOperation::Select)
            )
        )
    ));
    assert!(
        serde_json::from_value::<CoreCapabilitySnapshot>(
            serde_json::to_value(&forged_owner).expect("serialize forged owner snapshot")
        )
        .is_err()
    );

    let mut missing = serde_json::to_value(&snapshot).expect("serialize value");
    missing["resolutions"]
        .as_array_mut()
        .expect("resolution array")
        .pop();
    assert!(serde_json::from_value::<CoreCapabilitySnapshot>(missing).is_err());
}

#[test]
fn matrix_resolution_is_fenced_to_exact_engine_version_target_and_adapter_revision() {
    let operation = CoreOperation::Version(VersionOperation::Install);
    let matrix_scope = CoreCapabilityScope::new_for_target(
        EngineId::new("mihomo").expect("valid engine ID"),
        Some(CoreVersion::new("v1.19.32").expect("valid version")),
        HostKind::Android,
        EngineBinaryTarget::Aarch64LinuxAndroid,
        AdapterRevision::new(3),
    );
    let matrix = CoreCapabilityMatrix::new(matrix_scope.clone(), vec![supported_entry(operation)])
        .expect("unique operation");
    let stale_scopes = [
        CoreCapabilityScope::new_for_target(
            EngineId::new("mihomo").expect("valid engine ID"),
            Some(CoreVersion::new("v1.19.31").expect("valid version")),
            HostKind::Android,
            EngineBinaryTarget::Aarch64LinuxAndroid,
            AdapterRevision::new(3),
        ),
        CoreCapabilityScope::new_for_target(
            EngineId::new("mihomo").expect("valid engine ID"),
            Some(CoreVersion::new("v1.19.32").expect("valid version")),
            HostKind::Android,
            EngineBinaryTarget::X86_64LinuxAndroid,
            AdapterRevision::new(3),
        ),
        CoreCapabilityScope::new_for_target(
            EngineId::new("mihomo").expect("valid engine ID"),
            Some(CoreVersion::new("v1.19.32").expect("valid version")),
            HostKind::Android,
            EngineBinaryTarget::Aarch64LinuxAndroid,
            AdapterRevision::new(4),
        ),
    ];

    for stale_scope in stale_scopes {
        assert_eq!(
            matrix.resolve(key(stale_scope, operation)).status,
            CoreCapabilityStatus::ScopeMismatch
        );
    }
    assert!(matches!(
        matrix.resolve(key(matrix_scope, operation)).status,
        CoreCapabilityStatus::Available { .. }
    ));
}

#[test]
fn read_only_and_experimental_decisions_are_both_preserved() {
    let operation = CoreOperation::Profile(ProfileOperation::Apply);
    let scope = scope(Some("1.19.0"));
    let matrix = CoreCapabilityMatrix::new(
        scope.clone(),
        vec![entry(
            operation,
            EngineCapabilityDecision::Experimental,
            HostCapabilityDecision::ReadOnly,
            AdapterCapabilityDecision::Supported,
        )],
    )
    .expect("unique operation");

    let resolution = matrix.resolve(key(scope, operation));

    assert_eq!(
        resolution.status,
        CoreCapabilityStatus::Available {
            access: CapabilityAccess::ReadOnly,
            maturity: CapabilityMaturity::Experimental,
        }
    );
}

#[test]
fn unsupported_decision_from_any_layer_fails_closed() {
    let operation = CoreOperation::Rules(RulesOperation::Apply);
    let scope = scope(Some("1.19.0"));
    let matrix = CoreCapabilityMatrix::new(
        scope.clone(),
        vec![entry(
            operation,
            EngineCapabilityDecision::Supported,
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::Unsupported {
                reason: "adapter does not implement writes".to_string(),
            },
        )],
    )
    .expect("unique operation");

    let resolution = matrix.resolve(key(scope, operation));

    assert_eq!(
        resolution.status,
        CoreCapabilityStatus::Unsupported {
            blockers: vec![CapabilityBlocker::Unsupported {
                layer: CapabilityLayer::Adapter,
                reason: "adapter does not implement writes".to_string(),
            }],
        }
    );
}

#[test]
fn host_version_and_adapter_absence_are_distinct_fail_closed_states() {
    let operation = CoreOperation::Tun(TunOperation::Start);
    let scope = scope(Some("1.19.0"));
    let matrix = CoreCapabilityMatrix::new(
        scope.clone(),
        vec![entry(
            operation,
            EngineCapabilityDecision::Supported,
            HostCapabilityDecision::HostUnavailable {
                reason: "VPN consent is not granted".to_string(),
            },
            AdapterCapabilityDecision::AdapterMissing {
                reason: "tun adapter is not installed".to_string(),
            },
        )],
    )
    .expect("unique operation");

    let resolution = matrix.resolve(key(scope, operation));

    assert!(matches!(
        resolution.status,
        CoreCapabilityStatus::HostUnavailable { blockers }
            if blockers.len() == 2
    ));
}

#[test]
fn missing_engine_version_blocks_a_nominally_supported_operation() {
    let operation = CoreOperation::Dns(DnsOperation::Query);
    let scope = scope(None);
    let matrix = CoreCapabilityMatrix::new(scope.clone(), vec![supported_entry(operation)])
        .expect("unique operation");

    let resolution = matrix.resolve(key(scope, operation));

    assert!(matches!(
        resolution.status,
        CoreCapabilityStatus::VersionUnavailable { blockers }
            if blockers.iter().any(|blocker| matches!(
                blocker,
                CapabilityBlocker::VersionUnavailable { .. }
            ))
    ));
}

#[test]
fn known_engine_version_without_a_capability_entry_is_version_unavailable() {
    let operation = CoreOperation::Subscription(SubscriptionOperation::Refresh);
    let scope = scope(Some("1.19.0"));
    let matrix = CoreCapabilityMatrix::new(
        scope.clone(),
        vec![entry(
            operation,
            EngineCapabilityDecision::VersionUnavailable {
                reason: "this release is absent from the compatibility matrix".to_string(),
            },
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::Supported,
        )],
    )
    .expect("unique operation");

    let resolution = matrix.resolve(key(scope, operation));

    assert!(matches!(
        resolution.status,
        CoreCapabilityStatus::VersionUnavailable { blockers }
            if blockers.len() == 1
    ));
}

#[test]
fn missing_adapter_is_distinct_from_an_unsupported_operation() {
    let operation = CoreOperation::Connection(ConnectionOperation::Close);
    let scope = scope(Some("1.19.0"));
    let matrix = CoreCapabilityMatrix::new(
        scope.clone(),
        vec![entry(
            operation,
            EngineCapabilityDecision::Supported,
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::AdapterMissing {
                reason: "connection control adapter is not registered".to_string(),
            },
        )],
    )
    .expect("unique operation");

    let resolution = matrix.resolve(key(scope, operation));

    assert!(matches!(
        resolution.status,
        CoreCapabilityStatus::AdapterMissing { blockers }
            if blockers.len() == 1
    ));
}

#[test]
fn host_target_mismatch_remains_the_effective_blocker_without_engine_evidence() {
    let operation = CoreOperation::Outbound(OutboundOperation::List);
    let scope = CoreCapabilityScope::new_for_target(
        EngineId::new("mihomo").expect("valid engine ID"),
        Some(CoreVersion::new("v1.19.32").expect("valid version")),
        HostKind::Desktop,
        EngineBinaryTarget::Aarch64LinuxAndroid,
        AdapterRevision::new(1),
    );
    let matrix = CoreCapabilityMatrix::new(
        scope.clone(),
        vec![entry(
            operation,
            EngineCapabilityDecision::VersionUnavailable {
                reason: "no operation evidence for Android target".to_owned(),
            },
            HostCapabilityDecision::HostUnavailable {
                reason: "host and binary target do not match".to_owned(),
            },
            AdapterCapabilityDecision::AdapterMissing {
                reason: "no adapter port".to_owned(),
            },
        )],
    )
    .expect("unique typed operation");

    assert!(matches!(
        matrix.resolve(key(scope, operation)).status,
        CoreCapabilityStatus::HostUnavailable { .. }
    ));
}

#[test]
fn missing_registration_and_mismatched_scope_fail_closed() {
    let matrix_scope = scope(Some("1.19.0"));
    let operation = CoreOperation::Logs(LogsOperation::Read);
    let matrix = CoreCapabilityMatrix::new(matrix_scope.clone(), vec![supported_entry(operation)])
        .expect("unique operation");

    assert_eq!(
        matrix
            .resolve(key(
                matrix_scope.clone(),
                CoreOperation::Connection(ConnectionOperation::List),
            ))
            .status,
        CoreCapabilityStatus::Unregistered
    );
    assert_eq!(
        matrix
            .resolve(key(scope_for("sing-box", Some("1.19.0")), operation))
            .status,
        CoreCapabilityStatus::ScopeMismatch
    );
}

#[test]
fn duplicate_operation_entries_are_rejected() {
    let operation = CoreOperation::Version(VersionOperation::Install);
    let error = CoreCapabilityMatrix::new(
        scope(Some("1.19.0")),
        vec![supported_entry(operation), supported_entry(operation)],
    )
    .expect_err("duplicate operation is ambiguous");

    assert_eq!(
        error,
        CoreCapabilityMatrixError::DuplicateOperation(operation)
    );
}

#[test]
fn deserialized_matrix_cannot_bypass_duplicate_operation_validation() {
    let value = serde_json::json!({
        "scope": {
            "engine_id": "mihomo",
            "core_version": "1.19.0",
            "host": "Android",
            "binary_target": "aarch64-linux-android",
            "adapter_revision": 3
        },
        "entries": [
            {
                "operation": {"category": "version", "operation": "install"},
                "engine": "Supported",
                "host": "Supported",
                "adapter": "Supported"
            },
            {
                "operation": {"category": "version", "operation": "install"},
                "engine": "Supported",
                "host": "Supported",
                "adapter": "Supported"
            }
        ]
    });

    let error = serde_json::from_value::<CoreCapabilityMatrix>(value)
        .expect_err("deserialization must preserve matrix uniqueness");

    assert!(
        error
            .to_string()
            .contains("duplicate core capability operation")
    );
}

#[test]
fn operation_key_serializes_engine_neutral_semantics() {
    let key = key(
        scope_for("sing-box", Some("1.11.0")),
        CoreOperation::Outbound(OutboundOperation::Select),
    );
    let json = serde_json::to_value(&key).expect("serialize key");

    assert_eq!(json["scope"]["engine_id"], "sing-box");
    assert_eq!(json["scope"]["core_version"], "1.11.0");
    assert_eq!(json["operation"]["category"], "outbound");
    assert_eq!(json["operation"]["operation"], "select");
}
