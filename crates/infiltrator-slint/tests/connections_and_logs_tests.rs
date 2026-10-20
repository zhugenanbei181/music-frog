//! SLINT-001.8: Connection closing, connection sorting and log filtering of the
//! real Slint surface.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness. The connection sort order and the log level filter belong
//! to the shared projection in `infiltrator_slint::lumen_projection`; these
//! tests assert the exact typed owner command the surface emits and the rows
//! the surface projects.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    ConnectionSnapshot, ConnectionsPageSnapshot, LogSnapshot, LogsPageSnapshot, SurfaceSnapshot,
};
use infiltrator_contract::traffic_history::{ClosedConnectionSnapshot, ClosedConnectionsSnapshot};
use infiltrator_slint::SlintAction;
use infiltrator_slint::surface::harness;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::PageData;

fn connection(id: &str, host: &str, upload_total: u64, download_total: u64) -> ConnectionSnapshot {
    ConnectionSnapshot {
        id: id.into(),
        start: "2026-10-16T12:00:00Z".into(),
        network: "tcp".into(),
        destination_host: host.into(),
        host: host.into(),
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
        upload_total,
        download_total,
    }
}

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
        rule: "Rule".into(),
        outbound: "HKG-01".into(),
        closed_at: "2026-10-16T11:59:00Z".into(),
    }
}

fn observe_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );

    snapshot.pages.connections = PageData::ready(ConnectionsPageSnapshot {
        connections: vec![
            connection("conn-1", "a.example", 100, 5_000),
            connection("conn-2", "b.example", 500, 1_000),
        ],
        // Exceeds the published window, so the surface must warn about it.
        total_connections: 150,
        total_upload_bytes: 600,
        total_download_bytes: 6_000,
    });

    snapshot.closed_connections = ClosedConnectionsSnapshot {
        records: vec![
            closed_connection("closed-1", "a.example", 100, 5_000),
            closed_connection("closed-2", "b.example", 500, 1_000),
        ],
        total_closed: 2,
    };

    snapshot.pages.logs = PageData::ready(LogsPageSnapshot {
        stream: Default::default(),
        entries: vec![
            LogSnapshot {
                id: 1,
                raw: None,
                level: "info".into(),
                message: "Proxy core started".into(),
                tag: "core".into(),
                timestamp: "2026-10-16T12:00:01Z".into(),
            },
            LogSnapshot {
                id: 2,
                raw: None,
                level: "warn".into(),
                message: "Connection timeout to upstream".into(),
                tag: "dial".into(),
                timestamp: "2026-10-16T12:00:05Z".into(),
            },
        ],
        // Exceeds the published window, so the surface must warn about it.
        total_entries: 500,
        active_level: None,
    });

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

#[test]
fn closing_a_connection_dispatches_the_shared_close_command() {
    let surface = harness::SurfaceHarness::new(Some(observe_snapshot())).unwrap();

    let command = single_command(surface.close_connection("conn-123"));
    assert_eq!(
        command,
        CommandIntent::CloseConnection {
            id: "conn-123".to_owned()
        }
    );
    assert_eq!(surface.connection_breaking_id(), "conn-123");
}

#[test]
fn the_log_level_filter_dispatches_the_shared_filter_command() {
    let surface = harness::SurfaceHarness::new(Some(observe_snapshot())).unwrap();

    let command = single_command(surface.set_log_level("all"));
    assert_eq!(command, CommandIntent::SetLogLevelFilter { level: None });

    let command = single_command(surface.set_log_level("warn"));
    assert_eq!(
        command,
        CommandIntent::SetLogLevelFilter {
            level: Some("warn".to_owned())
        }
    );
}

#[test]
fn the_projection_sorts_active_connections_by_upload_and_download() {
    let snapshot = observe_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    surface.publish_snapshot(snapshot.clone());

    surface.set_connection_sort("upload");
    let rows = surface.connection_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "conn-2");
    assert_eq!(rows[1].id, "conn-1");

    surface.set_connection_sort("download");
    let rows = surface.connection_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "conn-1");
    assert_eq!(rows[1].id, "conn-2");
}

#[test]
fn the_projection_sorts_closed_connections_by_upload_and_download() {
    let snapshot = observe_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    surface.publish_snapshot(snapshot.clone());

    surface.set_connection_view(true);
    surface.set_connection_sort("upload");
    let rows = surface.connection_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "closed-2");
    assert_eq!(rows[1].id, "closed-1");

    surface.set_connection_sort("download");
    let rows = surface.connection_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "closed-1");
    assert_eq!(rows[1].id, "closed-2");
}

#[test]
fn the_projection_filters_the_log_rows_by_the_owner_level() {
    let mut snapshot = observe_snapshot();
    snapshot.pages.logs = PageData::ready(LogsPageSnapshot {
        stream: Default::default(),
        entries: vec![
            LogSnapshot {
                id: 1,
                raw: None,
                level: "info".into(),
                message: "Proxy core started".into(),
                tag: "core".into(),
                timestamp: "2026-10-16T12:00:01Z".into(),
            },
            LogSnapshot {
                id: 2,
                raw: None,
                level: "warn".into(),
                message: "Connection timeout to upstream".into(),
                tag: "dial".into(),
                timestamp: "2026-10-16T12:00:05Z".into(),
            },
        ],
        total_entries: 2,
        active_level: Some("warning".into()),
    });

    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();

    // The user picks the level; the owner applies it and publishes the
    // snapshot; the projection then filters the rows by that level.
    surface.set_log_level("warning");
    surface.publish_snapshot(snapshot.clone());

    let rows = surface.log_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "2");
    assert_eq!(rows[0].title, "Connection timeout to upstream");
}

#[test]
fn the_projection_narrows_the_log_window_by_the_search_text() {
    let snapshot = observe_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    surface.publish_snapshot(snapshot.clone());

    assert_eq!(surface.log_rows().len(), 2);

    surface.search_logs("timeout");
    let rows = surface.log_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "2");
}

#[test]
fn the_projection_warns_about_a_truncated_owner_window() {
    let snapshot = observe_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();

    surface.navigate("Connections");

    // The owner published 2 of 150 connections; the surface must say so
    // instead of presenting the window as the complete list.
    let warning = surface.snapshot_window_warning();
    assert!(
        warning.contains("2") && warning.contains("150"),
        "the truncation warning must name the published and total counts, got {warning:?}"
    );
}
