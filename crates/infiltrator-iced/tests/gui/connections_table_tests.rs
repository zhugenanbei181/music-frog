use super::*;
use infiltrator_contract::connection_view_preference::{
    ConnectionColumn, ConnectionColumnSet, ConnectionViewMode, ConnectionViewPreference,
};
use infiltrator_contract::design_resources::TextRole;
use infiltrator_presentation::connection_view_projection::project_connection_view;
use infiltrator_presentation::connection_view_table_projection::{
    ConnectionColumnWidth, ConnectionTableTone, connection_column_tone, connection_column_width,
};

#[test]
fn the_column_widths_follow_the_projection_classes() {
    assert!(
        column_width(ConnectionColumnWidth::Narrow) < column_width(ConnectionColumnWidth::Regular)
    );
    assert!(
        column_width(ConnectionColumnWidth::Regular) < column_width(ConnectionColumnWidth::Wide)
    );
}

#[test]
fn every_column_of_the_registry_has_a_width_and_a_tone() {
    for column in ConnectionColumn::ALL {
        let width = connection_column_width(column);
        assert!(matches!(
            width,
            ConnectionColumnWidth::Narrow
                | ConnectionColumnWidth::Regular
                | ConnectionColumnWidth::Wide
        ));
        assert!(matches!(
            connection_column_tone(column),
            ConnectionTableTone::Plain
                | ConnectionTableTone::Accent
                | ConnectionTableTone::Muted
                | ConnectionTableTone::Numeric
        ));
    }
}

#[test]
fn a_cell_budget_never_hides_the_text_entirely() {
    for role in [TextRole::Label, TextRole::Caption, TextRole::Mono] {
        for width in [
            column_width(ConnectionColumnWidth::Narrow),
            column_width(ConnectionColumnWidth::Regular),
            column_width(ConnectionColumnWidth::Wide),
        ] {
            assert!(text_budget_em(width, role) >= 1.0);
        }
    }
}

#[test]
fn the_table_reads_the_users_own_columns() {
    let preference = ConnectionViewPreference {
        mode: ConnectionViewMode::Table,
        visible_columns: ConnectionColumnSet::new(vec![
            ConnectionColumn::Status,
            ConnectionColumn::Host,
            ConnectionColumn::ProxyChain,
        ])
        .expect("column set"),
    };
    let projection = project_connection_view(&ConnectionColumnSet::all(), &preference);
    assert_eq!(projection.mode, ConnectionViewMode::Table);
    assert_eq!(projection.columns.len(), 3);
    assert_eq!(projection.columns[0].column, ConnectionColumn::Status);
    assert_eq!(projection.columns[2].column, ConnectionColumn::ProxyChain);
}
