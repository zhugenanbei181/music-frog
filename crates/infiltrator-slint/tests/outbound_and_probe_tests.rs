//! SLINT-001.8: Outbound selection, probing and presentation preferences of the
//! real Slint surface.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness. The selection and probe request builders are owned by
//! `infiltrator_slint::surface::action`; these tests assert the exact typed
//! owner command the surface emits and the typed failure it raises.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::runtime_settings::{RuntimeProbeSettings, RuntimeProbeSettingsState};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_slint::SlintAction;
use infiltrator_slint::surface::harness;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::outbound::details::OutboundSnapshot;
use lurker_core_contract::outbound::identity::{OutboundId, SnapshotRevision};
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
    OutboundSelectionState, SelectionAvailability,
};
use lurker_core_contract::outbound::preferences::{
    OutboundSortOrder, OutboundUiPreferenceIntent, OutboundUiPreferencesSnapshot,
    OutboundUiPreferencesValues,
};
use lurker_core_contract::outbound::probe::OutboundProbeBatchScope;
use lurker_core_contract::page::PageData;

const PROBE_URL: &str = "http://cp.cloudflare.com/generate_204";
const PROBE_TIMEOUT_MS: u32 = 5_000;

fn scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("slint-outbound-test").unwrap(),
            None,
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

fn outbound_id(value: &str) -> OutboundId {
    OutboundId::new(EngineId::mihomo(), value).unwrap()
}

fn outbound_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    let inventory = OutboundInventorySnapshot::new(
        scope(),
        SnapshotRevision::new(10),
        OutboundInventoryState::Ready {
            entries: vec![
                OutboundSnapshot {
                    id: outbound_id("SGP-01"),
                    display_name: Some("SGP-01".into()),
                },
                OutboundSnapshot {
                    id: outbound_id("HKG-01"),
                    display_name: Some("HKG-01".into()),
                },
            ],
            collections: vec![OutboundSelectionSet {
                id: outbound_id("PROXIES"),
                member_ids: vec![outbound_id("SGP-01"), outbound_id("HKG-01")],
                availability: SelectionAvailability::Selectable,
                selection: OutboundSelectionState::Ready {
                    selected: outbound_id("HKG-01"),
                },
            }],
        },
    )
    .expect("valid engine-scoped inventory");
    snapshot.outbound_inventory = PageData::ready(inventory);

    let preferences = OutboundUiPreferencesSnapshot::new(
        scope(),
        SnapshotRevision::new(4),
        OutboundUiPreferencesValues::default(),
    )
    .expect("valid engine-scoped preferences");
    snapshot.outbound_ui_preferences = PageData::ready(preferences);

    snapshot.probe_settings = RuntimeProbeSettingsState::Available {
        settings: RuntimeProbeSettings::new(PROBE_URL, PROBE_TIMEOUT_MS)
            .expect("valid probe settings"),
    };
    snapshot
}

/// Asserts that the surface dispatched exactly one owner command.
fn single_command(outcome: harness::SurfaceOutcome) -> CommandIntent {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => {
            assert_eq!(
                actions.len(),
                1,
                "expected one owner action, got {actions:?}"
            );
            match &actions[0] {
                SlintAction::Command(intent) => intent.clone(),
                other => panic!("expected an owner command, got {other:?}"),
            }
        }
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected an owner command, got the failure {failure:?}")
        }
    }
}

/// Asserts that the surface refused the gesture with the given typed failure.
fn rejection(outcome: harness::SurfaceOutcome) -> Failure {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => {
            panic!("expected a refusal, got the owner actions {actions:?}")
        }
        harness::SurfaceOutcome::Rejected { failure, actions } => {
            assert!(
                actions.is_empty(),
                "a refused gesture must not dispatch owner actions, got {actions:?}"
            );
            failure
        }
    }
}

#[test]
fn select_proxy_dispatches_a_selection_pinned_to_the_inventory_revision() {
    let surface = harness::SurfaceHarness::new(Some(outbound_snapshot())).unwrap();

    let command = single_command(surface.select_proxy("PROXIES", "SGP-01"));
    let CommandIntent::SelectOutbound { request } = command else {
        panic!("expected a selection command, got {command:?}");
    };
    assert_eq!(request.collection_id().value(), "PROXIES");
    assert_eq!(request.outbound_id().value(), "SGP-01");
    assert_eq!(request.expected_revision(), SnapshotRevision::new(10));
}

#[test]
fn select_proxy_refuses_an_unknown_group() {
    let surface = harness::SurfaceHarness::new(Some(outbound_snapshot())).unwrap();

    let failure = rejection(surface.select_proxy("MISSING", "SGP-01"));
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(failure.message, "Unknown proxy group");
    assert_eq!(surface.notice(), "Unknown proxy group");
}

