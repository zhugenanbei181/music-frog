//! test-intent: behavior
use super::*;
use crate::native_input::{click_entity, press, type_text};
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::profiles_filter_dedup::ReloadFilterDedup;
use infiltrator_bevy_ui::pages::profiles_filter_form::{
    FilterFormState, FilterFormStatus, FilterText,
};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::subscription_filter_form::FilterField;
use infiltrator_contract::surface_snapshot::PageStatus;

fn input(app: &mut App, kind: FilterField) -> Entity {
    app.world_mut()
        .query::<(Entity, &FilterText)>()
        .iter(app.world())
        .find(|(_, field)| field.0 == kind)
        .unwrap()
        .0
}
#[test]
fn native_all_filter_fields_survive_read_replay_and_source_change_until_explicit_discard() {
    for (kind, value) in [
        (FilterField::Include, "user"),
        (FilterField::Exclude, "user"),
        (FilterField::Protocols, "ss"),
        (FilterField::Renames, "left=>right"),
        (FilterField::Advanced, "{drop-private-ip: true}"),
    ] {
        let sink = Arc::new(DemoCommandSink::accepting());
        let mut app = setup_matrix_a_app(sink.clone());
        app.add_plugins(ButtonPlugin);
        navigate_to(&mut app, Route::Profiles);
        let mut projection = subscription_fetch_projection();
        app.world_mut()
            .commands()
            .trigger(ProfilesProjectionUpdated(projection.clone()));
        app.update();
        let entity = input(&mut app, kind);
        let original = app
            .world()
            .get::<TextField>(entity)
            .unwrap()
            .0
            .text()
            .to_owned();
        press(&mut app, entity);
        assert!(
            !app.world()
                .get::<TextField>(entity)
                .unwrap()
                .0
                .is_disabled(),
            "{kind:?} is enabled: {:?}",
            app.world().resource::<FilterFormState>().editor
        );
        assert!(
            app.world().get::<TextFieldFocused>(entity).unwrap().0,
            "native pointer focuses {kind:?}"
        );
        type_text(&mut app, value);
        let expected = app
            .world()
            .get::<TextField>(entity)
            .unwrap()
            .0
            .text()
            .to_owned();
        assert_ne!(expected, original, "native input changes {kind:?}");
        assert!(
            expected.contains(value),
            "native insertion in {kind:?}: {expected:?}"
        );
        app.world_mut()
            .get_mut::<TextField>(entity)
            .unwrap()
            .0
            .set_preedit("zhong");
        let editing = app.world().get::<TextField>(entity).unwrap().0.clone();
        app.world_mut()
            .commands()
            .trigger(ProfilesProjectionUpdated(projection.clone()));
        app.update();
        assert_eq!(app.world().get::<TextField>(entity).unwrap().0, editing);
        assert!(app.world().get::<TextFieldFocused>(entity).unwrap().0);
        let profile = projection
            .profiles
            .iter_mut()
            .find(|p| p.is_active)
            .unwrap();
        kind.set(
            &mut profile.filter,
            match kind {
                FilterField::Renames => "remote=>next",
                FilterField::Advanced => "{remove-emojis: true}",
                _ => "remote",
            }
            .into(),
        );
        profile.filter_source =
            observation(&profile.id, FIXTURE_DOCUMENT, profile.filter.clone()).map(|o| o.source);
        app.world_mut()
            .commands()
            .trigger(ProfilesProjectionUpdated(projection));
        app.update();
        assert_eq!(
            app.world().get::<TextField>(entity).unwrap().0.text(),
            expected
        );
        assert_eq!(
            app.world().get::<TextField>(entity).unwrap().0.preedit(),
            "zhong"
        );
        assert!(
            app.world()
                .get::<TextField>(entity)
                .unwrap()
                .0
                .is_disabled()
        );
        let save = marker_entity::<SaveSubscriptionFilterButton>(&mut app);
        assert!(app.world().get::<InteractionDisabled>(save).is_some());
        click_entity(&mut app, save);
        assert!(sink.submitted().is_empty());
        let discard = marker_entity::<ReloadFilterDedup>(&mut app);
        click_entity(&mut app, discard);
        assert_eq!(
            app.world().get::<TextField>(entity).unwrap().0.text(),
            match kind {
                FilterField::Renames => "remote=>next",
                FilterField::Advanced => "{remove-emojis: true}",
                _ => "remote",
            }
        );
        assert_eq!(
            app.world().get::<TextField>(entity).unwrap().0.preedit(),
            ""
        );
        assert!(sink.submitted().is_empty(), "discard never writes");
    }
}
#[test]
fn native_filter_pending_failure_retry_and_wrong_receipt_stay_in_the_same_form() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.add_plugins(ButtonPlugin);
    navigate_to(&mut app, Route::Profiles);
    let projection = subscription_fetch_projection();
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection.clone()));
    app.update();
    let field = input(&mut app, FilterField::Include);
    press(&mut app, field);
    type_text(&mut app, "draft");
    let expected = app
        .world()
        .get::<TextField>(field)
        .unwrap()
        .0
        .text()
        .to_owned();
    let save = marker_entity::<SaveSubscriptionFilterButton>(&mut app);
    let discard = marker_entity::<ReloadFilterDedup>(&mut app);
    click_entity(&mut app, save);
    let request_id = app.world().resource::<FilterFormState>().request.unwrap().0;
    let command = sink.submitted()[0].clone();
    assert!(app.world().get::<InteractionDisabled>(save).is_some());
    assert!(app.world().get::<InteractionDisabled>(discard).is_some());
    assert!(app.world().get::<TextField>(field).unwrap().0.is_disabled());
    app.world_mut().trigger(CommandExecutedEvent {
        command: UiCommand::UpdateAllSubscriptions,
        request_id,
        result: Ok(CommandOutput::Unit),
    });
    app.update();
    assert!(
        app.world()
            .resource::<FilterFormState>()
            .editor
            .pending
            .is_some()
    );
    let failure = Failure::new(ErrorCode::Storage, "permission denied", true);
    app.world_mut().trigger(CommandExecutedEvent {
        command: command.clone(),
        request_id,
        result: Err(failure.clone()),
    });
    app.update();
    assert_eq!(
        app.world().resource::<FilterFormState>().editor.failure,
        Some(failure)
    );
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        expected
    );
    assert!(!app.world().get::<TextField>(field).unwrap().0.is_disabled());
    let status = marker_entity::<FilterFormStatus>(&mut app);
    assert!(
        app.world()
            .get::<Text>(status)
            .unwrap()
            .0
            .contains("permission denied")
    );
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();
    assert!(
        app.world()
            .get::<Text>(status)
            .unwrap()
            .0
            .contains("permission denied")
    );
    click_entity(&mut app, save);
    let second = app.world().resource::<FilterFormState>().request.unwrap().0;
    assert_ne!(second, request_id);
    app.world_mut().trigger(CommandExecutedEvent {
        command,
        request_id,
        result: Ok(CommandOutput::Unit),
    });
    app.update();
    assert_eq!(
        app.world().resource::<FilterFormState>().request.unwrap().0,
        second
    );
    assert!(
        app.world()
            .resource::<FilterFormState>()
            .editor
            .pending
            .is_some()
    );
}

