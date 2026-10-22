//! The shared fixture of the Slint business panel tests.
//!
//! Every panel of the fixture observes the shared read models of the
//! businesses it carries, so a driven test sees the same panel a capture
//! photographs.

use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::rule_document::RuleDocumentSnapshot;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    ConnectionsPageSnapshot, LogsPageSnapshot, RulesPageSnapshot, SurfaceSnapshot,
};
use infiltrator_slint::surface::action::workbench::business;
use infiltrator_slint::surface::action::workbench::business::{
    cancel_action, confirm_action, open_action, refresh_action,
};
use infiltrator_slint::surface::business::{BusinessId, BusinessPhase};
use infiltrator_slint::surface::harness::{SurfaceHarness, SurfaceOutcome};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::details::OutboundSnapshot;
use lurker_core_contract::outbound::identity::{OutboundId, SnapshotRevision};
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
    OutboundSelectionState, SelectionAvailability,
};
use lurker_core_contract::outbound::preferences::{
    OutboundHealthFilter, OutboundLayout, OutboundSortOrder, OutboundUiPreferencesSnapshot,
    OutboundUiPreferencesValues,
};
use lurker_core_contract::page::{PageData, PageStatus};

/// The owner snapshot the business panels observe.
pub fn business_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    snapshot.failure = None;
    snapshot.core.failure = None;
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.core.identity = Some(core_identity());
    publish_outbound_inventory(&mut snapshot);
    publish_rule_document(&mut snapshot);
    publish_log_stream(&mut snapshot);
    publish_connections(&mut snapshot);
    snapshot
}

