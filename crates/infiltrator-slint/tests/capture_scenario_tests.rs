//! GAP-4.1: the parity capture scenarios of the real Slint surface.
//!
//! The capture path may only publish a business the surface really opens.
//! Every case drives the production capture activation against the headless
//! surface harness and asserts that the carrier state is really open, that a
//! business the surface does not carry is refused, and that the capture
//! matrix lists exactly the businesses the surface carries.

use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::overview_layout::OverviewLayoutSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    ConnectionsPageSnapshot, LogsPageSnapshot, ProfileSnapshot, ProfilesPageSnapshot,
    RulesPageSnapshot, SurfaceSnapshot,
};
use infiltrator_slint::capture_scenario::{
    Activation, CarriedScenario, PageSearchField, ScenarioCarrier, activate,
};
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
    OutboundUiPreferencesSnapshot, OutboundUiPreferencesValues,
};
use lurker_core_contract::page::PageData;
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn profile(id: &str, name: &str, active: bool) -> ProfileSnapshot {
    serde_json::from_value(json!({
        "id": id,
        "name": name,
        "url": "https://example.com/sub",
        "updated_at": "2026-10-16 12:00:00",
        "is_active": active,
        "write_protection": "editable"
    }))
    .expect("profile JSON")
}

fn capture_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    let page: ProfilesPageSnapshot = serde_json::from_value(json!({
        "profiles": [
            profile("profile-1", "Alpha Fast (HK/JP/US)", true),
            profile("profile-2", "Beta Local Direct", false)
        ],
        "auto_update_interval_hours": 24,
        "updating": false
    }))
    .expect("profiles page JSON");
    snapshot.pages.profiles = PageData::ready(page);
    // The product pages the search businesses are carried by need rows the
    // shared filter really narrows.
    let rules: RulesPageSnapshot = serde_json::from_value(json!({
        "document": null,
        "total_rules": 2,
        "default_action": "DIRECT",
        "providers": [],
        "rules": [{
            "id": 1,
            "rule_type": "DomainSuffix",
            "payload": "github.com",
            "proxy": "PROXIES",
            "hit_count": 152,
            "raw": "DOMAIN-SUFFIX,github.com,PROXIES",
            "source_ip": false,
            "failure": null,
            "is_enabled": true,
            "no_resolve": false
        }, {
            "id": 2,
            "rule_type": "GeoIP",
            "payload": "CN",
            "proxy": "DIRECT",
            "hit_count": 1420,
            "raw": "GEOIP,CN,DIRECT",
            "source_ip": false,
            "failure": null,
            "is_enabled": true,
            "no_resolve": false
        }]
    }))
    .expect("rules page JSON");
    snapshot.pages.rules = PageData::ready(rules);
    let logs: LogsPageSnapshot = serde_json::from_value(json!({
        "stream": "Live",
        "total_entries": 2,
        "active_level": "info",
        "entries": [{
            "id": 1,
            "raw": null,
            "timestamp": "12:00:01",
            "level": "info",
            "tag": "CORE",
            "message": "Mihomo proxy core initialized"
        }, {
            "id": 2,
            "raw": null,
            "timestamp": "12:00:02",
            "level": "info",
            "tag": "DNS",
            "message": "Fake-IP mapping pool active"
        }]
    }))
    .expect("logs page JSON");
    snapshot.pages.logs = PageData::ready(logs);
    let connections: ConnectionsPageSnapshot = serde_json::from_value(json!({
        "total_connections": 2,
        "total_upload_bytes": 0,
        "total_download_bytes": 0,
        "connections": [{
            "id": "conn-1",
            "start": "2026-10-16T12:10:05Z",
            "destination_host": "api.github.com:443",
            "host": "api.github.com",
            "process": "git",
            "rule": "DomainSuffix",
            "rule_payload": "github.com",
            "chain": "PROXIES -> HK-01",
            "chains": ["PROXIES", "HK-01"],
            "network": "tcp",
            "source_ip": "127.0.0.1",
            "source_port": "54320",
            "destination_ip": "20.205.243.166",
            "destination_port": "443",
            "destination_geo_ip": ["US"],
            "destination_ip_asn": "15169 Google LLC",
            "rate_observation": "unobserved",
            "upload_bps": 0.0,
            "download_bps": 0.0,
            "upload_total": 0,
            "download_total": 0
        }, {
            "id": "conn-2",
            "start": "2026-10-16T12:11:10Z",
            "destination_host": "slint.dev:443",
            "host": "slint.dev",
            "process": "firefox",
            "rule": "DomainSuffix",
            "rule_payload": "slint.dev",
            "chain": "PROXIES -> JP-01",
            "chains": ["PROXIES", "JP-01"],
            "network": "tcp",
            "source_ip": "127.0.0.1",
            "source_port": "54322",
            "destination_ip": "104.21.5.88",
            "destination_port": "443",
            "destination_geo_ip": ["US"],
            "destination_ip_asn": "13335 Cloudflare",
            "rate_observation": "unobserved",
            "upload_bps": 0.0,
            "download_bps": 0.0,
            "upload_total": 0,
            "download_total": 0
        }]
    }))
    .expect("connections page JSON");
    snapshot.pages.connections = PageData::ready(connections);
    publish_outbound_inventory(&mut snapshot, "");
    snapshot
}

