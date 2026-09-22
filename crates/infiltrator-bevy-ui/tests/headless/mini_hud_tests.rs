//! Headless tests for the Bevy Mini HUD overlay (DUAL-15-03/04): the shared
//! read model, the mounted scene, the toggle event and the pin persistence
//! path through the shared settings command.

use std::sync::Arc;

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::query::With;
use bevy::scene::ScenePlugin;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_ui::app::{ShellPlugin, SidebarToggleProjection};
use infiltrator_bevy_ui::command::{CommandSinkHandle, DemoCommandSink, UiCommand};
use infiltrator_bevy_ui::mini_hud::{
    MiniHudMode, MiniHudModel, MiniHudRoot, MiniHudSystemProxyToggle, MiniHudTunToggle,
    SetMiniHudPinned, ToggleMiniHud,
};
use infiltrator_bevy_ui::route::PagesPlugin;
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleSnapshot};

fn mounted_app() -> (App, Arc<DemoCommandSink>) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::new(
        infiltrator_contract::theme::ThemePreference::Fixed(
            infiltrator_contract::theme::ThemeSkin::Dark,
        ),
    ));
    // The demo surface is the deterministic projection source (same fixture
    // the shell uses for screenshots).
    app.add_plugins(PagesPlugin::demo());
    let sink = Arc::new(DemoCommandSink::accepting());
    app.insert_resource(CommandSinkHandle(sink.clone()));
    app.update();
    (app, sink)
}

fn hud_roots(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query::<&MiniHudRoot>().iter(world).count()
}

fn hud_texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world
        .query::<&Text>()
        .iter(world)
        .map(|text| text.0.clone())
        .collect()
}

#[test]
fn the_toggle_event_mounts_and_unmounts_the_overlay() {
    let (mut app, _) = mounted_app();
    assert_eq!(hud_roots(&mut app), 0, "the HUD starts hidden");

    app.world_mut().commands().trigger(ToggleMiniHud);
    app.update();
    app.update();
    assert!(app.world().resource::<MiniHudMode>().0);
    assert_eq!(hud_roots(&mut app), 1, "one mounted HUD root");

    app.world_mut().commands().trigger(ToggleMiniHud);
    app.update();
    app.update();
    assert!(!app.world().resource::<MiniHudMode>().0);
    assert_eq!(hud_roots(&mut app), 0, "toggling again unmounts it");
}

#[test]
fn the_read_model_comes_from_the_live_projections() {
    let (mut app, _) = mounted_app();
    let model = app.world().resource::<MiniHudModel>().0.clone();
    // The demo fixture reports real non-zero rates, a real mode and a real
    // exit node — no baked literals in the scene.
    assert!(model.down_bytes_per_sec > 0 || model.up_bytes_per_sec > 0);
    assert!(!model.mode_zh.is_empty());
    assert!(!model.exit_node.is_empty());

    // A pending system toggle is visibly non-actionable through the shared
    // state letters.
    {
        let mut toggles = app
            .world_mut()
            .resource_mut::<infiltrator_bevy_ui::app::SidebarToggleProjection>();
        toggles.0 = toggles.0.clone().with_pending(SystemToggle::Tun, true);
    }
    app.update();
    let model = app.world().resource::<MiniHudModel>().0.clone();
    assert_eq!(model.next_value(SystemToggle::Tun), None);
    assert!(model.status_line().contains('…'));
}

#[test]
fn the_mounted_scene_renders_the_shared_read_model() {
    let (mut app, _) = mounted_app();
    app.world_mut().commands().trigger(ToggleMiniHud);
    app.update();
    app.update();

    let model = app.world().resource::<MiniHudModel>().0.clone();
    let texts = hud_texts(&mut app).join(" | ");
    assert!(
        texts.contains(&model.status_line()),
        "the shared toggle-state line renders verbatim: {texts}"
    );
    assert!(
        texts.contains(&model.exit_node),
        "the real exit node renders: {texts}"
    );
    assert!(
        texts.contains(&model.mode_zh),
        "the real proxy mode renders: {texts}"
    );
}

#[test]
fn the_pin_request_persists_through_the_shared_settings_command() {
    let (mut app, sink) = mounted_app();
    app.world_mut().commands().trigger(SetMiniHudPinned(true));
    app.update();
    assert!(
        sink.submitted().iter().any(|command| matches!(
            command,
            UiCommand::UpdateSetting { key, value }
                if key == "mini_hud.pinned" && value == "true"
        )),
        "the pin write goes through the validated shared settings path: {:?}",
        sink.submitted()
    );
}

fn mount_hud_with_toggles(app: &mut App) {
    app.insert_resource(SidebarToggleProjection(SystemToggleSnapshot::from_legacy(
        true,
        Some(false),
        0,
    )));
    app.world_mut().commands().trigger(ToggleMiniHud);
    app.update();
    app.update();
}

fn first_entity<M: bevy::ecs::component::Component>(app: &mut App) -> bevy::ecs::entity::Entity {
    let world = app.world_mut();
    let mut query = world.query_filtered::<bevy::ecs::entity::Entity, With<M>>();
    query
        .iter(world)
        .next()
        .expect("the mounted HUD owns this marker")
}

#[test]
fn the_hud_quick_switches_dispatch_the_shared_toggle_commands() {
    let (mut app, sink) = mounted_app();
    mount_hud_with_toggles(&mut app);

    let proxy = first_entity::<MiniHudSystemProxyToggle>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: proxy });
    app.update();
    assert!(
        sink.submitted()
            .iter()
            .any(|command| matches!(command, UiCommand::SetSystemProxy { enabled: false })),
        "the HUD proxy switch resolves the desired state through the shared snapshot: {:?}",
        sink.submitted()
    );

    let tun = first_entity::<MiniHudTunToggle>(&mut app);
    app.world_mut().commands().trigger(Activate { entity: tun });
    app.update();
    assert!(
        sink.submitted()
            .iter()
            .any(|command| matches!(command, UiCommand::ToggleTun { enabled: true })),
        "the HUD TUN switch uses the same command as the sidebar: {:?}",
        sink.submitted()
    );

    // Both presses went through the shared pending projection, so the read
    // model reports them as in-flight.
    let toggles = app.world().resource::<SidebarToggleProjection>();
    assert!(toggles.0.state(SystemToggle::SystemProxy).is_pending());
    assert!(toggles.0.state(SystemToggle::Tun).is_pending());
    let model = app.world().resource::<MiniHudModel>().0.clone();
    assert_eq!(model.next_value(SystemToggle::SystemProxy), None);
}

#[test]
fn a_pending_hud_quick_switch_dispatches_nothing() {
    let (mut app, sink) = mounted_app();
    mount_hud_with_toggles(&mut app);
    {
        let mut toggles = app.world_mut().resource_mut::<SidebarToggleProjection>();
        toggles.0 = toggles.0.clone().with_pending(SystemToggle::Tun, true);
    }
    app.update();

    let tun = first_entity::<MiniHudTunToggle>(&mut app);
    app.world_mut().commands().trigger(Activate { entity: tun });
    app.update();
    assert!(
        !sink
            .submitted()
            .iter()
            .any(|command| matches!(command, UiCommand::ToggleTun { .. })),
        "a toggle already in flight cannot be pressed again: {:?}",
        sink.submitted()
    );
}
