//! SLINT-001.8: The Slint surface reads the shared owners of the rules,
//! connections, routing and LAN sharing pages.
//!
//! Every case drives the production Slint projection through the headless
//! surface harness. The rule-list filter, the connection order, the routing
//! mode and the LAN sharing readback belong to the shared page models, so
//! the cases below prove the shared behaviour instead of a surface-local
//! copy of it: the rule case cannot pass with the surface's own substring
//! filter, the closed history case cannot pass with a surface-local order,
//! and the LAN card case cannot pass with a readback the owner never
//! published.

use infiltrator_contract::lan::LanSharingSnapshot;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::rule_snapshot::RuleSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    ConnectionSnapshot, ConnectionsPageSnapshot, RulesPageSnapshot, SurfaceSnapshot,
};
use infiltrator_contract::traffic_history::{ClosedConnectionSnapshot, ClosedConnectionsSnapshot};
use infiltrator_domain::connection_view::{ConnectionSortKey, ConnectionView, sort_connections};
use infiltrator_slint::surface::harness;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::PageData;

/// The owner snapshot the surface reads.
///
/// The language is pinned so the copy the surface renders is the one the
/// shared locale table holds, never the one the host locale would pick.
fn desktop_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    snapshot.language_settings.preference = Some(LanguagePreference::English);
    snapshot
}

/// The published rule the owner reports without a raw expression.
///
/// The shared rule view recombines the parts of such a rule, so the rule
/// stays searchable by its payload. The surface's own filter could only look
/// at the raw expression, the proxy and the rule type, so it lost this rule.
fn recombined_rule() -> RuleSnapshot {
    RuleSnapshot {
        id: 2,
        edit_id: None,
        rule_type: "DOMAIN".into(),
        payload: "example.com".into(),
        proxy: "PROXIES".into(),
        hit_count: None,
        raw: String::new(),
        source_ip: false,
        failure: None,
        is_enabled: true,
        no_resolve: false,
        last_hit_secs: None,
        is_shadowed: false,
        shadow_reason: None,
    }
}

/// The rules the owner publishes.
fn rules_page() -> RulesPageSnapshot {
    RulesPageSnapshot {
        document: None,
        total_rules: 2,
        default_action: "MATCH".into(),
        providers: Vec::new(),
        rules: vec![
            RuleSnapshot {
                id: 1,
                edit_id: None,
                rule_type: "DOMAIN-SUFFIX".into(),
                payload: "github.com".into(),
                proxy: "PROXIES".into(),
                hit_count: Some(12),
                raw: "DOMAIN-SUFFIX,github.com,PROXIES".into(),
                source_ip: false,
                failure: None,
                is_enabled: true,
                no_resolve: false,
                last_hit_secs: None,
                is_shadowed: false,
                shadow_reason: None,
            },
            recombined_rule(),
        ],
        tracer: Default::default(),
        mrs_acceleration: Default::default(),
        hit_audit: None,
        rule_publish_limit: 500,
        provider_cache: Default::default(),
        etag_support: Default::default(),
        json_documents: Vec::new(),
    }
}

/// The closed connection record the owner publishes.
fn closed_connection(
    id: &str,
    destination: &str,
    upload: u64,
    download: u64,
) -> ClosedConnectionSnapshot {
    ClosedConnectionSnapshot {
        id: id.into(),
        destination: destination.into(),
        duration_ms: 1200,
        upload_bytes: upload,
        download_bytes: download,
        rule: "DomainSuffix".into(),
        outbound: "HKG-01".into(),
        closed_at: "2026-10-16T11:59:00Z".into(),
    }
}

/// The open connection the owner publishes.
fn open_connection(id: &str, host: &str) -> ConnectionSnapshot {
    ConnectionSnapshot {
        id: id.into(),
        start: "2026-10-16T12:00:00Z".into(),
        destination_host: host.into(),
        host: host.into(),
        process: "curl".into(),
        rule: "Match".into(),
        rule_payload: "".into(),
        chain: "PROXIES -> HKG-01".into(),
        chains: vec!["PROXIES".into(), "HKG-01".into()],
        network: "tcp".into(),
        source_ip: "192.168.1.100".into(),
        source_port: "52140".into(),
        destination_ip: "1.1.1.1".into(),
        destination_port: "443".into(),
        destination_geo_ip: None,
        destination_ip_asn: "".into(),
        rate_observation: Default::default(),
        upload_bps: 100.0,
        download_bps: 2000.0,
        upload_total: 100,
        download_total: 5000,
    }
}