/// The outbound inventory the proxies page shows.
///
/// The page opens on the group the owner marked as active, so the fixture
/// publishes that group and its members.
fn publish_outbound_inventory(snapshot: &mut SurfaceSnapshot, query: &str) {
    let scope = CoreScope {
        identity: CoreIdentity {
            engine: EngineId::new("mihomo").expect("valid engine id"),
            instance: CoreInstanceId::new("capture-1").expect("valid instance id"),
            version: None,
        },
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let engine = scope.identity.engine.clone();
    let id = |value: &str| OutboundId::new(engine.clone(), value).expect("valid outbound id");
    let inventory = OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(1),
        OutboundInventoryState::Ready {
            entries: vec![
                OutboundSnapshot {
                    id: id("HK-01"),
                    display_name: Some("HK IPLC 01".to_owned()),
                },
                OutboundSnapshot {
                    id: id("JP-01"),
                    display_name: Some("Tokyo 01".to_owned()),
                },
            ],
            collections: vec![OutboundSelectionSet {
                id: id("PROXIES"),
                member_ids: vec![id("HK-01"), id("JP-01")],
                availability: SelectionAvailability::Selectable,
                selection: OutboundSelectionState::Ready {
                    selected: id("HK-01"),
                },
            }],
        },
    )
    .expect("valid capture inventory");
    snapshot.outbound_inventory = PageData::ready(inventory);
    // The preferences are the owner's fact, so the owner publishes them: the
    // search asks the owner for a new one instead of keeping a private copy.
    let preferences = OutboundUiPreferencesSnapshot::new(
        scope,
        SnapshotRevision::new(1),
        OutboundUiPreferencesValues {
            search_query: query.to_owned(),
            ..OutboundUiPreferencesValues::default()
        },
    )
    .expect("valid capture preferences");
    snapshot.outbound_ui_preferences = PageData::ready(preferences);
    // The owner marked the group as active, so the page opens on its nodes.
    snapshot.active_exit.group = Some("PROXIES".to_owned());
}

/// The snapshot whose outbound preferences carry the captured query.
///
/// The outbound search is the owner's own fact, so the owner publishes the
/// query it applied and the surface renders the narrowed node list.
fn searched_outbound_snapshot() -> SurfaceSnapshot {
    let mut snapshot = capture_snapshot();
    publish_outbound_inventory(&mut snapshot, PageSearchField::Proxies.value());
    snapshot
}

/// The owner snapshot the capture drives, with the card order the owner
/// publishes after the capture's own move.
fn moved_order_snapshot() -> SurfaceSnapshot {
    let mut snapshot = capture_snapshot();
    snapshot.overview_layout = OverviewLayoutSnapshot::new(vec![
        OverviewCardKind::Traffic,
        OverviewCardKind::RoutingIntentSegment,
        OverviewCardKind::Metrics,
        OverviewCardKind::MasterSwitches,
        OverviewCardKind::ActiveExit,
        OverviewCardKind::PublicIp,
        OverviewCardKind::Topology,
        OverviewCardKind::Quota,
    ]);
    snapshot
}

/// The rows of the crate's capture matrix.
fn matrix_rows() -> Vec<(String, String, String, String)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/capture/parity_scenarios.tsv");
    let text = fs::read_to_string(&path).expect("capture matrix");
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        rows.push((
            fields.next().unwrap_or_default().to_owned(),
            fields.next().unwrap_or_default().to_owned(),
            fields.next().unwrap_or_default().to_owned(),
            fields.next().unwrap_or_default().to_owned(),
        ));
    }
    rows
}

