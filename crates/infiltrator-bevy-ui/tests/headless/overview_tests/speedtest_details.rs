//! test-intent: behavior
use super::*;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::input::{ButtonInput, keyboard::KeyCode};
use infiltrator_application::speedtest_detail_projection::{listing, project_details};
use infiltrator_bevy_ui::pages::overview_speedtest::{
    OverviewSpeedtestDetailBodyText, OverviewSpeedtestDetailButton,
};
use infiltrator_bevy_widgets::adaptive_modal::{AdaptiveModalRoot, ModalCloseButton, ModalState};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::capability::Availability;
use infiltrator_contract::speedtest::SpeedtestSnapshot;
use infiltrator_shared::locales::{Lang, Localizer};

fn activate<T: Component>(app: &mut App) {
    let entity = app
        .world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .unwrap();
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
    app.update();
}

fn detail_text(app: &mut App) -> String {
    app.world_mut()
        .query_filtered::<&Text, With<OverviewSpeedtestDetailBodyText>>()
        .single(app.world())
        .unwrap()
        .0
        .clone()
}

#[test]
fn speedtest_details_render_shared_fold_and_close_escape_and_navigation() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = mounted_default();
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    let mut projection = DemoOverviewSource::running().current();
    projection.speedtest = SpeedtestSnapshot::demo_fixture();
    let locale = app.world().resource::<UiLocale>().code().to_string();
    let expected = listing(&project_details(&projection.speedtest), &|key| {
        Lang(&locale).tr(key).into_owned()
    });
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();
    activate::<OverviewSpeedtestDetailButton>(&mut app);
    assert!(app.world().resource::<ModalState>().is_open);
    assert_eq!(detail_text(&mut app), expected);
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<AdaptiveModalRoot>>()
        .single(app.world())
        .unwrap();
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::Flex
    );
    activate::<ModalCloseButton>(&mut app);
    assert!(!app.world().resource::<ModalState>().is_open);
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    activate::<OverviewSpeedtestDetailButton>(&mut app);
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::Escape);
    app.world_mut().insert_resource(keys);
    app.update();
    assert!(!app.world().resource::<ModalState>().is_open);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    activate::<OverviewSpeedtestDetailButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Profiles));
    app.update();
    app.update();
    assert!(!app.world().resource::<ModalState>().is_open);
    assert_eq!(sink.submitted(), Vec::<UiCommand>::new());
}

#[test]
fn speedtest_details_render_typed_unsupported_without_inventing_results() {
    let mut app = mounted_default();
    let mut projection = DemoOverviewSource::running().current();
    projection.speedtest = SpeedtestSnapshot::default();
    projection.speedtest.availability = Some(Availability::Unsupported {
        reason: "missing host engine".into(),
    });
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();
    activate::<OverviewSpeedtestDetailButton>(&mut app);
    assert!(app.world().resource::<ModalState>().is_open);
    let body = detail_text(&mut app);
    assert!(body.contains("missing host engine"));
    assert!(!body.contains("Mbps"));
    activate::<ModalCloseButton>(&mut app);
    assert!(!app.world().resource::<ModalState>().is_open);
}
