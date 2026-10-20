//! Headless test for the theme-switch seam: a `ThemeSwitch` trigger rewrites
//! the palette resource and restamps text and control fills in place — the
//! same entities before and after, no remount of the scene tree.
//!
//! The same module owns the shared toggle-switch geometry: the capsule track
//! and the sliding knob every product surface reuses. These cases pin the
//! open/closed geometry (track size, knob size, knob offset per state,
//! capsule radius, hairline border) so no page can re-declare it.

use super::support::headless_app;
use bevy::app::Startup;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, bsn};
use bevy::text::TextColor;
use bevy::ui::{BackgroundColor, BorderColor, BorderRadius, Node, PositionType, UiRect, Val};
use infiltrator_bevy_widgets::button::{ControlVisual, pill_scene};
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::switch::{
    SWITCH_KNOB_OFF_OFFSET_PX, SWITCH_KNOB_ON_OFFSET_PX, SWITCH_KNOB_SIZE_PX,
    SWITCH_TRACK_HEIGHT_PX, SWITCH_TRACK_WIDTH_PX, SwitchColors, SwitchKnob, SwitchPosition,
    SwitchTrack, ThemeSwitch, switch_control_scene, switch_knob_scene, switch_track_scene,
};
use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin};

#[test]
fn switch_to_light_rethemes_the_mounted_tree_in_place() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(pill_scene("proxy".to_string(), false, &palette));
            commands.spawn_scene(checkbox_scene("tun".to_string(), true, &palette));
        },
    );
    app.update();

    let dark = UiPalette::new(&Theme::dark());
    let light = UiPalette::new(&Theme::light());
    assert_ne!(dark.ink, light.ink, "the modes under test are distinct");

    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &TextColor)>();
    let text_ids: Vec<Entity> = texts.iter(world).map(|(entity, _)| entity).collect();
    assert!(!text_ids.is_empty());

    let world = app.world_mut();
    let mut pills = world.query::<(&ControlVisual, &BackgroundColor)>();
    let pill_fill_before = pills
        .iter(world)
        .next()
        .map(|(_, fill)| *fill)
        .expect("one pill with its fill");
    assert_eq!(pill_fill_before, BackgroundColor(dark.surface_elevated));

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();

    // The palette resource now resolves the light token set.
    let world = app.world_mut();
    assert_eq!(*world.resource::<UiPalette>(), light);

    // Text entities kept their identity and picked up light ink.
    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &TextColor)>();
    let after: Vec<Entity> = texts.iter(world).map(|(entity, _)| entity).collect();
    assert_eq!(
        after, text_ids,
        "text entities were restamped, not remounted"
    );
    let mut texts = world.query::<&TextColor>();
    for ink in texts.iter(world) {
        assert_eq!(ink.0, light.ink, "text ink refreshed to the light token");
    }

    // The idle pill re-projected the light control surface.
    let world = app.world_mut();
    let mut pills = world.query::<(&ControlVisual, &BackgroundColor)>();
    let pill_fill_after = pills
        .iter(world)
        .next()
        .map(|(_, fill)| *fill)
        .expect("the pill keeps its fill");
    assert_eq!(pill_fill_after, BackgroundColor(light.surface_elevated));

    // A second switch returns everything to the dark tokens.
    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(ThemeSkin::Dark));
    app.update();

    let world = app.world_mut();
    assert_eq!(*world.resource::<UiPalette>(), dark);
    let mut texts = world.query::<&TextColor>();
    for ink in texts.iter(world) {
        assert_eq!(ink.0, dark.ink);
    }
}

/// The track is one shared capsule: 38×22 baseline pixels, a hairline edge and
/// the capsule radius, with the caller's own fill and edge colors.
#[test]
fn switch_track_paints_the_shared_capsule_geometry() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(switch_track_scene(&palette, palette.accent, palette.border));
        },
    );
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut tracks = world.query::<(&Node, &BackgroundColor, &BorderColor)>();
    let (node, fill, edge) = tracks.iter(world).next().expect("track mounted");
    assert_eq!(node.width, rem(SWITCH_TRACK_WIDTH_PX));
    assert_eq!(node.height, rem(SWITCH_TRACK_HEIGHT_PX));
    assert_eq!(
        node.border,
        UiRect::all(Val::Px(palette.hairline_px)),
        "the track edge is the hairline, never a scaled rem size"
    );
    assert_eq!(node.border_radius, BorderRadius::MAX);
    assert_eq!(node.position_type, PositionType::Relative);
    assert_eq!(*fill, BackgroundColor(palette.accent));
    assert_eq!(edge.top, palette.border);
    assert_eq!(edge.right, palette.border);
    assert_eq!(edge.bottom, palette.border);
    assert_eq!(edge.left, palette.border);
}

