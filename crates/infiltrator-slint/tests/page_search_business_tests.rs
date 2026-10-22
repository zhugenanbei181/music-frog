//! The search businesses of the product pages of the real Slint surface.
//!
//! Every search business is carried by the search box of its own page, and
//! the filter is the shared page model: the surface never keeps a private
//! string filter of its own. Every case below drives the production Slint
//! callbacks through the headless surface harness, so a surface that lost
//! the shared filter fails here.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    ConnectionsPageSnapshot, LogsPageSnapshot, RulesPageSnapshot, SurfaceSnapshot,
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
    OutboundUiPreferenceIntent, OutboundUiPreferencesSnapshot, OutboundUiPreferencesValues,
};
use lurker_core_contract::page::PageData;
use serde_json::json;

/// The owner snapshot the product pages observe.
fn product_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
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

    snapshot.pages.logs = PageData::ready(logs_page(&[
        ("12:00:01", "CORE", "Mihomo proxy core initialized"),
        ("12:00:02", "DNS", "Fake-IP mapping pool active"),
    ]));

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
    publish_outbound_inventory(&mut snapshot);
    snapshot
}

/// The log page the fixture publishes.
///
/// The records are the owner's facts, so a test that publishes a new source
/// publishes its own records.
fn logs_page(entries: &[(&str, &str, &str)]) -> LogsPageSnapshot {
    let records: Vec<serde_json::Value> = entries
        .iter()
        .enumerate()
        .map(|(index, (timestamp, tag, message))| {
            json!({
                "id": index + 1,
                "raw": null,
                "timestamp": timestamp,
                "level": "info",
                "tag": tag,
                "message": message
            })
        })
        .collect();
    serde_json::from_value(json!({
        "stream": "Live",
        "total_entries": entries.len(),
        "active_level": "info",
        "entries": records
    }))
    .expect("logs page JSON")
}

/// The engine scope the published outbound inventory belongs to.
fn core_scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity {
            engine: EngineId::new("mihomo").expect("valid engine id"),
            instance: CoreInstanceId::new("fixture-1").expect("valid instance id"),
            version: None,
        },
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

/// The outbound inventory the proxies page shows.
///
/// The page opens on the group the owner marked as active, so the fixture
/// publishes that group and its members.
fn publish_outbound_inventory(snapshot: &mut SurfaceSnapshot) {
    let scope = core_scope();
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
                OutboundSnapshot {
                    id: id("US-01"),
                    display_name: Some("Los Angeles 01".to_owned()),
                },
            ],
            collections: vec![OutboundSelectionSet {
                id: id("PROXIES"),
                member_ids: vec![id("HK-01"), id("JP-01"), id("US-01")],
                availability: SelectionAvailability::Selectable,
                selection: OutboundSelectionState::Ready {
                    selected: id("HK-01"),
                },
            }],
        },
    )
    .expect("valid fixture inventory");
    snapshot.outbound_inventory = PageData::ready(inventory);
    // The preferences are the owner's fact, so the owner publishes them: the
    // search asks the owner for a new one instead of keeping a private copy.
    let preferences = OutboundUiPreferencesSnapshot::new(
        scope,
        SnapshotRevision::new(1),
        OutboundUiPreferencesValues::default(),
    )
    .expect("valid fixture preferences");
    snapshot.outbound_ui_preferences = PageData::ready(preferences);
    // The owner marked the group as active, so the page opens on its nodes.
    snapshot.active_exit.group = Some("PROXIES".to_owned());
}

/// The snapshot whose outbound preferences carry the given search query.
///
/// The preference is the owner's fact, so the owner publishes the query it
/// applied and the surface renders the narrowed node list.
fn proxies_snapshot(query: &str) -> SurfaceSnapshot {
    let mut snapshot = product_snapshot();
    let preferences = OutboundUiPreferencesSnapshot::new(
        core_scope(),
        SnapshotRevision::new(2),
        OutboundUiPreferencesValues {
            search_query: query.to_owned(),
            ..OutboundUiPreferencesValues::default()
        },
    )
    .expect("valid fixture preferences");
    snapshot.outbound_ui_preferences = PageData::ready(preferences);
    snapshot
}

