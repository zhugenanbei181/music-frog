use super::{
    CONNECTION_VIEW_MODE_LIST_KEY, CONNECTION_VIEW_MODE_TABLE_KEY,
    CONNECTION_VIEW_RESET_COLUMNS_KEY, CONNECTION_VIEW_TITLE_KEY, connection_view_mode_label_key,
    project_connection_column_choices, project_connection_view,
};
use infiltrator_contract::connection_view_preference::{
    ConnectionColumn, ConnectionColumnSet, ConnectionViewMode, ConnectionViewPreference,
};
use std::collections::BTreeSet;

#[test]
fn default_projection_preserves_preference_order_without_duplicates() {
    let registry = ConnectionColumnSet::all();
    let preference = ConnectionViewPreference::default();
    let projection = project_connection_view(&registry, &preference);

    assert_eq!(projection.mode, ConnectionViewMode::List);
    assert_eq!(
        projection
            .columns
            .iter()
            .map(|column| column.column)
            .collect::<Vec<_>>(),
        ConnectionColumn::DEFAULT_VISIBLE.to_vec()
    );
    assert_eq!(
        projection
            .columns
            .iter()
            .map(|column| column.semantic_id)
            .collect::<Vec<_>>(),
        [
            "connection.column.status",
            "connection.column.establish_time",
            "connection.column.type",
            "connection.column.host",
            "connection.column.process",
            "connection.column.rule",
            "connection.column.proxy_chain",
            "connection.column.remote_destination",
            "connection.column.upload_speed",
            "connection.column.download_speed",
            "connection.column.upload",
            "connection.column.download",
        ]
    );
}

#[test]
fn projection_uses_preference_order_when_registry_differs() {
    let registry = ConnectionColumnSet::from_wire_ids([
        "dnsMode",
        "processPath",
        "host",
        "status",
        "sourceIP",
    ])
    .unwrap();
    let preference = ConnectionViewPreference {
        mode: ConnectionViewMode::Table,
        visible_columns: ConnectionColumnSet::from_wire_ids([
            "status", "sourceIP", "host", "dnsMode",
        ])
        .unwrap(),
    };
    let projection = project_connection_view(&registry, &preference);

    assert_eq!(projection.mode, ConnectionViewMode::Table);
    assert_eq!(
        projection
            .columns
            .iter()
            .map(|column| column.column)
            .collect::<Vec<_>>(),
        [
            ConnectionColumn::Status,
            ConnectionColumn::SourceIp,
            ConnectionColumn::Host,
            ConnectionColumn::DnsMode,
        ]
    );
    assert_eq!(
        projection
            .columns
            .iter()
            .map(|column| column.semantic_id)
            .collect::<Vec<_>>(),
        [
            "connection.column.status",
            "connection.column.source_ip",
            "connection.column.host",
            "connection.column.dns_mode",
        ]
    );
}

#[test]
fn every_column_has_a_stable_semantic_id() {
    let mut ids = Vec::new();
    for column in ConnectionColumn::ALL {
        let preference = ConnectionViewPreference {
            mode: ConnectionViewMode::List,
            visible_columns: ConnectionColumnSet::new(vec![column]).unwrap(),
        };
        let projection = project_connection_view(&ConnectionColumnSet::all(), &preference);
        assert_eq!(projection.columns.len(), 1);
        assert_eq!(projection.columns[0].column, column);
        let id = projection.columns[0].semantic_id;
        assert!(id.starts_with("connection.column."));
        ids.push(id);
    }
    let unique: BTreeSet<_> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ConnectionColumn::ALL.len());
}

#[test]
fn unknown_preference_columns_are_absent_without_fallback() {
    let registry =
        ConnectionColumnSet::from_wire_ids(["host", "process", "rule", "remoteDestination"])
            .unwrap();
    let preference = ConnectionViewPreference {
        mode: ConnectionViewMode::Table,
        visible_columns: ConnectionColumnSet::from_wire_ids(["dnsMode", "host", "sniffHost"])
            .unwrap(),
    };
    let projection = project_connection_view(&registry, &preference);

    assert_eq!(
        projection
            .columns
            .iter()
            .map(|column| column.column)
            .collect::<Vec<_>>(),
        [ConnectionColumn::Host]
    );
    assert_eq!(projection.columns[0].semantic_id, "connection.column.host");
}

#[test]
fn duplicate_preference_ids_cannot_reach_projection() {
    assert!(ConnectionColumnSet::from_wire_ids(["host", "host"]).is_err());
}

#[test]
fn column_choices_publish_the_complete_registry_and_shared_copy_keys() {
    let choices = project_connection_column_choices();
    assert_eq!(choices.len(), ConnectionColumn::ALL.len());
    assert_eq!(
        choices
            .iter()
            .map(|choice| choice.column)
            .collect::<Vec<_>>(),
        ConnectionColumn::ALL.to_vec()
    );

    let mut keys = BTreeSet::new();
    for choice in &choices {
        let preference = ConnectionViewPreference {
            mode: ConnectionViewMode::List,
            visible_columns: ConnectionColumnSet::new(vec![choice.column]).unwrap(),
        };
        let projection = project_connection_view(&ConnectionColumnSet::all(), &preference);
        assert_eq!(projection.columns.len(), 1);
        assert_eq!(choice.semantic_id, projection.columns[0].semantic_id);
        assert_eq!(choice.label_key, choice.semantic_id);
        assert!(choice.semantic_id.starts_with("connection."));
        assert!(keys.insert(choice.label_key));
    }
}

#[test]
fn connection_view_control_keys_are_distinct_non_empty_shared_copy_keys() {
    let controls = [
        CONNECTION_VIEW_MODE_LIST_KEY,
        CONNECTION_VIEW_MODE_TABLE_KEY,
        CONNECTION_VIEW_RESET_COLUMNS_KEY,
        CONNECTION_VIEW_TITLE_KEY,
    ];
    let unique: BTreeSet<_> = controls.iter().copied().collect();
    assert_eq!(unique.len(), controls.len());
    for key in controls {
        assert!(!key.is_empty());
        assert!(key.starts_with("connection."));
    }
    assert_eq!(
        connection_view_mode_label_key(ConnectionViewMode::List),
        CONNECTION_VIEW_MODE_LIST_KEY
    );
    assert_eq!(
        connection_view_mode_label_key(ConnectionViewMode::Table),
        CONNECTION_VIEW_MODE_TABLE_KEY
    );
}
