//! test-intent: behavior
use super::*;
use crate::native_input::{click_entity, replace_text, type_text};
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::profiles_editor_filter::{EditorFilterDiscard, EditorFilterText};
use infiltrator_bevy_ui::pages::profiles_editor_panes::{
    EditorFilterSaveButton, ProfileEditorOptionsState,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::subscription_filter_result::{FilterReport, SubscriptionFilterApplied};

fn mount() -> (App, Arc<DemoCommandSink>, ProfileOptionsSnapshot) {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.add_plugins(ButtonPlugin);
    navigate_to(&mut app, Route::Profiles);
    let observed = observation("main", FIXTURE_DOCUMENT, Default::default()).unwrap();
    let options = ProfileOptionsSnapshot::new(observed.source, "{}\n", observed.filter);
    replay(&mut app, options.clone());
    let tab = pane_button_entity(&mut app, ProfileEditorPane::Filter);
    click_entity(&mut app, tab);
    (app, sink, options)
}
fn replay(app: &mut App, options: ProfileOptionsSnapshot) {
    app.world_mut()
        .trigger(ProfilesProjectionUpdated(editor_options_page_projection(
            FIXTURE_DOCUMENT,
            Some(options),
        )));
    app.update();
}
fn input(app: &mut App, kind: FilterField) -> Entity {
    app.world_mut()
        .query::<(Entity, &EditorFilterText)>()
        .iter(app.world())
        .find(|(_, marker)| marker.0 == kind)
        .unwrap()
        .0
}

#[test]
fn native_document_filter_hidden_tab_releases_focus_and_source_change_requires_discard() {
    let (mut app, sink, options) = mount();
    let field = input(&mut app, FilterField::Include);
    click_entity(&mut app, field);
    type_text(&mut app, "HK");
    assert_eq!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .filter
            .draft
            .include,
        "HK"
    );
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    let original = app.world().get::<TextField>(field).unwrap().0.clone();
    replay(&mut app, options.clone());
    assert_eq!(app.world().get::<TextField>(field).unwrap().0, original);
    let tab = pane_button_entity(&mut app, ProfileEditorPane::Mixin);
    click_entity(&mut app, tab);
    assert!(!app.world().get::<TextFieldFocused>(field).unwrap().0);
    type_text(&mut app, "hidden-input");
    assert_eq!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .filter
            .draft
            .include,
        "HK"
    );
    let tab = pane_button_entity(&mut app, ProfileEditorPane::Filter);
    click_entity(&mut app, tab);
    let changed = observation(
        "main",
        "# externally edited\nmode: rule\n",
        Default::default(),
    )
    .unwrap();
    let changed = ProfileOptionsSnapshot::new(changed.source, "{}\n", changed.filter);
    app.world_mut()
        .trigger(ProfilesProjectionUpdated(editor_options_page_projection(
            "# externally edited\nmode: rule\n",
            Some(changed.clone()),
        )));
    app.update();
    assert_eq!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .filter
            .draft
            .include,
        "HK"
    );
    assert!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .filter
            .stale()
    );
    let save = marker_entity::<EditorFilterSaveButton>(&mut app);
    assert!(app.world().get::<ButtonDisabled>(save).unwrap().0);
    let before = sink.submitted().len();
    click_entity(&mut app, save);
    assert_eq!(sink.submitted().len(), before);
    let discard = marker_entity::<EditorFilterDiscard>(&mut app);
    click_entity(&mut app, discard);
    assert_eq!(sink.submitted().len(), before, "discard never writes");
    let editor = &app.world().resource::<ProfileEditorOptionsState>().filter;
    assert!(editor.current());
    assert_eq!(editor.draft.include, "");
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), "");
}