/// The headless surface with one product page open.
fn product_surface(page: &str) -> harness::SurfaceHarness {
    let snapshot = product_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).expect("headless surface");
    surface.publish_snapshot(snapshot);
    surface.open_page(page);
    surface
}

#[test]
fn the_rule_search_narrows_the_rule_list() {
    let surface = product_surface("Rules");
    assert_eq!(surface.page_rows("Rules").len(), 2);

    surface.search_page("Rules", "github");

    let rows = surface.page_rows("Rules");
    assert_eq!(rows.len(), 1, "the shared filter keeps the matching rule");
    assert!(
        rows[0].title.as_str().contains("github"),
        "the surviving rule is the one the search matched"
    );
}

#[test]
fn the_log_search_narrows_the_log_list() {
    let surface = product_surface("Logs");
    assert_eq!(surface.page_rows("Logs").len(), 2);

    surface.search_page("Logs", "Fake-IP");

    let rows = surface.page_rows("Logs");
    assert_eq!(rows.len(), 1, "the shared filter keeps the matching entry");
    assert!(
        rows[0].title.as_str().contains("Fake-IP"),
        "the surviving entry is the one the search matched"
    );
}

#[test]
fn the_connection_search_narrows_the_connection_list() {
    let surface = product_surface("Connections");
    assert_eq!(surface.page_rows("Connections").len(), 2);

    surface.search_page("Connections", "github");

    let rows = surface.page_rows("Connections");
    assert_eq!(rows.len(), 1, "the shared filter keeps the matching row");
    assert!(
        rows[0].title.as_str().contains("github"),
        "the surviving connection is the one the search matched"
    );
}

#[test]
fn the_connection_search_reports_no_match() {
    let surface = product_surface("Connections");

    surface.search_page("Connections", "no-connection-matches-this");

    assert!(
        surface.page_rows("Connections").is_empty(),
        "a query that matches nothing must leave the list empty"
    );
}

#[test]
fn the_rule_search_reports_no_match() {
    let surface = product_surface("Rules");

    surface.search_page("Rules", "no-rule-matches-this");

    assert!(
        surface.page_rows("Rules").is_empty(),
        "a query that matches nothing must leave the list empty"
    );
}

#[test]
fn the_log_search_reports_no_match() {
    let surface = product_surface("Logs");

    surface.search_page("Logs", "no-log-matches-this");

    assert!(
        surface.page_rows("Logs").is_empty(),
        "a query that matches nothing must leave the list empty"
    );
    assert!(
        surface.log_search_error().is_empty(),
        "a query that matches nothing is not a broken query"
    );
}

#[test]
fn the_log_search_reports_an_invalid_pattern() {
    let surface = product_surface("Logs");

    surface.search_page("Logs", "[unclosed");

    let error = surface.log_search_error();
    assert!(
        error.contains("regex parse error"),
        "the page must show the reason the shared search reports: {error}"
    );
    assert!(
        surface.page_rows("Logs").is_empty(),
        "a query the shared search cannot apply keeps no record"
    );
}

#[test]
fn the_log_search_clear_restores_the_entries() {
    let surface = product_surface("Logs");
    surface.search_page("Logs", "Fake-IP");
    assert_eq!(surface.page_rows("Logs").len(), 1);

    surface.search_page("Logs", "");

    assert_eq!(
        surface.page_rows("Logs").len(),
        2,
        "the cleared query restores every published record"
    );
}

#[test]
fn the_log_search_survives_a_fresh_read() {
    let surface = product_surface("Logs");
    surface.search_page("Logs", "Fake-IP");

    surface.publish_snapshot(product_snapshot());

    assert_eq!(
        surface.page_rows("Logs").len(),
        1,
        "the fresh read keeps the query the owner published"
    );
}

#[test]
fn the_log_search_survives_a_language_change() {
    let surface = product_surface("Logs");
    surface.search_page("Logs", "Fake-IP");

    let mut snapshot = product_snapshot();
    snapshot.language_settings.preference = Some(LanguagePreference::English);
    surface.publish_snapshot(snapshot);

    assert_eq!(
        surface.page_rows("Logs").len(),
        1,
        "the language change keeps the query the user typed"
    );
}

