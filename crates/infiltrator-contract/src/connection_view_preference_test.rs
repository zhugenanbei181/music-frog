use super::{
    ConnectionColumn, ConnectionColumnSet, ConnectionColumnSupport, ConnectionViewIntent,
    ConnectionViewMode, ConnectionViewPreference, ConnectionViewPreferenceError,
    apply_connection_view_intent,
};

const DEFAULT_WIRE_IDS: [&str; 12] = [
    "status",
    "establishTime",
    "type",
    "host",
    "process",
    "rule",
    "proxyChain",
    "remoteDestination",
    "uploadSpeed",
    "downloadSpeed",
    "upload",
    "download",
];

const ALL_WIRE_IDS: [&str; 23] = [
    "status",
    "establishTime",
    "type",
    "host",
    "sniffHost",
    "process",
    "processPath",
    "rule",
    "proxyChain",
    "sourceIP",
    "sourcePort",
    "destinationIP",
    "destinationPort",
    "inboundIP",
    "inboundPort",
    "uploadSpeed",
    "downloadSpeed",
    "upload",
    "download",
    "dscp",
    "remoteDestination",
    "dnsMode",
    "duration",
];

#[test]
fn default_preference_is_list_with_exact_ordered_default_columns() {
    let preference = ConnectionViewPreference::default();
    assert_eq!(preference.mode, ConnectionViewMode::List);
    assert_eq!(preference.visible_columns.wire_ids(), DEFAULT_WIRE_IDS);
    assert_eq!(
        ConnectionColumn::DEFAULT_VISIBLE
            .iter()
            .map(|column| column.wire_id())
            .collect::<Vec<_>>(),
        DEFAULT_WIRE_IDS
    );
    assert_eq!(
        serde_json::to_value(&preference).unwrap(),
        serde_json::json!({
            "mode": "list",
            "visible_columns": DEFAULT_WIRE_IDS,
        })
    );
}

#[test]
fn both_view_modes_and_every_column_wire_id_round_trip() {
    for mode in ConnectionViewMode::ALL {
        assert_eq!(
            ConnectionViewMode::from_wire_id(mode.wire_id()).unwrap(),
            mode
        );
        let encoded = serde_json::to_string(&mode).unwrap();
        assert_eq!(
            serde_json::from_str::<ConnectionViewMode>(&encoded).unwrap(),
            mode
        );
    }
    assert_eq!(
        ConnectionViewMode::from_wire_id("grid").unwrap_err(),
        ConnectionViewPreferenceError::UnknownMode("grid".to_owned())
    );

    for (index, column) in ConnectionColumn::ALL.into_iter().enumerate() {
        assert_eq!(column.wire_id(), ALL_WIRE_IDS[index]);
        assert_eq!(
            ConnectionColumn::from_wire_id(column.wire_id()).unwrap(),
            column
        );
        let encoded = serde_json::to_string(&column).unwrap();
        assert_eq!(
            serde_json::from_str::<ConnectionColumn>(&encoded).unwrap(),
            column
        );
    }
    assert_eq!(
        ConnectionColumn::from_wire_id("destinationIp").unwrap_err(),
        ConnectionViewPreferenceError::UnknownColumn("destinationIp".to_owned())
    );
}

#[test]
fn complete_registry_and_preference_round_trip_through_wire() {
    let registry = ConnectionColumnSet::from_wire_ids(ALL_WIRE_IDS).unwrap();
    let encoded_registry = serde_json::to_string(&registry).unwrap();
    assert_eq!(
        serde_json::from_str::<ConnectionColumnSet>(&encoded_registry).unwrap(),
        registry
    );
    assert_eq!(registry, ConnectionColumnSet::all());

    let preference = ConnectionViewPreference {
        mode: ConnectionViewMode::Table,
        visible_columns: ConnectionColumnSet::from_wire_ids(["dnsMode", "host", "processPath"])
            .unwrap(),
    };
    let encoded = serde_json::to_string(&preference).unwrap();
    assert_eq!(
        serde_json::from_str::<ConnectionViewPreference>(&encoded).unwrap(),
        preference
    );
}

#[test]
fn explicit_wire_parser_is_fail_closed() {
    let preference = ConnectionViewPreference::from_wire("table", &["host", "process"]).unwrap();
    assert_eq!(preference.mode, ConnectionViewMode::Table);
    assert_eq!(preference.visible_columns.wire_ids(), ["host", "process"]);
    assert_eq!(
        ConnectionViewPreference::from_wire("grid", &["host"]).unwrap_err(),
        ConnectionViewPreferenceError::UnknownMode("grid".to_owned())
    );
    assert_eq!(
        ConnectionViewPreference::from_wire("list", &[]).unwrap_err(),
        ConnectionViewPreferenceError::EmptyColumns
    );
    assert_eq!(
        ConnectionViewPreference::new(
            ConnectionViewMode::Table,
            ConnectionColumnSet::from_wire_ids(["host"]).unwrap(),
        )
        .unwrap()
        .mode,
        ConnectionViewMode::Table
    );
}