#[test]
fn the_lan_sharing_card_shows_the_owner_readback_and_says_not_observed_without_one() {
    let snapshot = desktop_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    surface.publish_snapshot(snapshot.clone());

    // The owner published no LAN sharing readback: the card says that the
    // sharing was never observed instead of claiming that it is off.
    assert_eq!(
        surface.window().get_lan_sharing_detail().to_string(),
        "Not observed"
    );

    // The owner published a confirmed configuration: the card shows the
    // owner's own facts, the enabled state and the bound port.
    let mut confirmed = snapshot.clone();
    confirmed.runtime_control.lan_sharing = Some(LanSharingSnapshot::new(4, true, 7890, "*"));
    surface.publish_snapshot(confirmed);
    let detail = surface.window().get_lan_sharing_detail().to_string();
    assert!(
        detail.contains("Enabled"),
        "the card must show the owner's confirmed state, got {detail:?}"
    );
    assert!(
        detail.contains("7890"),
        "the card must show the owner's confirmed port, got {detail:?}"
    );
    assert!(
        detail.contains('*'),
        "the card must show the owner's confirmed bind address, got {detail:?}"
    );

    // A confirmed configuration that is off is still an observation: the card
    // shows it as off and never as "not observed".
    let mut disabled = snapshot.clone();
    disabled.runtime_control.lan_sharing = Some(LanSharingSnapshot::new(4, false, 7890, "*"));
    surface.publish_snapshot(disabled);
    let detail = surface.window().get_lan_sharing_detail().to_string();
    assert!(
        detail.contains("Disabled"),
        "the card must show the owner's confirmed off state, got {detail:?}"
    );
    assert_ne!(detail, "Not observed");
}

#[test]
fn the_rule_list_filters_through_the_shared_page_model() {
    let mut snapshot = desktop_snapshot();
    snapshot.pages.rules = PageData::ready(rules_page());
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    surface.publish_snapshot(snapshot.clone());

    // Without a query every published rule is shown.
    assert_eq!(surface.rule_rows().len(), 2);

    // The shared model searches the whole expression of a rule, so the rule
    // the owner published without a raw expression stays reachable by its
    // payload.
    surface.search_rules("example.com");
    let rows = surface.rule_rows();
    assert_eq!(
        rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        ["2"],
        "the shared model finds the rule the surface's own filter lost"
    );

    // A query that matches nothing leaves the list empty.
    surface.search_rules("no-such-rule");
    assert!(surface.rule_rows().is_empty());

    // Clearing the query restores the published window.
    surface.search_rules("");
    assert_eq!(surface.rule_rows().len(), 2);
}

#[test]
fn the_closed_history_joins_the_shared_connection_model() {
    let mut snapshot = desktop_snapshot();
    snapshot.pages.connections = PageData::ready(ConnectionsPageSnapshot {
        connections: vec![open_connection("conn-1", "b.example")],
        total_connections: 1,
        total_upload_bytes: 100,
        total_download_bytes: 5000,
    });
    snapshot.closed_connections = ClosedConnectionsSnapshot {
        records: vec![
            closed_connection("closed-1", "a.example", 100, 5_000),
            closed_connection("closed-2", "b.example", 500, 1_000),
        ],
        total_closed: 2,
    };
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    surface.publish_snapshot(snapshot.clone());
    surface.set_connection_view(true);

    // The closed history is ordered by the shared reduction for every widget
    // dimension, exactly like the open list.
    let published = snapshot.closed_connections.records.clone();
    for (widget_value, key) in [
        ("time", ConnectionSortKey::LatestDesc),
        ("host", ConnectionSortKey::HostAsc),
        ("upload", ConnectionSortKey::UploadDesc),
        ("download", ConnectionSortKey::DownloadDesc),
    ] {
        surface.set_connection_sort(widget_value);
        let mut expected = published.clone();
        sort_connections(&mut expected, key);
        assert_eq!(
            surface
                .connection_rows()
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            expected.iter().map(|row| row.view_id()).collect::<Vec<_>>(),
            "the closed rows must be the shared {key:?} order"
        );
    }

    // The shared query folds the closed history by the facts the record
    // really carries.
    surface.window().set_connection_search("a.example".into());
    surface.publish_snapshot(snapshot.clone());
    let rows = surface.connection_rows();
    assert_eq!(
        rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        ["closed-1"],
        "the shared query keeps the record that matches it"
    );
}

#[test]
fn the_routing_mode_says_not_observed_when_the_owner_published_none() {
    let snapshot = desktop_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    surface.publish_snapshot(snapshot.clone());
    surface.open_page("AppRouting");

    // The owner published no routing policy: the panel says so instead of
    // naming a mode the owner never reported.
    let header = surface
        .workbench_item("app-routing")
        .expect("the routing panel is projected");
    assert_eq!(header.status, "Routing mode not observed");
    let summary = surface
        .workbench_item("app-routing-summary")
        .expect("the routing summary is projected");
    assert!(
        summary.detail.contains("mode not observed"),
        "the summary must say that the mode was not observed, got {:?}",
        summary.detail
    );
}
