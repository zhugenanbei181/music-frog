use super::{ColumnChoiceState, column_choices, column_chooser_mounted, mode_label_key};
use infiltrator_contract::connection_view_preference::{
    ConnectionColumn, ConnectionColumnSet, ConnectionViewIntent, ConnectionViewMode,
    ConnectionViewPreference,
};
use infiltrator_presentation::connection_view_projection::{
    CONNECTION_VIEW_MODE_LIST_KEY, CONNECTION_VIEW_MODE_TABLE_KEY,
    CONNECTION_VIEW_RESET_COLUMNS_KEY, CONNECTION_VIEW_TITLE_KEY, ConnectionViewProjection,
    project_connection_column_choices, project_connection_view,
};
use infiltrator_shared::locales::{Lang, Localizer};

fn projection(
    mode: ConnectionViewMode,
    columns: Vec<ConnectionColumn>,
) -> ConnectionViewProjection {
    let preference = ConnectionViewPreference {
        mode,
        visible_columns: ConnectionColumnSet::new(columns).unwrap(),
    };
    project_connection_view(&ConnectionColumnSet::all(), &preference)
}

#[test]
fn control_copy_comes_from_the_shared_catalog() {
    assert_eq!(
        mode_label_key(ConnectionViewMode::List),
        CONNECTION_VIEW_MODE_LIST_KEY
    );
    assert_eq!(
        mode_label_key(ConnectionViewMode::Table),
        CONNECTION_VIEW_MODE_TABLE_KEY
    );
    for key in [
        CONNECTION_VIEW_MODE_LIST_KEY,
        CONNECTION_VIEW_MODE_TABLE_KEY,
        CONNECTION_VIEW_RESET_COLUMNS_KEY,
        CONNECTION_VIEW_TITLE_KEY,
    ] {
        for language in ["zh-CN", "en-US"] {
            let label = Lang(language).tr(key);
            assert_ne!(label.as_ref(), key, "{key} must resolve in {language}");
        }
    }
    for choice in project_connection_column_choices() {
        for language in ["zh-CN", "en-US"] {
            let label = Lang(language).tr(choice.label_key);
            assert_ne!(
                label.as_ref(),
                choice.label_key,
                "{} must resolve in {}",
                choice.semantic_id,
                language
            );
        }
    }
}

#[test]
fn column_choices_are_the_shared_vocabulary_and_the_typed_readback() {
    let visible = vec![
        ConnectionColumn::Status,
        ConnectionColumn::Host,
        ConnectionColumn::Download,
    ];
    let choices = column_choices(&projection(ConnectionViewMode::Table, visible.clone()));
    let vocabulary = project_connection_column_choices();

    assert_eq!(choices.len(), vocabulary.len());
    for (choice, published) in choices.iter().zip(vocabulary) {
        assert_eq!(choice.semantic_id, published.semantic_id);
        assert_eq!(choice.label_key, published.label_key);
        assert_eq!(choice.visible, visible.contains(&published.column));
        assert_eq!(
            choice.intent,
            ConnectionViewIntent::ToggleColumn {
                column: published.column,
            }
        );
    }
}

#[test]
fn column_choices_never_invent_a_second_preference_state() {
    let first = projection(
        ConnectionViewMode::List,
        vec![ConnectionColumn::Status, ConnectionColumn::Host],
    );
    let second = projection(ConnectionViewMode::Table, vec![ConnectionColumn::Process]);
    let first_choices = column_choices(&first);
    let second_choices = column_choices(&second);

    assert_eq!(first_choices.len(), second_choices.len());
    assert_ne!(
        first_choices
            .iter()
            .map(|choice| choice.visible)
            .collect::<Vec<_>>(),
        second_choices
            .iter()
            .map(|choice| choice.visible)
            .collect::<Vec<_>>()
    );
    for choice in first_choices.iter().chain(second_choices.iter()) {
        assert!(matches!(
            choice.intent,
            ConnectionViewIntent::ToggleColumn { .. }
        ));
        assert!(choice.semantic_id.starts_with("connection.column."));
    }
    let marker = ColumnChoiceState {
        semantic_id: first_choices[0].semantic_id,
        label_key: first_choices[0].label_key,
        visible: !first_choices[0].visible,
        intent: first_choices[0].intent.clone(),
    };
    assert_ne!(marker, first_choices[0]);
}

#[test]
fn the_column_chooser_only_exists_in_the_table_morphology() {
    // The list morphology renders no column, so a chooser there would be a
    // control that cannot change what the user sees.
    assert!(!column_chooser_mounted(ConnectionViewMode::List));
    assert!(column_chooser_mounted(ConnectionViewMode::Table));
}