#[test]
fn every_carried_scenario_really_opens_its_carrier() {
    let snapshot = capture_snapshot();
    // The owner's typed answer of every business the capture drives.
    let answer = Failure::new(ErrorCode::Internal, "the owner refused the command", true);

    for scenario in CarriedScenario::all() {
        // Every business is captured in its own process, so every scenario
        // starts from the route-only render of its page.
        let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
        let window = surface.window();
        let activation = Activation::default();
        // The card order business reads the order the owner published, so the
        // surface observes that order before the business runs.
        if matches!(scenario.carrier(), ScenarioCarrier::OverviewOrderSheet) {
            surface.publish_snapshot(snapshot.clone());
        }
        // The first call drives the business's own gesture.
        activate(
            window,
            scenario.name(),
            &snapshot,
            surface.workbenches(),
            &activation,
        );
        if matches!(scenario.carrier(), ScenarioCarrier::WorkbenchPage(_)) {
            assert!(
                activation.performed(),
                "{} must drive its own gesture",
                scenario.name()
            );
        }
        // A page that projects its own filter does it on the next turn of
        // the event loop, so the pending events run before the surface is
        // read again.
        surface.run_pending_events();
        // The owner answers, so the business shows its own outcome.
        match scenario.carrier() {
            // The card order is the owner's fact, so the owner publishes the
            // new order and the list shows it.
            ScenarioCarrier::OverviewOrderSheet => {
                surface.publish_snapshot(moved_order_snapshot());
            }
            // The outbound search is the owner's own fact, so the owner
            // publishes the query it applied and the page shows the narrowed
            // node list.
            ScenarioCarrier::ProductPage("Proxy") => {
                surface.publish_snapshot(searched_outbound_snapshot());
            }
            _ => surface.publish_owner_failure(&answer),
        }
        assert!(
            activate(
                window,
                scenario.name(),
                &snapshot,
                surface.workbenches(),
                &activation,
            ),
            "the surface carries {} and must show its outcome",
            scenario.name()
        );
        match scenario.carrier() {
            ScenarioCarrier::WorkbenchPage(page) => {
                assert_eq!(
                    window.get_page().as_str(),
                    page,
                    "{} must open the {} page",
                    scenario.name(),
                    page
                );
                assert!(
                    !surface.workbench_items().is_empty(),
                    "{} must project the business panel of its page",
                    scenario.name()
                );
            }
            ScenarioCarrier::ProfileImportSheet => {
                assert!(
                    window.get_profile_add_open(),
                    "{} must open the add-profile sheet",
                    scenario.name()
                );
            }
            ScenarioCarrier::ProfileDeleteSheet => {
                assert_eq!(
                    window.get_pending_delete_id(),
                    "profile-2",
                    "{} must open the delete sheet of the deletable profile",
                    scenario.name()
                );
            }
            ScenarioCarrier::OverviewOrderSheet => {
                assert!(
                    window.get_overview_order_open(),
                    "{} must open the Overview card order sheet",
                    scenario.name()
                );
                let order = surface.overview_card_order();
                assert_eq!(
                    order.len(),
                    OverviewCardKind::DEFAULT_ORDER.len(),
                    "{} must show every shared Overview card",
                    scenario.name()
                );
                assert_eq!(
                    order.first().map(String::as_str),
                    Some(OverviewCardKind::Traffic.as_str()),
                    "{} must show the order the owner published",
                    scenario.name()
                );
            }
            ScenarioCarrier::ProductPage(page) => {
                assert_eq!(
                    window.get_page().as_str(),
                    page,
                    "{} must open the {} page",
                    scenario.name(),
                    page
                );
                assert!(
                    !surface.page_rows(page).is_empty(),
                    "{} must project the rows of its page",
                    scenario.name()
                );
            }
        }
    }
}

#[test]
fn the_capture_gesture_changes_the_rendered_page() {
    let snapshot = capture_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    let window = surface.window();
    let activation = Activation::default();
    let answer = Failure::new(ErrorCode::Internal, "the owner refused the command", true);

    // The DNS hosts editor is typed into and the row is added, so the
    // business's own list grows.
    let before = surface.workbench_items().len();
    activate(
        window,
        "dns-hosts-editor",
        &snapshot,
        surface.workbenches(),
        &activation,
    );
    let after = surface.workbench_items();
    assert!(
        after.len() > before,
        "the hosts editor adds the row the capture typed"
    );
    assert!(
        after
            .iter()
            .any(|item| item.detail.contains("blocked.example.com")),
        "the hosts editor shows the row the capture typed"
    );

    // The owner answers, so the business shows its own outcome.
    surface.publish_owner_failure(&answer);
    assert!(
        activate(
            window,
            "dns-hosts-editor",
            &snapshot,
            surface.workbenches(),
            &activation,
        ),
        "the business shows the owner's answer"
    );
}

#[test]
fn a_business_the_surface_does_not_carry_is_never_activated() {
    let snapshot = capture_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    let window = surface.window();
    let activation = Activation::default();

    // The businesses the Slint surface still has no carrier for.
    for absent in ["shell-keyboard-focus", "a-business-that-does-not-exist"] {
        assert!(
            !activate(
                window,
                absent,
                &snapshot,
                surface.workbenches(),
                &activation,
            ),
            "{absent} has no carrier on the Slint surface and must never be activated"
        );
    }

    // A refused scenario leaves the surface where it was.
    assert_eq!(window.get_page().as_str(), "Home");
    assert!(!window.get_profile_add_open());
}

#[test]
fn the_capture_matrix_lists_exactly_the_carried_businesses() {
    let rows = matrix_rows();
    let expected: BTreeSet<String> = CarriedScenario::all()
        .into_iter()
        .flat_map(|scenario| {
            [
                format!("{}-standard", scenario.name()),
                format!("{}-compact", scenario.name()),
            ]
        })
        .collect();
    let listed: BTreeSet<String> = rows.iter().map(|(name, ..)| name.clone()).collect();
    assert_eq!(
        listed, expected,
        "the capture matrix must list every carried business in both viewports"
    );

    for (name, page, skin, window_size) in &rows {
        assert!(!page.is_empty(), "{name} must name the page it captures");
        assert!(!skin.is_empty(), "{name} must name the skin it captures");
        assert!(
            window_size.contains('x'),
            "{name} must name the window size it captures"
        );
    }
}
