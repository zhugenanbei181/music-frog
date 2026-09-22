//! Headless tests for the Bevy frameless window chrome (DUAL-15-13): the
//! shared chrome shape is applied to the window, the drag bar starts a real
//! OS drag move, and the three controls request minimize / maximize / close.

use bevy::MinimalPlugins;
use bevy::app::{App, AppExit};
use bevy::asset::AssetPlugin;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Click, Pointer, Press};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::scene::ScenePlugin;
use bevy::window::{PrimaryWindow, Window, WindowRef};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::chrome::{
    ChromeCloseButton, ChromeDragBar, ChromeMaximizeButton, ChromeMaximizeLatch,
    ChromeMinimizeButton, WindowChromePlugin, WindowChromeReport, chrome_shape, support,
};

fn chrome_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(WindowChromePlugin);
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    app.update();
    (app, window)
}

fn press_on(app: &mut App, window: Entity, entity: Entity, count: u8) {
    let location = Location {
        target: NormalizedRenderTarget::Window(
            WindowRef::Entity(window)
                .normalize(Some(window))
                .expect("a mounted window normalizes"),
        ),
        position: bevy::math::Vec2::ZERO,
    };
    app.world_mut().trigger(Pointer::new(
        PointerId::Mouse,
        location,
        Press {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
            count,
        },
        entity,
    ));
}

fn double_click_on(app: &mut App, window: Entity, entity: Entity) {
    let location = Location {
        target: NormalizedRenderTarget::Window(
            WindowRef::Entity(window)
                .normalize(Some(window))
                .expect("a mounted window normalizes"),
        ),
        position: bevy::math::Vec2::ZERO,
    };
    app.world_mut().trigger(Pointer::new(
        PointerId::Mouse,
        location,
        Click {
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
            duration: std::time::Duration::from_millis(120),
            count: 2,
        },
        entity,
    ));
}

#[test]
fn the_host_applies_the_shared_frameless_shape_and_reports_it() {
    let (app, window) = chrome_app();
    let chrome = chrome_shape();
    assert!(!chrome.os_decorations());
    assert!(chrome.needs_custom_controls());

    {
        let entity = app.world().entity(window);
        let window = entity.get::<Window>().expect("window mounted");
        assert_eq!(
            window.decorations,
            chrome.os_decorations(),
            "the real window follows the shared chrome contract"
        );
        assert!(!window.decorations);
    }

    let report = *app.world().resource::<WindowChromeReport>();
    assert_eq!(report.0, support());
    assert!(report.0.is_hosted());
    assert!(report.0.drag(), "the OS drag path is wired");
    assert!(report.0.maximize());
    assert_eq!(report.0.unsupported_reason(), None);
}

#[test]
fn a_press_on_the_chrome_bar_starts_the_os_drag_move() {
    let (mut app, window) = chrome_app();
    let bar = app.world_mut().spawn(ChromeDragBar).id();

    press_on(&mut app, window, bar, 1);
    app.update();

    let mut entity = app.world_mut().entity_mut(window);
    let mut window_component = entity.get_mut::<Window>().expect("window mounted");
    assert!(
        window_component.internal.take_move_request(),
        "the press must reach winit's drag_window request"
    );
}

#[test]
fn a_double_click_toggles_the_maximize_request_and_the_latch() {
    let (mut app, window) = chrome_app();
    let bar = app.world_mut().spawn(ChromeDragBar).id();
    assert!(!app.world().resource::<ChromeMaximizeLatch>().0);

    double_click_on(&mut app, window, bar);
    app.update();
    {
        let mut entity = app.world_mut().entity_mut(window);
        let mut window_component = entity.get_mut::<Window>().expect("window mounted");
        assert_eq!(
            window_component.internal.take_maximize_request(),
            Some(true)
        );
    }
    assert!(app.world().resource::<ChromeMaximizeLatch>().0);

    double_click_on(&mut app, window, bar);
    app.update();
    {
        let mut entity = app.world_mut().entity_mut(window);
        let mut window_component = entity.get_mut::<Window>().expect("window mounted");
        assert_eq!(
            window_component.internal.take_maximize_request(),
            Some(false),
            "the second double click restores"
        );
    }
    assert!(!app.world().resource::<ChromeMaximizeLatch>().0);
}

#[test]
fn the_three_chrome_controls_request_minimize_maximize_and_exit() {
    use bevy::ui_widgets::Activate;

    let (mut app, window) = chrome_app();
    let minimize = app.world_mut().spawn(ChromeMinimizeButton).id();
    let maximize = app.world_mut().spawn(ChromeMaximizeButton).id();
    let close = app.world_mut().spawn(ChromeCloseButton).id();

    app.world_mut()
        .commands()
        .trigger(Activate { entity: minimize });
    app.update();
    {
        let mut entity = app.world_mut().entity_mut(window);
        let mut window_component = entity.get_mut::<Window>().expect("window mounted");
        assert_eq!(
            window_component.internal.take_minimize_request(),
            Some(true)
        );
    }

    app.world_mut()
        .commands()
        .trigger(Activate { entity: maximize });
    app.update();
    {
        let mut entity = app.world_mut().entity_mut(window);
        let mut window_component = entity.get_mut::<Window>().expect("window mounted");
        assert_eq!(
            window_component.internal.take_maximize_request(),
            Some(true)
        );
    }

    app.world_mut()
        .commands()
        .trigger(Activate { entity: close });
    app.update();
    let exits: Vec<AppExit> = app
        .world_mut()
        .resource_mut::<bevy::ecs::message::Messages<AppExit>>()
        .drain()
        .collect();
    assert_eq!(exits, vec![AppExit::Success]);
}

#[test]
fn the_mounted_shell_carries_the_chrome_bar_and_its_controls() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::new(
        infiltrator_contract::theme::ThemePreference::Fixed(
            infiltrator_contract::theme::ThemeSkin::Dark,
        ),
    ));
    app.update();

    let world = app.world_mut();
    let bars = world.query::<&ChromeDragBar>().iter(world).count();
    assert_eq!(bars, 1, "exactly one drag bar is mounted");
    let world = app.world_mut();
    let buttons = world
        .query_filtered::<Entity, bevy::ecs::query::Or<(
            bevy::ecs::query::With<ChromeMinimizeButton>,
            bevy::ecs::query::With<ChromeMaximizeButton>,
            bevy::ecs::query::With<ChromeCloseButton>,
        )>>()
        .iter(world)
        .count();
    assert_eq!(buttons, 3, "minimize/maximize/close are all mounted");
}
