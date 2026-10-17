use super::*;
use crate::android_control::{
    VpnOwnerCoreReadback, VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceFact,
    VpnOwnerResourceKind, VpnOwnerResourceState, VpnOwnerSnapshot,
};
use crate::offline_startup::StartupNetworkPolicy;
use crate::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileFormatId, ProfileId, ProfileSchemaVersion,
    ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
};
use crate::resources::{CoreGcStatus, CoreResourceSnapshot, ResourceAvailability};
use crate::rule_trace_run::{
    RuleTraceOutboundFacts, RuleTraceOwnerBinding, RuleTraceOwnerObservation,
};
use crate::runtime_status::{CoreProcessObservation, ProxyEffectivenessObservation};
use crate::snapshot::CoreLifecycleSnapshot;
use crate::vpn::VpnGeneration;
use lurker_core_contract::error::FailureReason;
#[cfg(test)]
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::outbound::details::{OutboundDetailsSnapshot, OutboundDetailsState};
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
};
#[cfg(test)]
use lurker_core_contract::session::SessionToken;

#[test]
fn page_vocabulary_has_exactly_eleven_entries() {
    assert_eq!(PageId::ALL.len(), 11);
    assert_eq!(PageId::ALL[0].as_str(), "overview");
    assert_eq!(PageId::ALL[10].as_str(), "settings");
}

#[test]
fn unavailable_is_not_an_empty_page() {
    let failure = Failure::unsupported("host did not provide this capability");
    let page = PageData::<String>::unavailable(failure.clone());
    assert!(page.data.is_none());
    assert_eq!(page.status, PageStatus::Unavailable { failure });
}

#[test]
fn typed_outbound_details_and_preferences_are_required_surface_fields() {
    let snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("not composed"),
    );
    let serialized = serde_json::to_value(snapshot).expect("surface snapshot serializes");

    for field in [
        "outbound_inventory",
        "outbound_details",
        "outbound_ui_preferences",
    ] {
        let mut value = serialized.clone();
        value
            .as_object_mut()
            .expect("surface snapshot object")
            .remove(field);
        assert!(
            serde_json::from_value::<SurfaceSnapshot>(value).is_err(),
            "missing typed field {field} must be rejected"
        );
    }
}

#[test]
fn surface_snapshot_without_runtime_status_defaults_to_unknown_axes() {
    let snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::AndroidCompose,
        HostKind::Android,
        Failure::new(ErrorCode::NotReady, "owner not observed", true),
    );
    let mut value = serde_json::to_value(snapshot).expect("surface snapshot serializes");
    value
        .as_object_mut()
        .expect("surface snapshot object")
        .remove("runtime_status");

    let decoded: SurfaceSnapshot =
        serde_json::from_value(value).expect("legacy snapshot remains readable");
    assert_eq!(decoded.runtime_status, RuntimeStatusSnapshot::default());
    assert_eq!(
        decoded.runtime_status.core_process,
        CoreProcessObservation::Unknown
    );
    assert_eq!(
        decoded.runtime_status.proxy_effectiveness,
        ProxyEffectivenessObservation::Unknown
    );
}

#[test]
fn proxy_page_status_is_derived_from_typed_inventory_state() {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("not composed"),
    );
    let scope = CoreScope {
        identity: core_identity("mihomo", "owner-a"),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    snapshot.outbound_inventory = PageData::ready(
        OutboundInventorySnapshot::new(
            scope.clone(),
            SnapshotRevision::new(4),
            OutboundInventoryState::Empty,
        )
        .expect("empty typed inventory is valid"),
    );
    assert_eq!(snapshot.page_status(PageId::Proxies), PageStatus::Empty);

    snapshot.outbound_inventory = PageData::ready(
        OutboundInventorySnapshot::new(
            scope,
            SnapshotRevision::new(5),
            OutboundInventoryState::Unsupported,
        )
        .expect("unsupported typed inventory is valid"),
    );
    assert!(matches!(
        snapshot.page_status(PageId::Proxies),
        PageStatus::Unavailable { .. }
    ));
}

