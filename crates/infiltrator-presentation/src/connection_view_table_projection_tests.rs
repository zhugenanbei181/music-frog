use super::{
    ConnectionColumnWidth, ConnectionTableCell, ConnectionTableStatus, UNAVAILABLE, ellipsize,
    project_connection_view_table,
};
use crate::connection_view_projection::{ConnectionViewColumnProjection, project_connection_view};
use chrono::{DateTime, Utc};
use infiltrator_contract::connection::ConnectionRateObservation;
use infiltrator_contract::connection_view_preference::{
    ConnectionColumn, ConnectionColumnSet, ConnectionColumnSupport, ConnectionViewMode,
    ConnectionViewPreference,
};
use infiltrator_contract::traffic_history::ClosedConnectionSnapshot;
use infiltrator_domain::connection_observation::ConnectionObservationInstant;
use infiltrator_domain::connection_rate::ConnectionRate;
use infiltrator_domain::connection_view::ConnectionSortKey;
use infiltrator_domain::connection_view::RatedConnection;
use infiltrator_domain::runtime::{Connection, ConnectionMetadata};
use infiltrator_shared::byte_units::format_bytes;
use infiltrator_shared::effect_assets::text_width_em;

/// The instant the fixture rows were observed at.
fn observed_instant() -> ConnectionObservationInstant {
    ConnectionObservationInstant::observed_at(
        DateTime::parse_from_rfc3339("2026-11-13T09:05:00Z")
            .expect("instant")
            .with_timezone(&Utc),
    )
}

fn full_connection() -> Connection {
    Connection {
        id: "conn-1".into(),
        start: "2026-11-13T09:00:00Z".into(),
        rule: "DomainSuffix".into(),
        rule_payload: "example.org".into(),
        chains: vec!["HK-01".into(), "PROXIES".into()],
        upload: 2048,
        download: 4096,
        metadata: ConnectionMetadata {
            network: "tcp".into(),
            source_ip: "192.0.2.5".into(),
            destination_ip: "203.0.113.7".into(),
            source_port: "51000".into(),
            destination_port: "443".into(),
            host: "api.example.org".into(),
            dns_mode: "fake-ip".into(),
            process_path: "/usr/bin/client".into(),
            ..Default::default()
        },
    }
}

fn measured_rate() -> ConnectionRate {
    ConnectionRate {
        observation: ConnectionRateObservation::Measured,
        upload_bps: 2048.0,
        download_bps: 4096.0,
        upload_delta_bytes: 1024,
        download_delta_bytes: 2048,
    }
}

fn columns_of(visible: &[ConnectionColumn]) -> Vec<ConnectionViewColumnProjection> {
    let preference = ConnectionViewPreference {
        mode: ConnectionViewMode::Table,
        visible_columns: ConnectionColumnSet::new(visible.to_vec()).expect("column set"),
    };
    project_connection_view(&ConnectionColumnSet::all(), &preference).columns
}

fn table_of(
    visible: &[ConnectionColumn],
    connection: &Connection,
    status: ConnectionTableStatus,
) -> super::ConnectionTableProjection {
    let columns = columns_of(visible);
    let rows = vec![RatedConnection::new(connection, measured_rate())];
    project_connection_view_table(&columns, &rows, status, observed_instant(), None)
}

/// The header is the sort affordance: a rankable column publishes its key and
/// reads as the active rank only when it is the key the owner ranked by.
#[test]
fn a_rankable_header_publishes_its_key_and_only_the_active_one_is_marked() {
    let visible = [
        ConnectionColumn::Host,
        ConnectionColumn::Download,
        ConnectionColumn::Status,
    ];
    let columns = columns_of(&visible);
    let connection = full_connection();
    let rows = vec![RatedConnection::new(&connection, measured_rate())];
    let table = project_connection_view_table(
        &columns,
        &rows,
        ConnectionTableStatus::Active,
        observed_instant(),
        Some(ConnectionSortKey::DownloadDesc),
    );

    let by_column = |want: ConnectionColumn| {
        table
            .headers
            .iter()
            .find(|header| header.column == want)
            .unwrap_or_else(|| panic!("{want:?} missing from the header row"))
    };
    assert_eq!(
        by_column(ConnectionColumn::Host).sort,
        Some(ConnectionSortKey::HostAsc)
    );
    assert_eq!(
        by_column(ConnectionColumn::Download).sort,
        Some(ConnectionSortKey::DownloadDesc)
    );
    assert_eq!(by_column(ConnectionColumn::Status).sort, None);
    assert!(!by_column(ConnectionColumn::Host).sorted);
    assert!(by_column(ConnectionColumn::Download).sorted);
    assert!(!by_column(ConnectionColumn::Status).sorted);
}

