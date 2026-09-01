//! Headless tests for the popover: the pure placement core (prefer the
//! requested side, flip when out of room, clamp into the viewport across the
//! four quadrants), the overlay scene's stamped geometry, and token reskin
//! on a theme flip — entity ids never change.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, Scene, ScenePlugin, bsn};
use bevy::ui::BackgroundColor;
use bevy::ui::prelude::{Node, Val, percent};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::popover::{
    ANCHOR_GAP_PX, AnchorHint, PopoverPanel, PopoverScrim, Rect, Side, placement, popover_scene,
};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{LightDark, Theme};

const VIEWPORT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 400.0,
    h: 300.0,
};
const PANEL: (f32, f32) = (120.0, 60.0);
const GAP: f32 = 8.0;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

fn hint(anchor: Rect, side: Side) -> AnchorHint {
    AnchorHint {
        anchor,
        viewport: VIEWPORT,
        side,
        panel: PANEL,
    }
}

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

/// The popover's content is caller-owned; any composed scene does.
fn content_scene() -> Box<dyn Scene> {
    Box::new(bsn! {
        Node {
            width: percent(100),
        }
    })
}

#[test]
fn below_fits_on_both_quadrants_horizontally() {
    let anchor = rect(10.0, 10.0, 80.0, 20.0);
    let placed = placement(hint(anchor, Side::Below), GAP);
    assert_eq!(
        placed.y,
        anchor.bottom() + GAP,
        "below the anchor with the gap"
    );
    assert_eq!(placed.x, 10.0, "left edges align on the left side");

    // Anchor near the right edge but low enough for Above to hold the
    // panel: above wins and the panel clamps into the viewport horizontally.
    let anchor = rect(380.0, 200.0, 80.0, 20.0);
    let placed = placement(hint(anchor, Side::Above), GAP);
    assert_eq!(placed.y, anchor.y - GAP - PANEL.1);
    assert_eq!(placed.x, VIEWPORT.right() - PANEL.0, "right-clamped");
}

#[test]
fn out_of_room_flips_to_the_other_side() {
    // Below cannot hold the panel: flip above.
    let anchor = rect(10.0, 260.0, 80.0, 20.0);
    let placed = placement(hint(anchor, Side::Below), GAP);
    assert_eq!(placed.y, anchor.y - GAP - PANEL.1, "flipped above");

    // Above cannot hold the panel: flip below.
    let anchor = rect(10.0, 5.0, 80.0, 20.0);
    let placed = placement(hint(anchor, Side::Above), GAP);
    assert_eq!(placed.y, anchor.bottom() + GAP, "flipped below");
}

#[test]
fn neither_side_fits_keeps_the_preferred_side_clamped() {
    let hint = AnchorHint {
        anchor: rect(0.0, 30.0, 100.0, 20.0),
        viewport: rect(0.0, 0.0, 100.0, 60.0),
        side: Side::Below,
        panel: (80.0, 50.0),
    };
    let placed = placement(hint, GAP);
    assert_eq!(
        placed.y, 10.0,
        "the below position clamps to the last legal top"
    );
    assert!(placed.bottom() <= 60.0 && placed.y >= 0.0, "in-viewport");
}

#[test]
fn a_panel_bigger_than_the_viewport_never_panics_and_stays_inside() {
    let hint = AnchorHint {
        anchor: rect(50.0, 50.0, 20.0, 20.0),
        viewport: rect(0.0, 0.0, 40.0, 30.0),
        side: Side::Below,
        panel: (90.0, 80.0),
    };
    let placed = placement(hint, GAP);
    assert_eq!(placed.x, 0.0);
    assert_eq!(placed.y, 0.0);
}

#[test]
fn popover_scene_stamps_the_placement_and_reskins_in_place() {
    let spawn_hint = AnchorHint {
        anchor: rect(10.0, 10.0, 80.0, 20.0),
        viewport: VIEWPORT,
        side: Side::Below,
        panel: PANEL,
    };
    let expected = placement(spawn_hint, ANCHOR_GAP_PX);

    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(popover_scene(
                AnchorHint {
                    anchor: rect(10.0, 10.0, 80.0, 20.0),
                    viewport: VIEWPORT,
                    side: Side::Below,
                    panel: PANEL,
                },
                content_scene(),
                &palette,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut panels = world.query::<(&PopoverPanel, &Node)>();
    let (_, node) = panels.iter(world).next().expect("panel mounted");
    assert_eq!(node.left, Val::Px(expected.x), "stamped left edge");
    assert_eq!(node.top, Val::Px(expected.y), "stamped top edge");
    assert_eq!(node.width, Val::Px(expected.w));
    assert_eq!(node.height, Val::Px(expected.h));
    let panel_entity: Entity = world
        .query::<(Entity, &PopoverPanel)>()
        .iter(world)
        .next()
        .expect("panel entity")
        .0;

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let mut scrims = world.query::<(&PopoverScrim, &BackgroundColor)>();
    let (_, scrim) = scrims.iter(world).next().expect("scrim survives");
    assert_eq!(scrim.0, light.scrim(), "scrim re-derives from light");
    assert!(
        world.get::<PopoverPanel>(panel_entity).is_some(),
        "the panel kept its entity id across the flip"
    );
}