#[test]
fn boxed_observation_payloads_keep_their_existing_json_shape() {
    let scope = CoreScope {
        identity: core_identity("mihomo", "wire-shape-owner"),
        generation: CoreGeneration::new(3),
        session_token: None,
    };
    let resource_value = CoreResourceSnapshot::Observed {
        scope: Box::new(scope.clone()),
        memory: ResourceAvailability::Available(42),
        cpu_percent: ResourceAvailability::Available(2.5),
        memory_soft_limit_bytes: 128,
        gc: Box::new(CoreGcStatus::NotNeeded),
    };
    let resource_wire = serde_json::to_value(&resource_value).expect("serialize resources");
    assert_eq!(resource_wire["observation"], "observed");
    assert_eq!(resource_wire["scope"]["generation"], 3);
    assert_eq!(resource_wire["memory"]["value"], 42);
    assert_eq!(resource_wire["gc"]["state"], "not_needed");
    assert!(resource_wire.get("value").is_none());
    assert_eq!(
        serde_json::from_value::<CoreResourceSnapshot>(resource_wire.clone())
            .expect("round-trip resources"),
        resource_value
    );

    let revision = SnapshotRevision::new(4);
    let inventory =
        OutboundInventorySnapshot::new(scope.clone(), revision, OutboundInventoryState::Empty)
            .expect("empty inventory is valid");
    let details =
        OutboundDetailsSnapshot::new(scope.clone(), revision, OutboundDetailsState::Empty)
            .expect("empty details are valid");
    let outbound = OutboundObservationSnapshot::new(inventory, details)
        .expect("empty snapshots form one observation");
    let outbound_value = RuleTraceOutboundFacts::observed(outbound);
    let outbound_wire =
        serde_json::to_value(&outbound_value).expect("serialize outbound observation");
    assert_eq!(outbound_wire["state"], "observed");
    assert_eq!(outbound_wire["value"]["inventory"]["inventory"], "empty");
    assert!(outbound_wire["value"].get("value").is_none());
    assert_eq!(
        serde_json::from_value::<RuleTraceOutboundFacts>(outbound_wire)
            .expect("round-trip outbound facts"),
        outbound_value
    );

    let document = ProfileDocumentIdentity::new(
        ProfileId::new("wire-profile").expect("profile ID"),
        scope.identity.engine.clone(),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("wire-source").expect("source ID"),
        ),
        ProfileDocumentRevision::new("revision-1").expect("document revision"),
        ProfileFormatId::new("mihomo-yaml").expect("format ID"),
        ProfileSchemaVersion::new(1).expect("schema version"),
    );
    let workspace = ProfileWorkspaceIdentity::new(document, ProfileSidecarRevision::Absent);
    let owner = RuleTraceOwnerBinding::new(scope, workspace).expect("matching owner");
    let owner_value = RuleTraceOwnerObservation::observed(owner);
    let owner_wire = serde_json::to_value(&owner_value).expect("serialize owner observation");
    assert_eq!(owner_wire["Observed"]["scope"]["generation"], 3);
    assert_eq!(
        owner_wire["Observed"]["workspace"]["document"]["profile_id"],
        "wire-profile"
    );
    assert!(owner_wire["Observed"].get("value").is_none());
    assert_eq!(
        serde_json::from_value::<RuleTraceOwnerObservation>(owner_wire)
            .expect("round-trip owner observation"),
        owner_value
    );
}

#[test]
fn session_token_fences_old_snapshots_even_with_a_larger_revision() {
    let mut current = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("not ready"),
    );
    current.generation = 4;
    current.core.generation = 4;
    current.core.session_token = Some(SessionToken::new(40));
    current.revision = 10;
    current.core.revision = 10;

    let mut stale = current.clone();
    stale.core.session_token = Some(SessionToken::new(39));
    stale.revision = 11;
    stale.core.revision = 11;
    assert!(!stale.is_newer_than(&current));

    let mut next = current.clone();
    next.generation = 5;
    next.core.generation = 5;
    next.core.session_token = Some(SessionToken::new(50));
    next.revision = 1;
    next.core.revision = 1;
    assert!(next.is_newer_than(&current));

    let mut stopped = current.clone();
    stopped.core.session_token = None;
    stopped.revision = 11;
    stopped.core.revision = 11;
    assert!(stopped.is_newer_than(&current));
}