#[test]
fn select_proxy_refuses_an_unknown_node() {
    let surface = harness::SurfaceHarness::new(Some(outbound_snapshot())).unwrap();

    let failure = rejection(surface.select_proxy("PROXIES", "NOPE"));
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(failure.message, "Unknown proxy node");
}

#[test]
fn select_proxy_without_an_inventory_is_refused_as_not_ready() {
    let mut snapshot = outbound_snapshot();
    snapshot.outbound_inventory = PageData::unavailable(Failure::new(
        ErrorCode::NotReady,
        "controller unreachable",
        true,
    ));
    let surface = harness::SurfaceHarness::new(Some(snapshot)).unwrap();

    let failure = rejection(surface.select_proxy("PROXIES", "SGP-01"));
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(failure.message, "Outbound inventory is unavailable");
}

#[test]
fn test_proxy_dispatches_a_single_node_probe_with_the_shared_probe_settings() {
    let surface = harness::SurfaceHarness::new(Some(outbound_snapshot())).unwrap();

    let command = single_command(surface.test_proxy("PROXIES", "HKG-01"));
    let CommandIntent::ProbeOutbound { request } = command else {
        panic!("expected a probe command, got {command:?}");
    };
    assert_eq!(request.outbound_id().value(), "HKG-01");
    assert_eq!(request.test_url(), PROBE_URL);
    assert_eq!(request.timeout_ms(), u64::from(PROBE_TIMEOUT_MS));
    assert_eq!(surface.proxy_testing_group(), "PROXIES");
    assert_eq!(surface.proxy_testing_node(), "HKG-01");
}

#[test]
fn test_proxy_on_a_group_dispatches_the_batch_probe_with_concurrency_eight() {
    let surface = harness::SurfaceHarness::new(Some(outbound_snapshot())).unwrap();

    let command = single_command(surface.test_proxy("PROXIES", ""));
    let CommandIntent::ProbeOutboundBatch { request } = command else {
        panic!("expected a batch probe command, got {command:?}");
    };
    assert_eq!(
        request.target_scope(),
        &OutboundProbeBatchScope::Collection(outbound_id("PROXIES"))
    );
    assert_eq!(request.concurrency().limit(), 8);
    assert_eq!(request.timeout_ms(), u64::from(PROBE_TIMEOUT_MS));
}

#[test]
fn test_proxy_refuses_an_unknown_group() {
    let surface = harness::SurfaceHarness::new(Some(outbound_snapshot())).unwrap();

    let failure = rejection(surface.test_proxy("MISSING", ""));
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(failure.message, "Unknown proxy group");
}

#[test]
fn test_proxy_refuses_when_the_probe_settings_are_unavailable() {
    let mut snapshot = outbound_snapshot();
    snapshot.probe_settings = RuntimeProbeSettingsState::Unavailable {
        failure: Failure::unsupported("probe settings have not been observed"),
    };
    let surface = harness::SurfaceHarness::new(Some(snapshot)).unwrap();

    let failure = rejection(surface.test_proxy("PROXIES", "HKG-01"));
    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert_eq!(failure.message, "probe settings have not been observed");
}

#[test]
fn search_proxy_nodes_dispatches_the_shared_search_preference() {
    let surface = harness::SurfaceHarness::new(Some(outbound_snapshot())).unwrap();

    let command = single_command(surface.search_proxy_nodes("HKG"));
    let CommandIntent::ApplyOutboundUiPreference { request } = command else {
        panic!("expected a preference command, got {command:?}");
    };
    assert_eq!(
        request.intent(),
        &OutboundUiPreferenceIntent::SetSearchQuery("HKG".into())
    );
}

#[test]
fn set_proxy_sort_dispatches_the_shared_sort_preference() {
    let surface = harness::SurfaceHarness::new(Some(outbound_snapshot())).unwrap();

    let command = single_command(surface.set_proxy_sort("name_desc"));
    let CommandIntent::ApplyOutboundUiPreference { request } = command else {
        panic!("expected a preference command, got {command:?}");
    };
    assert_eq!(
        request.intent(),
        &OutboundUiPreferenceIntent::SetSortOrder(OutboundSortOrder::NameDescending)
    );
}

#[test]
fn the_projection_lists_the_nodes_of_the_selected_group() {
    let snapshot = outbound_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    surface.publish_snapshot(snapshot);

    let rows = surface.proxy_node_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "SGP-01");
    assert_eq!(rows[1].id, "HKG-01");
}

#[test]
fn unknown_proxy_sort_is_refused_with_the_typed_failure() {
    let surface = harness::SurfaceHarness::new(Some(outbound_snapshot())).unwrap();

    let failure = rejection(surface.set_proxy_sort("random"));
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(failure.message, "Unknown proxy sort order");
    assert_eq!(surface.notice(), "Unknown proxy sort order");
}