#[test]
fn illegal_column_sets_fail_closed() {
    assert_eq!(
        ConnectionColumnSet::new(Vec::new()).unwrap_err(),
        ConnectionViewPreferenceError::EmptyColumns
    );
    assert_eq!(
        ConnectionColumnSet::from_wire_ids(["host", "unknown", "process"]).unwrap_err(),
        ConnectionViewPreferenceError::UnknownColumn("unknown".to_owned())
    );
    assert_eq!(
        ConnectionColumnSet::from_wire_ids(["host", "process", "host"]).unwrap_err(),
        ConnectionViewPreferenceError::DuplicateColumn(ConnectionColumn::Host)
    );

    for invalid in [
        serde_json::json!({"mode": "list", "visible_columns": []}),
        serde_json::json!({"mode": "list", "visible_columns": ["host", "host"]}),
        serde_json::json!({"mode": "list", "visible_columns": ["unknown"]}),
        serde_json::json!({"mode": "grid", "visible_columns": ["host"]}),
        serde_json::json!({"mode": "list", "visible_columns": ["host"], "extra": true}),
    ] {
        assert!(
            serde_json::from_value::<ConnectionViewPreference>(invalid.clone()).is_err(),
            "invalid preference must be rejected: {invalid}"
        );
    }
}

#[test]
fn set_view_mode_transition_is_exhaustive_and_keeps_columns() {
    let current = ConnectionViewPreference::default();
    let registry = ConnectionColumnSet::all();
    for mode in ConnectionViewMode::ALL {
        assert_eq!(
            apply_connection_view_intent(
                &current,
                &registry,
                ConnectionViewIntent::SetViewMode { mode },
            )
            .unwrap(),
            ConnectionViewPreference {
                mode,
                visible_columns: current.visible_columns.clone(),
            }
        );
    }
}

#[test]
fn toggle_column_transition_removes_and_appends_new_column() {
    let current = ConnectionViewPreference::default();
    let registry = ConnectionColumnSet::all();

    let removed = apply_connection_view_intent(
        &current,
        &registry,
        ConnectionViewIntent::ToggleColumn {
            column: ConnectionColumn::Process,
        },
    )
    .unwrap();
    assert!(!removed.visible_columns.contains(ConnectionColumn::Process));
    assert_eq!(removed.mode, ConnectionViewMode::List);

    let added = apply_connection_view_intent(
        &removed,
        &registry,
        ConnectionViewIntent::ToggleColumn {
            column: ConnectionColumn::ProcessPath,
        },
    )
    .unwrap();
    assert_eq!(
        added.visible_columns.wire_ids(),
        [
            "status",
            "establishTime",
            "type",
            "host",
            "rule",
            "proxyChain",
            "remoteDestination",
            "uploadSpeed",
            "downloadSpeed",
            "upload",
            "download",
            "processPath",
        ]
    );

    let one_column = ConnectionViewPreference {
        mode: ConnectionViewMode::Table,
        visible_columns: ConnectionColumnSet::from_wire_ids(["host"]).unwrap(),
    };
    assert_eq!(
        apply_connection_view_intent(
            &one_column,
            &registry,
            ConnectionViewIntent::ToggleColumn {
                column: ConnectionColumn::Host,
            },
        )
        .unwrap_err(),
        ConnectionViewPreferenceError::WouldRemoveLastColumn
    );

    let narrow_registry = ConnectionColumnSet::from_wire_ids(["host"]).unwrap();
    assert_eq!(
        apply_connection_view_intent(
            &one_column,
            &narrow_registry,
            ConnectionViewIntent::ToggleColumn {
                column: ConnectionColumn::Process,
            },
        )
        .unwrap_err(),
        ConnectionViewPreferenceError::ColumnOutsideRegistry(ConnectionColumn::Process)
    );
}

#[test]
fn reset_columns_transition_restores_exact_default_order() {
    let current = ConnectionViewPreference {
        mode: ConnectionViewMode::Table,
        visible_columns: ConnectionColumnSet::from_wire_ids(["dnsMode", "host"]).unwrap(),
    };
    let reset = apply_connection_view_intent(
        &current,
        &ConnectionColumnSet::all(),
        ConnectionViewIntent::ResetColumns,
    )
    .unwrap();
    assert_eq!(reset.mode, ConnectionViewMode::Table);
    assert_eq!(
        reset,
        ConnectionViewPreference {
            mode: ConnectionViewMode::Table,
            ..Default::default()
        }
    );
    assert_eq!(reset.visible_columns.wire_ids(), DEFAULT_WIRE_IDS);
}

#[test]
fn a_column_the_kernel_never_reports_is_marked_unreported() {
    for column in ConnectionColumn::ALL {
        let expected = match column {
            ConnectionColumn::SniffHost
            | ConnectionColumn::InboundIp
            | ConnectionColumn::InboundPort
            | ConnectionColumn::Dscp => ConnectionColumnSupport::Unreported,
            _ => ConnectionColumnSupport::Reported,
        };
        assert_eq!(column.support(), expected, "{column:?}");
        assert_eq!(
            column.is_reported(),
            expected == ConnectionColumnSupport::Reported
        );
    }
}

#[test]
fn every_default_visible_column_is_a_reported_fact() {
    for column in ConnectionColumn::DEFAULT_VISIBLE {
        assert!(
            column.is_reported(),
            "the default table must not open on a column the kernel never publishes: {column:?}"
        );
    }
}