#[test]
fn core_identity_fences_snapshots_with_matching_generation_and_token() {
    let mut current = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("not ready"),
    );
    current.core.identity = Some(core_identity("mihomo", "owner-a"));
    current.generation = 4;
    current.core.generation = 4;
    current.core.session_token = Some(SessionToken::new(40));
    current.revision = 10;

    let mut foreign = current.clone();
    foreign.core.identity = Some(core_identity("sing-box", "owner-b"));
    foreign.revision = 100;
    assert!(!foreign.is_newer_than(&current));

    let mut unscoped = current.clone();
    unscoped.core.identity = None;
    unscoped.revision = 101;
    assert!(!unscoped.is_newer_than(&current));
}

#[test]
fn active_version_changes_do_not_make_the_same_owner_snapshot_foreign() {
    let mut current = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("not ready"),
    );
    current.core.identity = Some(core_identity_with_version(
        "mihomo",
        "owner-a",
        Some("1.19.12"),
    ));
    current.revision = 10;
    current.core.revision = 10;

    let mut starting = current.clone();
    starting.core.identity = Some(core_identity_with_version("mihomo", "owner-a", None));
    starting.generation = 1;
    starting.core.generation = 1;
    starting.core.session_token = Some(SessionToken::new(11));
    starting.revision = 1;
    starting.core.revision = 1;
    assert!(starting.is_newer_than(&current));

    let mut ready = starting.clone();
    ready.core.identity = Some(core_identity_with_version(
        "mihomo",
        "owner-a",
        Some("1.19.12"),
    ));
    ready.core.generation = 2;
    ready.generation = 2;
    ready.core.session_token = Some(SessionToken::new(12));
    ready.revision = 2;
    ready.core.revision = 2;
    assert!(ready.is_newer_than(&starting));

    let mut same_generation_version_change = ready.clone();
    same_generation_version_change.core.identity =
        Some(core_identity_with_version("mihomo", "owner-a", None));
    same_generation_version_change.revision += 1;
    same_generation_version_change.core.revision += 1;
    assert!(!same_generation_version_change.is_newer_than(&ready));

    let mut stale_generation = starting.clone();
    stale_generation.core.identity = Some(core_identity_with_version(
        "mihomo",
        "owner-a",
        Some("1.19.12"),
    ));
    stale_generation.generation = 0;
    stale_generation.core.generation = 0;
    stale_generation.revision = 100;
    stale_generation.core.revision = 100;
    assert!(!stale_generation.is_newer_than(&starting));
}

