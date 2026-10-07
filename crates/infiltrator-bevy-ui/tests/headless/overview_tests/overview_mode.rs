//! Behavior cases for overview mode.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_ui::app::ModeActionState;
use infiltrator_bevy_ui::pages::overview_cards::{OverviewModeSegmentPill, ProxyModeSegmentCard};
use infiltrator_contract::command::ProxyMode;

#[test]
fn overview_mode_segment_uses_correlated_mode_state_without_a_second_command_sink() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(DemoOverviewSource::running()));
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.update();

    let button = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &OverviewModeSegmentPill)>();
        buttons
            .iter(world)
            .find(|(_, pill)| pill.0 == ProxyMode::Global)
            .expect("global mode pill mounted")
            .0
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<ModeActionState>().0.observed.current,
        Some(ProxyMode::Global)
    );
    assert!(
        app.world()
            .resource::<ModeActionState>()
            .0
            .pending
            .is_none()
    );
    assert!(
        !sink
            .submitted()
            .contains(&UiCommand::SetProxyMode(ProxyMode::Global))
    );
}

#[test]
fn overview_mode_segment_card_is_mounted_with_four_pills() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let mut card_query = world.query::<&ProxyModeSegmentCard>();
    assert!(
        card_query.iter(world).next().is_some(),
        "ProxyModeSegmentCard must be mounted"
    );

    let mut pills_query = world.query::<&OverviewModeSegmentPill>();
    let modes: Vec<ProxyMode> = pills_query.iter(world).map(|p| p.0).collect();
    assert_eq!(modes.len(), 4);
    assert!(modes.contains(&ProxyMode::Rule));
    assert!(modes.contains(&ProxyMode::Global));
    assert!(modes.contains(&ProxyMode::Direct));
    assert!(modes.contains(&ProxyMode::Script));
}
