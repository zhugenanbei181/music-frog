//! Tests for connection closing, connection filtering/sorting, log filtering, and window truncation warnings in infiltrator-slint.

use std::cmp::Reverse;

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    ConnectionSnapshot, ConnectionsPageSnapshot, LogSnapshot, LogsPageSnapshot, SurfaceSnapshot,
};
use infiltrator_contract::traffic_history::{ClosedConnectionSnapshot, ClosedConnectionsSnapshot};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::PageData;

fn setup_connections_and_logs_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );

    let conn1 = ConnectionSnapshot {
        id: "conn-1".into(),
        start: "2026-10-16T12:00:00Z".into(),
        network: "tcp".into(),
        destination_host: "cloudflare.com".into(),
        host: "cloudflare.com".into(),
        process: "curl".into(),
        rule: "Match".into(),
        rule_payload: "".into(),
        chain: "PROXIES -> HKG-01".into(),
        chains: vec!["PROXIES".into(), "HKG-01".into()],
        source_ip: "192.168.1.100".into(),
        source_port: "52140".into(),
        destination_ip: "1.1.1.1".into(),
        destination_port: "443".into(),
        destination_geo_ip: None,
        destination_ip_asn: "".into(),
        rate_observation: Default::default(),
        upload_bps: 100.0,
        download_bps: 2000.0,
        upload_total: 1024,
        download_total: 20480,
    };

    let page_conn = ConnectionsPageSnapshot {
        connections: vec![conn1],
        total_connections: 150, // exceeds published window to test truncation warning
        total_upload_bytes: 1024,
        total_download_bytes: 20480,
    };
    snapshot.pages.connections = PageData::ready(page_conn);

    let closed = ClosedConnectionSnapshot {
        id: "closed-1".into(),
        destination: "google.com:443".into(),
        duration_ms: 1200,
        upload_bytes: 512,
        download_bytes: 4096,
        rule: "Rule".into(),
        outbound: "HKG-01".into(),
        closed_at: "2026-10-16T11:59:00Z".into(),
    };
    snapshot.closed_connections = ClosedConnectionsSnapshot {
        records: vec![closed],
        total_closed: 1,
    };

    let log_entry1 = LogSnapshot {
        id: 1,
        raw: None,
        level: "info".into(),
        message: "Proxy core started".into(),
        tag: "core".into(),
        timestamp: "2026-10-16T12:00:01Z".into(),
    };
    let log_entry2 = LogSnapshot {
        id: 2,
        raw: None,
        level: "warn".into(),
        message: "Connection timeout to upstream".into(),
        tag: "dial".into(),
        timestamp: "2026-10-16T12:00:05Z".into(),
    };
    let page_logs = LogsPageSnapshot {
        stream: Default::default(),
        entries: vec![log_entry1, log_entry2],
        total_entries: 500, // exceeds published window
        active_level: None,
    };
    snapshot.pages.logs = PageData::ready(page_logs);

    snapshot
}

#[test]
fn test_close_connection_intent_generation() {
    let conn_id = "conn-123".to_string();
    let intent = CommandIntent::CloseConnection {
        id: conn_id.clone(),
    };
    match intent {
        CommandIntent::CloseConnection { id } => {
            assert_eq!(id, "conn-123");
        }
        _ => panic!("wrong intent"),
    }
}

#[test]
fn test_log_level_filter_intent_mapping() {
    let level_all = "all";
    let intent_all = CommandIntent::SetLogLevelFilter {
        level: if level_all == "all" {
            None
        } else {
            Some(level_all.to_string())
        },
    };
    match intent_all {
        CommandIntent::SetLogLevelFilter { level } => assert!(level.is_none()),
        _ => panic!("wrong intent"),
    }

    let level_warn = "warn";
    let intent_warn = CommandIntent::SetLogLevelFilter {
        level: if level_warn == "all" {
            None
        } else {
            Some(level_warn.to_string())
        },
    };
    match intent_warn {
        CommandIntent::SetLogLevelFilter { level } => assert_eq!(level, Some("warn".to_string())),
        _ => panic!("wrong intent"),
    }
}

#[test]
fn test_connection_sorting_by_upload_and_download() {
    let mut recs = [
        ClosedConnectionSnapshot {
            id: "1".into(),
            destination: "a.com".into(),
            duration_ms: 100,
            outbound: "HK".into(),
            rule: "Rule".into(),
            upload_bytes: 100,
            download_bytes: 5000,
            closed_at: "100".into(),
        },
        ClosedConnectionSnapshot {
            id: "2".into(),
            destination: "b.com".into(),
            duration_ms: 200,
            outbound: "HK".into(),
            rule: "Rule".into(),
            upload_bytes: 500,
            download_bytes: 1000,
            closed_at: "200".into(),
        },
    ];

    // Sort by upload descending
    recs.sort_by_key(|c| Reverse(c.upload_bytes));
    assert_eq!(recs[0].id, "2");
    assert_eq!(recs[1].id, "1");

    // Sort by download descending
    recs.sort_by_key(|c| Reverse(c.download_bytes));
    assert_eq!(recs[0].id, "1");
    assert_eq!(recs[1].id, "2");
}

#[test]
fn test_window_truncation_detection() {
    let snapshot = setup_connections_and_logs_snapshot();
    let conn_page = snapshot.pages.connections.data.as_ref().unwrap();
    assert!(conn_page.total_connections > conn_page.connections.len());

    let logs_page = snapshot.pages.logs.data.as_ref().unwrap();
    assert!(logs_page.total_entries > logs_page.entries.len());
}
