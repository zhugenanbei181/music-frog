//! test-intent: behavior
use super::*;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ui_widgets::{ModalDialog, ModalDialogBarrier, RequestClose};
use infiltrator_bevy_ui::pages::overview_speedtest::{
    OverviewSpeedtestDetailBodyText, OverviewSpeedtestDetailButton,
};
use infiltrator_bevy_widgets::adaptive_modal::{
    AdaptiveModalRoot, ModalCard, ModalCloseButton, ModalState,
};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::modal::ModalStage;
use infiltrator_contract::speedtest::SpeedtestSnapshot;
use infiltrator_presentation::speedtest_detail_projection::{listing, project_details};
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::error::Failure;

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

/// The adaptive modal's typed stage: the one truth about the inspection.
fn stage(app: &mut App) -> ModalStage {
    app.world().resource::<ModalState>().stage
}

/// The official barrier of the adaptive modal: the overlay root.
fn barrier(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, (With<ModalDialogBarrier>, With<AdaptiveModalRoot>)>()
        .single(app.world())
        .expect("the official barrier is mounted")
}

/// The official dialog of the adaptive modal: the card.
fn dialog(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, (With<ModalDialog>, With<ModalCard>)>()
        .single(app.world())
        .expect("the official dialog is mounted")
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
    assert_eq!(stage(&mut app), ModalStage::Open);
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
    // The inspection rides the official dialog layer: the overlay root is the
    // official barrier and the card is the official dialog.
    let barrier = barrier(&mut app);
    let dialog = dialog(&mut app);
    assert!(
        app.world().get::<AdaptiveModalRoot>(barrier).is_some(),
        "the barrier is the adaptive modal's root"
    );
    assert!(
        app.world().get::<ModalCard>(dialog).is_some(),
        "the dialog is the adaptive modal's card"
    );
    // The close button asks the official layer to close: the request travels
    // as the official `RequestClose` and the typed stage reports the
    // dismissal.
    activate::<ModalCloseButton>(&mut app);
    assert_eq!(stage(&mut app), ModalStage::Dismissed);
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    // The official close request — the barrier click-outside and the Escape
    // interception both travel as it — dismisses the inspection.
    activate::<OverviewSpeedtestDetailButton>(&mut app);
    assert_eq!(stage(&mut app), ModalStage::Open);
    app.world_mut()
        .commands()
        .trigger(RequestClose { source: barrier });
    app.update();
    app.update();
    assert_eq!(stage(&mut app), ModalStage::Dismissed);
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    // A different page closes the inspection for good.
    activate::<OverviewSpeedtestDetailButton>(&mut app);
    assert_eq!(stage(&mut app), ModalStage::Open);
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Profiles));
    app.update();
    app.update();
    assert_eq!(stage(&mut app), ModalStage::Hidden);
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    assert_eq!(sink.submitted(), Vec::<UiCommand>::new());
}

#[test]
fn speedtest_details_render_typed_unsupported_without_inventing_results() {
    let mut app = mounted_default();
    let mut projection = DemoOverviewSource::running().current();
    projection.speedtest =
        SpeedtestSnapshot::unavailable(Failure::unsupported("missing Core speedtest owner"));
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();
    activate::<OverviewSpeedtestDetailButton>(&mut app);
    assert_eq!(stage(&mut app), ModalStage::Open);
    let body = detail_text(&mut app);
    assert!(body.contains("missing Core speedtest owner"));
    assert!(!body.contains("Mbps"));
    activate::<ModalCloseButton>(&mut app);
    assert_eq!(stage(&mut app), ModalStage::Dismissed);
}