/// The knob is one shared 16×16 capsule whose absolute left offset is the
/// typed rest position: on rests at 18px, off rests at 2px.
#[test]
fn switch_knob_rests_at_the_typed_on_and_off_offsets() {
    for (position, expected_offset) in [
        (SwitchPosition::On, SWITCH_KNOB_ON_OFFSET_PX),
        (SwitchPosition::Off, SWITCH_KNOB_OFF_OFFSET_PX),
    ] {
        let mut app = headless_app();
        let knob = Color::WHITE;
        app.add_systems(Startup, move |mut commands: Commands| {
            commands.spawn_scene(switch_knob_scene(knob, position));
        });
        app.update();

        let world = app.world_mut();
        let mut knobs = world.query::<(&Node, &BackgroundColor)>();
        let (node, fill) = knobs.iter(world).next().expect("knob mounted");
        assert_eq!(node.left, rem(expected_offset));
        assert_eq!(node.width, rem(SWITCH_KNOB_SIZE_PX));
        assert_eq!(node.height, rem(SWITCH_KNOB_SIZE_PX));
        assert_eq!(node.border_radius, BorderRadius::MAX);
        assert_eq!(node.position_type, PositionType::Absolute);
        assert_eq!(*fill, BackgroundColor(knob));
    }
}

/// The binary on/off fact maps to exactly one offset, and the knob travels
/// exactly its own width between the two rest positions.
#[test]
fn switch_position_is_one_typed_decision_over_the_binary_fact() {
    assert_eq!(
        SwitchPosition::from_enabled(true),
        SwitchPosition::On,
        "an enabled switch rests on"
    );
    assert_eq!(
        SwitchPosition::from_enabled(false),
        SwitchPosition::Off,
        "a disabled switch rests off"
    );
    assert_eq!(
        SwitchPosition::On.knob_offset_px(),
        SWITCH_KNOB_ON_OFFSET_PX
    );
    assert_eq!(
        SwitchPosition::Off.knob_offset_px(),
        SWITCH_KNOB_OFF_OFFSET_PX
    );
    assert_eq!(
        SWITCH_KNOB_ON_OFFSET_PX - SWITCH_KNOB_OFF_OFFSET_PX,
        SWITCH_KNOB_SIZE_PX,
        "the knob travels exactly its own width"
    );
}

/// The composition the product pages use: the track scene carries the root
/// node and the knob scene is its child, both mounted from the shared owner.
#[test]
fn switch_track_and_knob_compose_as_parent_and_child() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(bsn! {
                @{ switch_track_scene(&palette, palette.accent, palette.accent) }
                Children [
                    @{ switch_knob_scene(palette.on_accent, SwitchPosition::On) }
                ]
            });
        },
    );
    app.update();

    let world = app.world_mut();
    let mut tracks = world.query::<(Entity, &Node, &Children)>();
    let (_, node, children) = tracks.iter(world).next().expect("track mounted");
    assert_eq!(node.width, rem(SWITCH_TRACK_WIDTH_PX));
    assert_eq!(node.height, rem(SWITCH_TRACK_HEIGHT_PX));

    assert_eq!(children.len(), 1, "the track carries exactly one knob");
    let knob = world.get::<Node>(children[0]).expect("knob node");
    assert_eq!(knob.left, rem(SWITCH_KNOB_ON_OFFSET_PX));
    assert_eq!(knob.width, rem(SWITCH_KNOB_SIZE_PX));
    assert_eq!(knob.height, rem(SWITCH_KNOB_SIZE_PX));
    assert_eq!(knob.border_radius, BorderRadius::MAX);
}