/// A column the table cannot rank must never read as the active rank, so no
/// surface can paint a header that does nothing when pressed.
#[test]
fn an_unrankable_column_is_never_marked_sorted() {
    let columns = columns_of(&[ConnectionColumn::Status, ConnectionColumn::Rule]);
    let connection = full_connection();
    let rows = vec![RatedConnection::new(&connection, measured_rate())];
    let table = project_connection_view_table(
        &columns,
        &rows,
        ConnectionTableStatus::Active,
        observed_instant(),
        Some(ConnectionSortKey::HostAsc),
    );

    assert!(
        table.headers.iter().all(|header| header.sort.is_none()),
        "{:?}",
        table
            .headers
            .iter()
            .map(|h| (h.column, h.sort))
            .collect::<Vec<_>>()
    );
    assert!(table.headers.iter().all(|header| !header.sorted));
}

#[test]
fn the_table_publishes_the_users_columns_in_preference_order() {
    let visible = [
        ConnectionColumn::Status,
        ConnectionColumn::Host,
        ConnectionColumn::Download,
    ];
    let table = table_of(&visible, &full_connection(), ConnectionTableStatus::Active);

    assert_eq!(table.headers.len(), 3);
    assert_eq!(
        table
            .headers
            .iter()
            .map(|header| header.column)
            .collect::<Vec<_>>(),
        visible.to_vec()
    );
    assert_eq!(table.headers[0].semantic_id, "connection.column.status");
    assert_eq!(table.headers[1].semantic_id, "connection.column.host");
    assert_eq!(table.headers[2].semantic_id, "connection.column.download");
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].id, "conn-1");
    assert_eq!(table.rows[0].cells.len(), 3);
}

#[test]
fn a_column_the_kernel_never_publishes_stays_the_honest_unknown() {
    let unreported = [
        ConnectionColumn::SniffHost,
        ConnectionColumn::InboundIp,
        ConnectionColumn::InboundPort,
        ConnectionColumn::Dscp,
    ];
    let table = table_of(
        &unreported,
        &full_connection(),
        ConnectionTableStatus::Active,
    );

    for header in &table.headers {
        assert_eq!(header.support, ConnectionColumnSupport::Unreported);
    }
    for cell in &table.rows[0].cells {
        assert_eq!(cell, &ConnectionTableCell::Unavailable);
    }
}

#[test]
fn every_reported_column_renders_the_owner_value() {
    let reported: Vec<ConnectionColumn> = ConnectionColumn::ALL
        .into_iter()
        .filter(|column| column.is_reported())
        .collect();
    let table = table_of(&reported, &full_connection(), ConnectionTableStatus::Active);

    assert_eq!(table.rows[0].cells.len(), reported.len());
    for (column, cell) in reported.iter().zip(table.rows[0].cells.iter()) {
        assert!(
            !cell.is_unavailable(),
            "{column:?} must render the value the owner published"
        );
    }
    let host_cell = &table.rows[0].cells[reported
        .iter()
        .position(|column| *column == ConnectionColumn::Host)
        .expect("host column")];
    assert_eq!(host_cell.text(), Some("api.example.org"));
}

#[test]
fn a_closed_row_never_answers_the_establishment_question() {
    let table = table_of(
        &[ConnectionColumn::Status, ConnectionColumn::EstablishTime],
        &full_connection(),
        ConnectionTableStatus::Closed,
    );

    assert_eq!(
        table.rows[0].cells[0],
        ConnectionTableCell::Status(ConnectionTableStatus::Closed)
    );
    assert_eq!(table.rows[0].cells[1], ConnectionTableCell::Unavailable);
}

#[test]
fn an_unmeasured_rate_is_the_unknown_and_a_measured_rate_is_live() {
    let columns = columns_of(&[
        ConnectionColumn::UploadSpeed,
        ConnectionColumn::DownloadSpeed,
    ]);
    let connection = full_connection();
    let unmeasured = ConnectionRate {
        observation: ConnectionRateObservation::Unobserved,
        ..Default::default()
    };
    let rows = vec![RatedConnection::new(&connection, unmeasured)];
    let table = project_connection_view_table(
        &columns,
        &rows,
        ConnectionTableStatus::Active,
        observed_instant(),
        None,
    );
    assert_eq!(table.rows[0].cells[0], ConnectionTableCell::Unavailable);
    assert_eq!(table.rows[0].cells[1], ConnectionTableCell::Unavailable);

    let rows = vec![RatedConnection::new(&connection, measured_rate())];
    let table = project_connection_view_table(
        &columns,
        &rows,
        ConnectionTableStatus::Active,
        observed_instant(),
        None,
    );
    assert!(matches!(
        table.rows[0].cells[0],
        ConnectionTableCell::Live(_)
    ));
    assert!(matches!(
        table.rows[0].cells[1],
        ConnectionTableCell::Live(_)
    ));
}