/// The connections the fixture publishes.
///
/// The drawer shows the connections the owner published, so the fixture
/// publishes more than one: a reorder and a removal are only observable when
/// the published list has something to reorder or to remove.
fn publish_connections(snapshot: &mut SurfaceSnapshot) {
    let page: ConnectionsPageSnapshot = serde_json::from_value(serde_json::json!({
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
    snapshot.pages.connections = PageData::ready(page);
}

/// The log stream the fixture publishes.
fn publish_log_stream(snapshot: &mut SurfaceSnapshot) {
    let page: LogsPageSnapshot = serde_json::from_value(serde_json::json!({
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
    snapshot.pages.logs = PageData::ready(page);
}

/// The outbound inventory the fixture publishes.
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
    .expect("valid fixture inventory");
    snapshot.outbound_inventory = PageData::ready(inventory);
    let preferences = OutboundUiPreferencesSnapshot::new(
        scope,
        SnapshotRevision::new(1),
        OutboundUiPreferencesValues {
            search_query: String::new(),
            favorite_outbound_ids: Vec::new(),
            group_order: vec![id("PROXIES")],
            collapsed_group_ids: Vec::new(),
            sort_order: OutboundSortOrder::LatencyAscending,
            health_filter: OutboundHealthFilter::All,
            layout: OutboundLayout::CompactList,
        },
    )
    .expect("valid fixture preferences");
    snapshot.outbound_ui_preferences = PageData::ready(preferences);
}

/// The snapshot whose outbound inventory carries the given groups.
pub fn inventory_snapshot(groups: &[&str]) -> SurfaceSnapshot {
    let mut snapshot = business_snapshot();
    let scope = core_scope();
    let engine = scope.identity.engine.clone();
    let id = |value: &str| OutboundId::new(engine.clone(), value).expect("valid outbound id");
    let inventory = OutboundInventorySnapshot::new(
        scope,
        SnapshotRevision::new(2),
        OutboundInventoryState::Ready {
            entries: vec![OutboundSnapshot {
                id: id("HK-01"),
                display_name: Some("HK IPLC 01".to_owned()),
            }],
            collections: groups
                .iter()
                .map(|group| OutboundSelectionSet {
                    id: id(group),
                    member_ids: vec![id("HK-01")],
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready {
                        selected: id("HK-01"),
                    },
                })
                .collect(),
        },
    )
    .expect("valid fixture inventory");
    snapshot.outbound_inventory = PageData::ready(inventory);
    snapshot
}

/// The snapshot whose outbound inventory carries the given nodes.
///
/// The node list is the owner's fact, so a panel that shows the nodes follows
/// the order and the removals the owner published.
pub fn inventory_nodes_snapshot(nodes: &[(&str, &str)]) -> SurfaceSnapshot {
    let mut snapshot = business_snapshot();
    let scope = core_scope();
    let engine = scope.identity.engine.clone();
    let id = |value: &str| OutboundId::new(engine.clone(), value).expect("valid outbound id");
    let entries = nodes
        .iter()
        .map(|(node_id, name)| OutboundSnapshot {
            id: id(node_id),
            display_name: Some((*name).to_owned()),
        })
        .collect();
    let selected = nodes
        .first()
        .map(|(node_id, _)| *node_id)
        .unwrap_or("HK-01");
    let inventory = OutboundInventorySnapshot::new(
        scope,
        SnapshotRevision::new(3),
        OutboundInventoryState::Ready {
            entries,
            collections: vec![OutboundSelectionSet {
                id: id("PROXIES"),
                member_ids: nodes.iter().map(|(node_id, _)| id(node_id)).collect(),
                availability: SelectionAvailability::Selectable,
                selection: OutboundSelectionState::Ready {
                    selected: id(selected),
                },
            }],
        },
    )
    .expect("valid fixture inventory");
    snapshot.outbound_inventory = PageData::ready(inventory);
    snapshot
}

/// The rule document the fixture publishes.
fn publish_rule_document(snapshot: &mut SurfaceSnapshot) {
    let mut page: RulesPageSnapshot = serde_json::from_value(serde_json::json!({
        "document": null,
        "total_rules": 1,
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
        }]
    }))
    .expect("rules page JSON");
    page.document = Some(RuleDocumentSnapshot {
        source: RuleSourceIdentity {
            profile: "fixture".to_owned(),
            document_hash: "0".repeat(64),
        },
        rules: Vec::new(),
        sub_rules: Default::default(),
        targets: vec!["PROXIES".to_owned()],
    });
    snapshot.pages.rules = PageData::ready(page);
}

/// The core identity the fixture publishes.
pub fn core_identity() -> CoreIdentity {
    CoreIdentity {
        engine: EngineId::new("mihomo").expect("valid engine id"),
        instance: CoreInstanceId::new("fixture-1").expect("valid instance id"),
        version: None,
    }
}

/// The core scope the fixture publishes.
pub fn core_scope() -> CoreScope {
    CoreScope {
        identity: core_identity(),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

/// The headless surface with one business workbench page open.
pub fn business_surface(page: &str) -> SurfaceHarness {
    let snapshot = business_snapshot();
    let surface = SurfaceHarness::new(Some(snapshot.clone())).expect("headless surface");
    surface.publish_snapshot(snapshot);
    surface.open_page(page);
    surface
}

/// Opens the review of one business panel.
pub fn open(surface: &SurfaceHarness, business: BusinessId) -> SurfaceOutcome {
    surface.workbench_action(&open_action(business), "")
}

/// Confirms the review of one business panel.
pub fn confirm(surface: &SurfaceHarness, business: BusinessId) -> SurfaceOutcome {
    surface.workbench_action(&confirm_action(business), "")
}

/// Closes the review of one business panel.
pub fn cancel(surface: &SurfaceHarness, business: BusinessId) -> SurfaceOutcome {
    surface.workbench_action(&cancel_action(business), "")
}

/// Asks the owner for a fresh read of one business panel.
pub fn refresh(surface: &SurfaceHarness, business: BusinessId) -> SurfaceOutcome {
    surface.workbench_action(&refresh_action(business), "")
}

/// The phase of one business panel.
pub fn phase(surface: &SurfaceHarness, business: BusinessId) -> BusinessPhase {
    business::phase(surface.workbenches(), business)
}

/// The workbench cell one business panel projects.
pub fn cell(surface: &SurfaceHarness, id: &str) -> Option<infiltrator_slint::WorkbenchItem> {
    surface.workbench_item(id)
}

/// The identity of the cell one business panel projects.
pub fn cell_id(business: BusinessId) -> String {
    business.name().to_owned()
}

/// The identity of the review cell one business panel projects.
pub fn review_id(business: BusinessId) -> String {
    format!("{}/confirm", business.name())
}

/// Asserts that the review of one business panel is open.
pub fn assert_review_open(surface: &SurfaceHarness, business: BusinessId) {
    assert!(
        cell(surface, &review_id(business)).is_some(),
        "{} must open its own review",
        business.name()
    );
}

/// Asserts that the review of one business panel is closed.
pub fn assert_review_closed(surface: &SurfaceHarness, business: BusinessId) {
    assert!(
        cell(surface, &review_id(business)).is_none(),
        "{} must close its own review",
        business.name()
    );
}

/// Asserts that one business panel shows the phase the surface owns.
pub fn assert_phase(surface: &SurfaceHarness, business: BusinessId, expected: BusinessPhase) {
    assert_eq!(
        phase(surface, business),
        expected,
        "{} must show its own phase",
        business.name()
    );
}

/// The typed refusal the owner answers a business command with.
pub fn refusal() -> Failure {
    Failure::new(ErrorCode::Internal, "the owner refused the command", true)
}

/// Asserts that one business panel waits for the owner's answer.
pub fn assert_pending(surface: &SurfaceHarness, business: BusinessId) {
    open(surface, business);
    confirm(surface, business);
    assert_phase(surface, business, BusinessPhase::Pending);
}

/// Asserts that one business panel shows the owner's acceptance.
pub fn assert_accepted(surface: &SurfaceHarness, business: BusinessId) {
    open(surface, business);
    confirm(surface, business);
    surface.publish_owner_success();
    assert_phase(surface, business, BusinessPhase::Accepted);
}

/// Asserts that one business panel shows the owner's refusal.
pub fn assert_refused(surface: &SurfaceHarness, business: BusinessId) {
    open(surface, business);
    confirm(surface, business);
    surface.publish_owner_failure(&refusal());
    assert!(
        matches!(phase(surface, business), BusinessPhase::Refused { .. }),
        "{} must show the owner's own refusal",
        business.name()
    );
}

/// Asserts that one business panel can be dismissed without the owner.
pub fn assert_dismissed(surface: &SurfaceHarness, business: BusinessId) {
    open(surface, business);
    cancel(surface, business);
    assert_phase(surface, business, BusinessPhase::Idle);
    assert_review_closed(surface, business);
}

/// Asserts that one business panel asks the owner again after a refusal.
pub fn assert_retried(surface: &SurfaceHarness, business: BusinessId) {
    open(surface, business);
    confirm(surface, business);
    surface.publish_owner_failure(&refusal());
    confirm(surface, business);
    assert_phase(surface, business, BusinessPhase::Pending);
}

/// Asserts that one business panel asks the owner for a fresh read.
pub fn assert_refreshed(surface: &SurfaceHarness, business: BusinessId) {
    let before = surface.read_requests();
    refresh(surface, business);
    assert!(
        surface.read_requests() > before,
        "{} must ask the owner for its next read",
        business.name()
    );
}

/// Asserts that one business panel keeps its phase when a foreign readback
/// arrives.
pub fn assert_stable(surface: &SurfaceHarness, business: BusinessId) {
    open(surface, business);
    confirm(surface, business);
    surface.publish_snapshot(business_snapshot());
    assert_phase(surface, business, BusinessPhase::Pending);
}

/// Asserts that one business panel keeps its review when the page is
/// re-rendered.
pub fn assert_keeps_review(surface: &SurfaceHarness, page: &str, business: BusinessId) {
    open(surface, business);
    surface.open_page(page);
    assert_review_open(surface, business);
}

/// Asserts that one business panel keeps its review when the owner changes
/// the language.
pub fn assert_locale(surface: &SurfaceHarness, business: BusinessId) {
    open(surface, business);
    let mut snapshot = business_snapshot();
    snapshot.language_settings.preference = Some(LanguagePreference::English);
    surface.publish_snapshot(snapshot);
    assert_review_open(surface, business);
}

/// The snapshot whose read model the owner cannot serve.
///
/// The typed reason is the owner's own answer, so a panel shows it instead of
/// a fact the owner never published.
pub fn unavailable_snapshot(page: &str) -> SurfaceSnapshot {
    let mut snapshot = business_snapshot();
    let failure = Failure::unsupported("the host cannot serve this business");
    let status = PageStatus::Unavailable { failure };
    match page {
        "ConnectionsWorkbench" => snapshot.pages.connections.status = status,
        "LogsWorkbench" => snapshot.pages.logs.status = status,
        "RulesWorkbench" => snapshot.pages.rules.status = status,
        "Filters" => snapshot.pages.profiles.status = status,
        "SettingsWorkbench" => snapshot.pages.settings.status = status,
        "ProxiesWorkbench" => snapshot.outbound_inventory.status = status,
        "RuntimeWorkbench" => {
            snapshot.shell_readout.rate_failure = Some(Failure::unsupported(
                "the host cannot read the runtime telemetry",
            ));
        }
        "ShellWorkbench" => {
            snapshot.failure = Some(Failure::unsupported("the host cannot serve the shell"));
        }
        _ => {}
    }
    snapshot
}

/// The snapshot whose read model carries no observation.
pub fn empty_snapshot(page: &str) -> SurfaceSnapshot {
    let mut snapshot = business_snapshot();
    let status = PageStatus::Empty;
    match page {
        "ConnectionsWorkbench" => snapshot.pages.connections.status = status,
        "LogsWorkbench" => snapshot.pages.logs.status = status,
        "RulesWorkbench" => snapshot.pages.rules.status = status,
        "Filters" => snapshot.pages.profiles.status = status,
        "SettingsWorkbench" => snapshot.pages.settings.status = status,
        "ProxiesWorkbench" => snapshot.outbound_inventory.status = status,
        _ => {}
    }
    snapshot
}

/// Asserts that one business panel shows the owner's empty answer.
pub fn assert_empty(surface: &SurfaceHarness, page: &str, business: BusinessId) {
    surface.publish_snapshot(empty_snapshot(page));
    assert!(
        cell(surface, &format!("{}/empty", business.name())).is_some(),
        "{} must show the owner's empty answer",
        business.name()
    );
}

/// Asserts that one business panel shows the owner's typed refusal to serve
/// the business.
pub fn assert_unsupported(surface: &SurfaceHarness, page: &str, business: BusinessId) {
    surface.publish_snapshot(unavailable_snapshot(page));
    assert!(
        cell(surface, &format!("{}/failure", business.name())).is_some(),
        "{} must show the owner's typed reason",
        business.name()
    );
}