#[test]
fn android_surface_snapshot_keeps_owner_facts_and_fences_the_embedded_core_scope() {
    let identity = core_identity("mihomo", "owner-a");
    let core_snapshot = CoreLifecycleSnapshot {
        identity: Some(identity.clone()),
        lifecycle: CoreLifecycle::Running,
        generation: 7,
        session_token: Some(SessionToken::new(70)),
        revision: 19,
        failure: None,
    };
    let owner_snapshot = VpnOwnerSnapshot {
        owner_session_id: "vpn-owner-a".to_owned(),
        revision: 11,
        vpn_generation: VpnGeneration::new(5),
        vpn_state: VpnOwnerLifecycle::Running,
        readiness: VpnOwnerReadiness::Ready,
        resources: [
            VpnOwnerResourceKind::PreparedConfiguration,
            VpnOwnerResourceKind::Worker,
            VpnOwnerResourceKind::TunnelDescriptor,
            VpnOwnerResourceKind::RoutingLease,
        ]
        .into_iter()
        .map(|kind| VpnOwnerResourceFact {
            kind,
            state: match kind {
                VpnOwnerResourceKind::Worker | VpnOwnerResourceKind::TunnelDescriptor => {
                    VpnOwnerResourceState::Held { generation: 5 }
                }
                VpnOwnerResourceKind::PreparedConfiguration
                | VpnOwnerResourceKind::RoutingLease => VpnOwnerResourceState::Absent,
            },
        })
        .collect(),
        foreground: true,
        mtu: Some(1500),
        route_count: 1,
        dns_servers: vec!["1.1.1.1".to_owned()],
        ipv6: false,
        core: VpnOwnerCoreReadback::Available(core_snapshot.clone()),
    };
    let mut surface = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyAndroid,
        HostKind::Android,
        Failure::new(ErrorCode::NotReady, "waiting for owner", true),
    );
    surface.core.identity = Some(identity);
    surface.core.lifecycle = core_snapshot.lifecycle.clone();
    surface.core.generation = core_snapshot.generation;
    surface.core.session_token = core_snapshot.session_token;
    surface.core.revision = core_snapshot.revision;
    surface.core.failure = core_snapshot.failure.clone();
    surface.vpn = VpnSurfaceObservation::from_android_owner(owner_snapshot.clone());

    assert_eq!(surface.validate_vpn_owner_core_binding(), Ok(()));
    let encoded = serde_json::to_value(&surface).expect("surface snapshot serializes");
    assert_eq!(
        encoded["vpn"]["AndroidOwner"]["owner_session_id"],
        "vpn-owner-a",
    );
    assert_eq!(encoded["vpn"]["AndroidOwner"]["vpn_generation"], 5);
    assert_eq!(
        encoded["vpn"]["AndroidOwner"]["core"]["Available"]["generation"],
        7,
    );
    assert_eq!(
        serde_json::from_value::<VpnSurfaceObservation>(encoded["vpn"].clone())
            .expect("boxed Android owner observation round-trips"),
        surface.vpn
    );

    surface.core.generation = 8;
    let failure = surface
        .validate_vpn_owner_core_binding()
        .expect_err("surface and VPN owner must use the same Core generation");
    assert_eq!(failure.code, ErrorCode::InvalidState);

    surface.core.generation = core_snapshot.generation;
    surface.core.revision = core_snapshot.revision + 1;
    assert!(
        surface.validate_vpn_owner_core_binding().is_err(),
        "Core lifecycle revision must come from the same owner readback"
    );
    surface.core.revision = core_snapshot.revision;
    surface.core.failure = Some(Failure::new(ErrorCode::NotReady, "typed owner error", true));
    let failure = surface
        .validate_vpn_owner_core_binding()
        .expect_err("Core failure category and retryability must remain bound");
    assert_eq!(failure.code, ErrorCode::InvalidState);

    let typed_failure = Failure::new(
        ErrorCode::Configuration,
        "profile YAML could not be parsed",
        true,
    )
    .with_reason(FailureReason::YamlSyntax {
        line: 23,
        column: 6,
    });
    let mut failed_core = core_snapshot.clone();
    failed_core.lifecycle = CoreLifecycle::Failed;
    failed_core.failure = Some(typed_failure.clone());
    let mut failed_surface = surface.clone();
    failed_surface.core.lifecycle = CoreLifecycle::Failed;
    failed_surface.core.failure = Some(typed_failure.clone());
    let VpnSurfaceObservation::AndroidOwner(failed_owner) = &mut failed_surface.vpn else {
        panic!("Android owner snapshot should stay available")
    };
    failed_owner.core = VpnOwnerCoreReadback::Available(failed_core);
    assert_eq!(failed_surface.validate_vpn_owner_core_binding(), Ok(()));

    failed_surface.core.failure = Some(
        Failure::new(ErrorCode::Network, "profile YAML could not be parsed", true).with_reason(
            FailureReason::YamlSyntax {
                line: 23,
                column: 6,
            },
        ),
    );
    assert!(
        failed_surface.validate_vpn_owner_core_binding().is_err(),
        "failure category from another read must not bind to this owner snapshot"
    );

    failed_surface.core.failure = Some(
        Failure::new(
            ErrorCode::Configuration,
            "profile YAML could not be parsed",
            false,
        )
        .with_reason(FailureReason::YamlSyntax {
            line: 23,
            column: 6,
        }),
    );
    assert!(
        failed_surface.validate_vpn_owner_core_binding().is_err(),
        "failure retryability from another read must not bind to this owner snapshot"
    );
}