#[test]
fn native_filter_read_failure_and_route_remount_preserve_owner_draft() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.add_plugins(ButtonPlugin);
    navigate_to(&mut app, Route::Profiles);
    let projection = subscription_fetch_projection();
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection.clone()));
    app.update();
    let field = input(&mut app, FilterField::Include);
    press(&mut app, field);
    type_text(&mut app, "unsent");
    let expected = app
        .world()
        .get::<TextField>(field)
        .unwrap()
        .0
        .text()
        .to_owned();
    let mut failed = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    failed.revision += 1;
    failed.pages.profiles.status = PageStatus::Failed {
        failure: Failure::new(ErrorCode::Storage, "read denied", true),
    };
    app.world_mut().trigger(SurfaceSnapshotUpdated(failed));
    app.update();
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        expected
    );
    assert!(app.world().get::<TextField>(field).unwrap().0.is_disabled());
    let save = marker_entity::<SaveSubscriptionFilterButton>(&mut app);
    click_entity(&mut app, save);
    assert!(sink.submitted().is_empty());
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection.clone()));
    app.update();
    assert!(!app.world().get::<TextField>(field).unwrap().0.is_disabled());
    navigate_to(&mut app, Route::Overview);
    assert!(
        app.world().get_entity(field).is_err(),
        "old native field retired"
    );
    navigate_to(&mut app, Route::Profiles);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();
    let remounted = input(&mut app, FilterField::Include);
    assert_ne!(remounted, field);
    assert_eq!(
        app.world().get::<TextField>(remounted).unwrap().0.text(),
        expected
    );
    assert_eq!(
        app.world()
            .resource::<FilterFormState>()
            .editor
            .draft
            .include,
        expected
    );
    assert!(sink.submitted().is_empty());
}
