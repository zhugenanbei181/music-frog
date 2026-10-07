//! test-intent: behavior
//! Actual SDK pointer-scroll observers operate against explicit layout fixtures.
//! These fixtures exercise input handling; delivery pixels use real layout only.
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::touch::TouchPhase;
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerScroll};
use bevy::picking::pointer::{Location, PointerId};
use bevy::ui::{ComputedNode, ScrollPosition};
use bevy::ui_widgets::{Activate, ScrollArea};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink};
use infiltrator_bevy_ui::command_palette::{CommandPaletteScrollArea, OpenCommandPalette};
use infiltrator_bevy_ui::pages::business_panel::{
    BusinessPanelScrollArea, OpenBusinessPanel, PanelKind,
};
use infiltrator_bevy_ui::pages::connections::ConnInspectButton;
use infiltrator_bevy_ui::pages::connections_drawer::ConnectionDrawerRoot;
use infiltrator_bevy_ui::pages::overview_speedtest::{
    OverviewSpeedtestDetailButton, SpeedtestDetailScrollArea,
};
use infiltrator_bevy_ui::pages::proxies::NodeDetailButton;
use infiltrator_bevy_ui::pages::proxy_inspection::{
    ProxyInspectionScrollArea, ProxyInspectionState,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use std::sync::Arc;

fn setup(route: Route) -> (App, Arc<DemoCommandSink>) {
    let mut app = App::new();
    headless_plugins(&mut app);
    let sink = Arc::new(DemoCommandSink::accepting());
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::new(sink.clone()));
    app.update();
    app.world_mut().commands().trigger(RouteChanged(route));
    app.update();
    (app, sink)
}

fn wheel(app: &mut App, entity: Entity, y: f32) {
    app.world_mut().commands().trigger(PointerScroll {
        entity,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::None {
                    width: 720,
                    height: 480,
                },
                position: Vec2::ZERO,
            },
        ),
        unit: MouseScrollUnit::Pixel,
        x: 0.0,
        y,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        phase: TouchPhase::Moved,
    });
    app.world_mut().flush();
}

fn verify_scroll<T: Component>(app: &mut App) {
    let entity = app
        .world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .unwrap();
    assert!(app.world().get::<ScrollArea>(entity).is_some());
    app.world_mut().entity_mut(entity).insert(ComputedNode {
        size: Vec2::new(400.0, 180.0),
        content_size: Vec2::new(400.0, 700.0),
        inverse_scale_factor: 1.0,
        ..Default::default()
    });
    wheel(app, entity, -250.0);
    assert_eq!(app.world().get::<ScrollPosition>(entity).unwrap().y, 250.0);
    wheel(app, entity, -10_000.0);
    assert_eq!(app.world().get::<ScrollPosition>(entity).unwrap().y, 520.0);
    wheel(app, entity, 10_000.0);
    assert_eq!(app.world().get::<ScrollPosition>(entity).unwrap().y, 0.0);
}

#[test]
fn connection_drawer_scrolls_to_its_lower_actions_without_commands() {
    let (mut app, sink) = setup(Route::Connections);
    let entity = app
        .world_mut()
        .query::<(Entity, &ConnInspectButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == 0)
        .unwrap()
        .0;
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
    verify_scroll::<ConnectionDrawerRoot>(&mut app);
    assert!(sink.submitted().is_empty());
}

#[test]
fn speedtest_detail_scrolls_without_starting_a_probe() {
    let (mut app, sink) = setup(Route::Overview);
    let entity = app
        .world_mut()
        .query_filtered::<Entity, With<OverviewSpeedtestDetailButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
    verify_scroll::<SpeedtestDetailScrollArea>(&mut app);
    assert!(sink.submitted().is_empty());
}

#[test]
fn command_palette_scrolls_to_its_remaining_rows_without_executing() {
    let (mut app, sink) = setup(Route::Overview);
    app.world_mut().commands().trigger(OpenCommandPalette);
    app.update();
    verify_scroll::<CommandPaletteScrollArea>(&mut app);
    assert!(sink.submitted().is_empty());
}

#[test]
fn proxy_inspection_scrolls_to_metadata_and_history_without_selecting_or_probing() {
    let (mut app, sink) = setup(Route::Proxies);
    let (entity, name) = app
        .world_mut()
        .query::<(Entity, &NodeDetailButton)>()
        .iter(app.world())
        .next()
        .map(|(entity, button)| (entity, button.node_name.clone()))
        .unwrap();
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
    assert_eq!(
        app.world()
            .resource::<ProxyInspectionState>()
            .selected
            .as_deref(),
        Some(name.as_str())
    );
    verify_scroll::<ProxyInspectionScrollArea>(&mut app);
    assert!(sink.submitted().is_empty());
}

#[test]
fn custom_node_form_scrolls_to_all_protocol_fields_without_saving() {
    let (mut app, sink) = setup(Route::Proxies);
    let entity = app
        .world_mut()
        .query::<(Entity, &OpenBusinessPanel)>()
        .iter(app.world())
        .find(|(_, open)| open.0 == PanelKind::CustomNode)
        .map(|(entity, _)| entity)
        .unwrap();
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
    verify_scroll::<BusinessPanelScrollArea>(&mut app);
    assert!(sink.submitted().is_empty());
}