#[test]
fn an_empty_value_is_the_unknown_and_a_zero_total_is_a_value() {
    let mut connection = full_connection();
    connection.metadata.host = String::new();
    connection.upload = 0;
    let table = table_of(
        &[ConnectionColumn::Host, ConnectionColumn::Upload],
        &connection,
        ConnectionTableStatus::Active,
    );

    assert_eq!(table.rows[0].cells[0], ConnectionTableCell::Unavailable);
    assert_eq!(
        table.rows[0].cells[1],
        ConnectionTableCell::Text(format_bytes(0))
    );
}

#[test]
fn the_column_width_follows_what_the_column_carries() {
    assert_eq!(
        super::connection_column_width(ConnectionColumn::Status),
        ConnectionColumnWidth::Narrow
    );
    assert_eq!(
        super::connection_column_width(ConnectionColumn::Rule),
        ConnectionColumnWidth::Regular
    );
    assert_eq!(
        super::connection_column_width(ConnectionColumn::ProxyChain),
        ConnectionColumnWidth::Wide
    );
}

#[test]
fn the_unknown_is_one_shared_honest_value() {
    assert_eq!(UNAVAILABLE, "—");
}

#[test]
fn the_chain_cell_names_the_outbound_and_the_hops_after_it() {
    let table = table_of(
        &[ConnectionColumn::ProxyChain],
        &full_connection(),
        ConnectionTableStatus::Active,
    );
    assert_eq!(
        table.rows[0].cells[0],
        ConnectionTableCell::Chain {
            outbound: "HK-01".to_owned(),
            rest: "PROXIES".to_owned(),
        }
    );
    assert_eq!(table.rows[0].cells[0].outbound(), Some("HK-01"));
}

#[test]
fn a_connection_without_a_chain_reports_the_honest_unknown() {
    let mut connection = full_connection();
    connection.chains.clear();
    let table = table_of(
        &[ConnectionColumn::ProxyChain],
        &connection,
        ConnectionTableStatus::Active,
    );
    assert_eq!(table.rows[0].cells[0], ConnectionTableCell::Unavailable);
    assert_eq!(table.rows[0].cells[0].outbound(), None);
}

#[test]
fn ellipsize_cuts_at_the_visual_width_and_marks_the_cut() {
    assert_eq!(ellipsize("short", 10.0), "short");
    let cut = ellipsize("api.example.org", 6.0);
    assert!(cut.ends_with('…'));
    assert!(cut.chars().count() < "api.example.org".chars().count());
    // A full-width character counts twice, so the same budget keeps fewer
    // characters of a CJK cell than of a latin one.
    let cjk = ellipsize("连接审计页面", 4.0);
    assert!(cjk.ends_with('…'));
    assert!(
        text_width_em(&cjk) <= 4.0,
        "the cut must stay inside the column: {cjk}"
    );
}

#[test]
fn a_live_row_publishes_its_age_against_the_owners_instant() {
    let table = table_of(
        &[ConnectionColumn::Duration],
        &full_connection(),
        ConnectionTableStatus::Active,
    );

    assert_eq!(table.rows[0].cells[0].text(), Some("5.0 min"));
}

#[test]
fn a_live_row_without_an_owner_instant_stays_the_honest_unknown() {
    let columns = columns_of(&[ConnectionColumn::Duration]);
    let connection = full_connection();
    let rows = vec![RatedConnection::new(&connection, measured_rate())];
    let table = project_connection_view_table(
        &columns,
        &rows,
        ConnectionTableStatus::Active,
        ConnectionObservationInstant::unobserved(),
        None,
    );

    assert_eq!(table.rows[0].cells[0], ConnectionTableCell::Unavailable);
}

#[test]
fn a_closed_row_publishes_the_length_it_lived() {
    let columns = columns_of(&[ConnectionColumn::Duration]);
    let closed = ClosedConnectionSnapshot {
        id: "conn-1".into(),
        destination: "api.example.org".into(),
        duration_ms: 204_000,
        upload_bytes: 2048,
        download_bytes: 4096,
        rule: "DomainSuffix".into(),
        outbound: "HK-01".into(),
        closed_at: "2026-11-13T09:05:00Z".into(),
    };
    let rows = vec![RatedConnection::new(&closed, ConnectionRate::default())];
    let table = project_connection_view_table(
        &columns,
        &rows,
        ConnectionTableStatus::Closed,
        ConnectionObservationInstant::unobserved(),
        None,
    );

    assert_eq!(table.rows[0].cells[0].text(), Some("3.4 min"));
}