#[test]
fn native_document_filter_rejects_unit_receipt_then_retries_and_keeps_actual_statistics() {
    let (mut app, sink, _) = mount();
    let field = input(&mut app, FilterField::Include);
    click_entity(&mut app, field);
    type_text(&mut app, "HK");
    let save = marker_entity::<EditorFilterSaveButton>(&mut app);
    click_entity(&mut app, save);
    let (request_id, token) = app
        .world()
        .resource::<ProfileEditorOptionsState>()
        .filter_request
        .unwrap();
    let command = sink.submitted().last().unwrap().clone();
    let discard = marker_entity::<EditorFilterDiscard>(&mut app);
    let before = sink.submitted().len();
    click_entity(&mut app, discard);
    assert_eq!(sink.submitted().len(), before);
    assert_eq!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .filter
            .pending
            .as_ref()
            .unwrap()
            .token,
        token
    );
    app.world_mut().trigger(CommandExecutedEvent {
        request_id,
        command,
        result: Ok(CommandOutput::Unit),
    });
    app.update();
    let editor = &app.world().resource::<ProfileEditorOptionsState>().filter;
    assert_eq!(
        editor.failure.as_ref().unwrap().code,
        ErrorCode::InvalidState
    );
    assert!(!editor.applied);
    assert!(editor.report.is_none());
    assert_eq!(editor.draft.include, "HK");
    click_entity(&mut app, save);
    let (retry, next_token) = app
        .world()
        .resource::<ProfileEditorOptionsState>()
        .filter_request
        .unwrap();
    assert_ne!(retry, request_id);
    assert!(next_token > token);
    let command = sink.submitted().last().unwrap().clone();
    let UiCommand::SaveSubscriptionFilter { source, .. } = &command else {
        panic!("filter intent");
    };
    let applied = SubscriptionFilterApplied {
        source: source.clone(),
        report: FilterReport {
            total_input: 3,
            passed: 1,
            excluded_by_whitelist: 2,
            ..Default::default()
        },
    };
    app.world_mut().trigger(CommandExecutedEvent {
        request_id: retry,
        command,
        result: Ok(CommandOutput::SubscriptionFilterApplied(applied.clone())),
    });
    app.update();
    let editor = &app.world().resource::<ProfileEditorOptionsState>().filter;
    assert!(editor.applied);
    assert!(editor.pending.is_none());
    assert!(editor.failure.is_none());
    assert_eq!(editor.report, Some(applied.report));
}

#[test]
fn native_document_advanced_filter_validates_keeps_user_input_and_discards_without_writing() {
    let (mut app, sink, _) = mount();
    let field = input(&mut app, FilterField::Advanced);
    click_entity(&mut app, field);
    type_text(&mut app, "{drop-private-ips: true}");
    let save = marker_entity::<EditorFilterSaveButton>(&mut app);
    let before = sink.submitted().len();
    click_entity(&mut app, save);
    assert_eq!(sink.submitted().len(), before);
    let editor = &app.world().resource::<ProfileEditorOptionsState>().filter;
    assert_eq!(
        editor.failure.as_ref().unwrap().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        editor.draft.advanced_policy.as_deref(),
        Some("{drop-private-ips: true}")
    );
    click_entity(&mut app, field);
    replace_text(&mut app, "{drop-private-ip: true, sort-by: name-desc}");
    click_entity(&mut app, save);
    let command = sink.submitted().last().unwrap().clone();
    let UiCommand::SaveSubscriptionFilter { filter, source } = &command else {
        panic!("typed advanced filter");
    };
    assert_eq!(
        filter.advanced_policy.as_deref(),
        Some("{drop-private-ip: true, sort-by: name-desc}")
    );
    let (request_id, _) = app
        .world()
        .resource::<ProfileEditorOptionsState>()
        .filter_request
        .unwrap();
    app.world_mut().trigger(CommandExecutedEvent {
        request_id,
        command: command.clone(),
        result: Ok(CommandOutput::SubscriptionFilterApplied(
            SubscriptionFilterApplied {
                source: source.clone(),
                report: Default::default(),
            },
        )),
    });
    app.update();
    click_entity(&mut app, field);
    replace_text(&mut app, "{remove-emojis: true}");
    let discard = marker_entity::<EditorFilterDiscard>(&mut app);
    let before = sink.submitted().len();
    click_entity(&mut app, discard);
    assert_eq!(
        sink.submitted().len(),
        before,
        "discard never submits policy changes"
    );
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        "{drop-private-ip: true, sort-by: name-desc}"
    );
}