/// The shared control renders one geometry for every consumer: two controls
/// with different colors have the same track and knob geometry, and only the
/// colors differ.
#[test]
fn shared_switch_control_renders_one_geometry_for_every_consumer() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(switch_control_scene(
                &palette,
                SwitchColors {
                    track_fill: palette.accent,
                    track_edge: palette.accent,
                    knob_fill: palette.on_accent,
                },
                SwitchPosition::On,
            ));
            commands.spawn_scene(switch_control_scene(
                &palette,
                SwitchColors {
                    track_fill: palette.surface_elevated,
                    track_edge: palette.border,
                    knob_fill: palette.ink_dim,
                },
                SwitchPosition::On,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut tracks = world.query::<(&SwitchTrack, &Node, &BackgroundColor)>();
    let track_nodes: Vec<Node> = tracks
        .iter(world)
        .map(|(_, node, _)| node.clone())
        .collect();
    let track_fills: Vec<BackgroundColor> = tracks.iter(world).map(|(_, _, fill)| *fill).collect();
    assert_eq!(
        track_nodes.len(),
        2,
        "two consumers mounted the shared control"
    );
    assert_eq!(track_fills.len(), 2);

    // The geometry is the shared owner's: both consumers render the same one.
    assert_eq!(track_nodes[0].width, rem(SWITCH_TRACK_WIDTH_PX));
    assert_eq!(track_nodes[0].height, rem(SWITCH_TRACK_HEIGHT_PX));
    assert_eq!(track_nodes[0].border_radius, BorderRadius::MAX);
    assert_eq!(track_nodes[1].width, track_nodes[0].width);
    assert_eq!(track_nodes[1].height, track_nodes[0].height);
    assert_eq!(track_nodes[1].border, track_nodes[0].border);
    assert_eq!(track_nodes[1].border_radius, track_nodes[0].border_radius);
    assert_eq!(track_nodes[1].position_type, track_nodes[0].position_type);
    // The colors stay the surface's own.
    assert_ne!(track_fills[0], track_fills[1]);

    let world = app.world_mut();
    let mut knobs = world.query::<(&SwitchKnob, &Node, &BackgroundColor)>();
    let knob_nodes: Vec<Node> = knobs.iter(world).map(|(_, node, _)| node.clone()).collect();
    let knob_fills: Vec<BackgroundColor> = knobs.iter(world).map(|(_, _, fill)| *fill).collect();
    assert_eq!(knob_nodes.len(), 2);
    assert_eq!(knob_fills.len(), 2);
    assert_eq!(knob_nodes[0].left, rem(SWITCH_KNOB_ON_OFFSET_PX));
    assert_eq!(knob_nodes[0].width, rem(SWITCH_KNOB_SIZE_PX));
    assert_eq!(knob_nodes[0].height, rem(SWITCH_KNOB_SIZE_PX));
    assert_eq!(knob_nodes[0].border_radius, BorderRadius::MAX);
    assert_eq!(knob_nodes[1].left, knob_nodes[0].left);
    assert_eq!(knob_nodes[1].width, knob_nodes[0].width);
    assert_eq!(knob_nodes[1].height, knob_nodes[0].height);
    assert_eq!(knob_nodes[1].border_radius, knob_nodes[0].border_radius);
    assert_ne!(knob_fills[0], knob_fills[1]);
}

/// The surface flips the typed position; the shared control owns the offset.
#[test]
fn switch_control_knob_follows_the_typed_position_without_a_surface_offset() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(switch_control_scene(
                &palette,
                SwitchColors {
                    track_fill: palette.surface_elevated,
                    track_edge: palette.border,
                    knob_fill: palette.ink_dim,
                },
                SwitchPosition::Off,
            ));
        },
    );
    app.update();

    {
        let world = app.world_mut();
        let mut knobs = world.query::<(&SwitchKnob, &Node)>();
        let (knob, node) = knobs.iter(world).next().expect("knob mounted");
        assert_eq!(knob.0, SwitchPosition::Off);
        assert_eq!(node.left, rem(SWITCH_KNOB_OFF_OFFSET_PX));
    }

    {
        let world = app.world_mut();
        let mut knobs = world.query::<&mut SwitchKnob>();
        for mut knob in knobs.iter_mut(world) {
            knob.0 = SwitchPosition::On;
        }
    }
    app.update();

    let world = app.world_mut();
    let mut knobs = world.query::<(&SwitchKnob, &Node)>();
    let (knob, node) = knobs.iter(world).next().expect("knob mounted");
    assert_eq!(knob.0, SwitchPosition::On);
    assert_eq!(
        node.left,
        rem(SWITCH_KNOB_ON_OFFSET_PX),
        "the shared control moved the knob; no surface computed the offset"
    );
}