#[test]
fn the_log_search_retires_the_records_of_a_new_source() {
    let surface = product_surface("Logs");
    surface.search_page("Logs", "Fake-IP");
    assert_eq!(surface.page_rows("Logs").len(), 1);

    let mut snapshot = product_snapshot();
    snapshot.generation = 2;
    snapshot.pages.logs =
        PageData::ready(logs_page(&[("12:00:09", "DNS", "Fake-IP pool resized")]));
    surface.publish_snapshot(snapshot);

    let rows = surface.page_rows("Logs");
    assert_eq!(rows.len(), 1, "the new source publishes its own records");
    assert!(
        rows[0].title.as_str().contains("resized"),
        "the record of the retired source must not survive"
    );
}

/// The outbound command one search gesture dispatched.
fn search_intent(outcome: harness::SurfaceOutcome) -> CommandIntent {
    let actions = match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => actions,
        other => panic!("the search must dispatch one owner command, got {other:?}"),
    };
    actions
        .iter()
        .find_map(|action| match action {
            infiltrator_slint::SlintAction::Command(intent) => Some(intent.clone()),
            _ => None,
        })
        .expect("the search dispatches one owner command")
}

#[test]
fn the_proxies_search_submits_the_typed_owner_preference() {
    let surface = product_surface("Proxy");
    assert_eq!(surface.proxy_node_rows().len(), 3);

    let intent = search_intent(surface.search_proxy_nodes("Tokyo"));

    match intent {
        CommandIntent::ApplyOutboundUiPreference { request } => assert!(
            matches!(
                request.intent(),
                OutboundUiPreferenceIntent::SetSearchQuery(query) if query.as_str() == "Tokyo"
            ),
            "the search must ask the owner for the typed query"
        ),
        other => panic!("the search must dispatch the typed preference, got {other:?}"),
    }

    surface.publish_snapshot(proxies_snapshot("Tokyo"));

    let rows = surface.proxy_node_rows();
    assert_eq!(rows.len(), 1, "the owner's readback narrows the node list");
    assert!(
        rows[0].title.as_str().contains("Tokyo"),
        "the surviving node is the one the search matched"
    );
}

#[test]
fn the_proxies_search_reports_no_match() {
    let surface = product_surface("Proxy");

    surface.publish_snapshot(proxies_snapshot("no-node-matches-this"));

    assert!(
        surface.proxy_node_rows().is_empty(),
        "a query that matches nothing must leave the node list empty"
    );
}

#[test]
fn the_proxies_search_clear_restores_the_nodes() {
    let surface = product_surface("Proxy");
    surface.publish_snapshot(proxies_snapshot("Tokyo"));
    assert_eq!(surface.proxy_node_rows().len(), 1);

    surface.publish_snapshot(proxies_snapshot(""));

    assert_eq!(
        surface.proxy_node_rows().len(),
        3,
        "the cleared query restores every published node"
    );
}

#[test]
fn the_proxies_search_keeps_the_facts_after_a_refusal() {
    let surface = product_surface("Proxy");
    surface.publish_snapshot(proxies_snapshot("Tokyo"));
    let before = surface.proxy_node_rows().len();

    surface.publish_owner_failure(&Failure::unsupported(
        "the outbound preference could not be applied",
    ));

    assert_eq!(
        surface.proxy_node_rows().len(),
        before,
        "a refused write keeps the facts the owner published"
    );
    assert!(
        surface
            .notice()
            .contains("the outbound preference could not be applied"),
        "the typed refusal reaches the owner-visible notice"
    );

    let intent = search_intent(surface.search_proxy_nodes("Tokyo"));
    assert!(
        matches!(intent, CommandIntent::ApplyOutboundUiPreference { .. }),
        "the retry asks the owner for the same typed preference"
    );
}

#[test]
fn the_proxies_search_survives_a_fresh_read() {
    let surface = product_surface("Proxy");
    surface.publish_snapshot(proxies_snapshot("Tokyo"));

    surface.publish_snapshot(proxies_snapshot("Tokyo"));

    assert_eq!(
        surface.proxy_node_rows().len(),
        1,
        "the fresh read keeps the narrowed node list"
    );
    assert_eq!(
        surface.proxy_search(),
        "Tokyo",
        "the fresh read keeps the query the owner published"
    );
}