#[test]
fn core_capability_snapshot_is_fenced_to_surface_engine_version_and_binary_host() {
    use crate::core_capabilities::{
        AdapterRevision, CoreCapabilityMatrix, CoreCapabilityScope, CoreCapabilitySnapshot,
        EngineBinaryTarget,
    };
    use crate::surface::HostKind;

    let identity = core_identity_with_version("mihomo", "android-owner", Some("v1.20.0"));
    let mut surface = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyAndroid,
        HostKind::Android,
        Failure::unsupported("bootstrap"),
    );
    surface.core.identity = Some(identity.clone());
    surface.capabilities.host = HostKind::Android;
    let capability_scope = CoreCapabilityScope::new_for_target(
        identity.engine.clone(),
        identity.version.clone(),
        HostKind::Android,
        EngineBinaryTarget::Aarch64LinuxAndroid,
        AdapterRevision::new(7),
    );
    let capability_matrix = CoreCapabilityMatrix::new(capability_scope, Vec::new())
        .expect("empty test matrix has no duplicate operation");
    surface.core_capability_snapshot =
        PageData::ready(CoreCapabilitySnapshot::from_matrix(&capability_matrix));

    assert_eq!(surface.validate_core_capability_scope_binding(), Ok(()));

    let mut stale_identity = surface.clone();
    stale_identity.core.identity = Some(core_identity_with_version(
        "sing-box",
        "android-owner",
        Some("v1.20.0"),
    ));
    assert!(
        stale_identity
            .validate_core_capability_scope_binding()
            .is_err()
    );

    let mut stale_version = surface.clone();
    stale_version.core.identity = Some(core_identity_with_version(
        "mihomo",
        "android-owner",
        Some("v1.19.0"),
    ));
    assert!(
        stale_version
            .validate_core_capability_scope_binding()
            .is_err()
    );

    let mut target_host_mismatch = surface.clone();
    target_host_mismatch.capabilities.host = HostKind::Desktop;
    assert!(
        target_host_mismatch
            .validate_core_capability_scope_binding()
            .is_err()
    );

    let mut stale_resolution_scope = surface;
    stale_resolution_scope
        .core_capability_snapshot
        .data
        .as_mut()
        .expect("ready capability snapshot")
        .resolutions[0]
        .key
        .scope
        .adapter_revision = AdapterRevision::new(6);
    assert!(
        stale_resolution_scope
            .validate_core_capability_scope_binding()
            .is_err(),
        "an old owner resolution cannot be carried under a current snapshot scope"
    );
}

#[test]
fn first_scoped_snapshot_can_replace_an_unavailable_bootstrap_snapshot() {
    let current = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("not composed"),
    );
    let mut scoped = current.clone();
    scoped.core.identity = Some(core_identity("mihomo", "owner-a"));
    scoped.revision = 1;
    assert!(scoped.is_newer_than(&current));
}

#[test]
fn unavailable_surface_defaults_to_offline_first_without_claiming_ready() {
    let snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("not composed"),
    );
    assert_eq!(
        snapshot.offline_startup.policy,
        StartupNetworkPolicy::OfflineFirst
    );
    assert!(!snapshot.offline_startup.is_offline_startable());
}

fn core_identity(engine: &str, instance: &str) -> CoreIdentity {
    CoreIdentity::new(
        EngineId::new(engine).expect("valid engine ID"),
        CoreInstanceId::new(instance).expect("valid instance ID"),
        None,
    )
}

fn core_identity_with_version(engine: &str, instance: &str, version: Option<&str>) -> CoreIdentity {
    CoreIdentity::new(
        EngineId::new(engine).expect("valid engine ID"),
        CoreInstanceId::new(instance).expect("valid instance ID"),
        version.map(|value| CoreVersion::new(value).expect("valid engine version")),
    )
}
