//! Official directional navigation: Bevy 0.20's spatial arrow-key focus.
//!
//! `bevy_ui::auto_directional_navigation` owns the movement. The app marks
//! the focusable controls with `AutoDirectionalNavigation`, and the official
//! `AutoDirectionalNavigator` picks the neighbour in a compass octant from
//! the *measured* layout: a candidate needs its box (`ComputedNode`), its
//! position (`UiGlobalTransform`) and the camera it is rendered into, so
//! the search never leaves the surface the user is on. The manual edges of
//! `DirectionalNavigationMap` win over the automatic ones, so a single hop
//! can be overridden without giving up the rest.
//!
//! This module owns the application's half: the official plugin's
//! registration, the repository's `AutoNavigationConfig`, the arrow-key
//! gesture, and the typed guard that keeps the gesture away from the
//! surfaces that own the arrow keys themselves — a focused text field (the
//! caret) and an official tab group (a tab list or a dialog).

use bevy::app::{App, Plugin, PreUpdate};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input_focus::directional_navigation::{
    AutoNavigationConfig, DirectionalNavigationPlugin,
};
use bevy::input_focus::tab_navigation::TabGroup;
use bevy::math::{CompassOctant, Dir2};
use bevy::ui::auto_directional_navigation::AutoDirectionalNavigator;

use crate::text_input::TextFieldFocused;

/// The minimum overlap two controls need on the perpendicular axis before
/// the official navigator may call them neighbours.
///
/// The shell's controls sit in rows and columns, so a candidate that only
/// grazes the origin's edge is a control in another lane, not the one the
/// arrow key points at.
pub const MIN_ALIGNMENT_FACTOR: f32 = 0.5;

/// Why the arrow gesture did not move the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrowBlock {
    /// A text field holds the focus: the arrow keys belong to its caret.
    TextFieldFocused,
    /// The focused control sits inside an official tab group — a tab list or
    /// a dialog — and that surface owns the arrow keys.
    TabGroupFocused,
}

/// What an arrow gesture did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrowNavigation {
    /// No arrow key was pressed, or only opposing keys.
    NoGesture,
    /// The gesture was withheld from a surface that owns the arrow keys.
    Blocked(ArrowBlock),
    /// The official navigator moved the focus.
    Moved(Entity),
    /// The official navigator found no neighbour in that direction.
    NoNeighbour,
}

/// The compass octant an arrow gesture asks for.
///
/// Opposing keys cancel (up + down is not a direction), so a gesture that
/// presses both ways on one axis stays "no gesture" instead of picking an
/// arbitrary diagonal.
pub fn arrow_octant(keys: &ButtonInput<KeyCode>) -> Option<CompassOctant> {
    let east =
        keys.just_pressed(KeyCode::ArrowRight) as i8 - keys.just_pressed(KeyCode::ArrowLeft) as i8;
    let north =
        keys.just_pressed(KeyCode::ArrowUp) as i8 - keys.just_pressed(KeyCode::ArrowDown) as i8;
    Dir2::from_xy(east as f32, north as f32)
        .ok()
        .map(CompassOctant::from)
}

/// The surface that owns the arrow keys, if any.
pub fn arrow_block(
    focus: Option<Entity>,
    parents: &Query<&ChildOf>,
    groups: &Query<(), With<TabGroup>>,
    fields: &Query<&TextFieldFocused, With<TextFieldFocused>>,
) -> Option<ArrowBlock> {
    if fields.iter().any(|field| field.0) {
        return Some(ArrowBlock::TextFieldFocused);
    }
    let focused = focus?;
    let mut current = focused;
    while let Ok(parent) = parents.get(current) {
        current = parent.parent();
        if groups.contains(current) {
            return Some(ArrowBlock::TabGroupFocused);
        }
    }
    None
}

/// Decide what an arrow gesture does.
///
/// The decision is pure: the gesture, the surface that owns the arrow keys
/// and the official navigator's answer are its three inputs, so the rule is
/// testable without a laid-out window.
pub fn arrow_navigation(
    keys: &ButtonInput<KeyCode>,
    block: Option<ArrowBlock>,
    navigate: impl FnOnce(CompassOctant) -> Option<Entity>,
) -> ArrowNavigation {
    let Some(octant) = arrow_octant(keys) else {
        return ArrowNavigation::NoGesture;
    };
    if let Some(block) = block {
        return ArrowNavigation::Blocked(block);
    }
    match navigate(octant) {
        Some(entity) => ArrowNavigation::Moved(entity),
        None => ArrowNavigation::NoNeighbour,
    }
}

/// Move the input focus with the arrow keys, through the official
/// directional navigation.
pub fn navigate_arrow_keys(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    parents: Query<&ChildOf>,
    groups: Query<(), With<TabGroup>>,
    fields: Query<&TextFieldFocused, With<TextFieldFocused>>,
    mut navigator: AutoDirectionalNavigator,
) {
    // A host without a keyboard stack (a headless composition) has no
    // gesture to read.
    let Some(keys) = keys else {
        return;
    };
    let focus = navigator.input_focus();
    let block = arrow_block(focus, &parents, &groups, &fields);
    // The official navigator owns the movement: it reads the measured layout,
    // honours the manual edge map and moves the official input focus. The
    // outcome is the application's read of the same gesture.
    let _ = arrow_navigation(&keys, block, |octant| navigator.navigate(octant).ok());
}

/// Installs the official directional navigation: the upstream plugin, the
/// repository's configuration and the arrow-key gesture.
pub struct DirectionalNavPlugin;

impl Plugin for DirectionalNavPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<DirectionalNavigationPlugin>() {
            app.add_plugins(DirectionalNavigationPlugin);
        }
        app.insert_resource(AutoNavigationConfig {
            min_alignment_factor: MIN_ALIGNMENT_FACTOR,
            max_search_distance: None,
            prefer_aligned: true,
        });
        app.add_systems(PreUpdate, navigate_arrow_keys);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WidgetsPlugin;
    use crate::button::{ButtonVariant, button_scene};
    use crate::palette::UiPalette;
    use crate::theme::Theme;
    use bevy::MinimalPlugins;
    use bevy::app::{App, Startup};
    use bevy::asset::AssetPlugin;
    use bevy::ecs::entity::Entity;
    use bevy::ecs::system::Commands;
    use bevy::input_focus::directional_navigation::DirectionalNavigationMap;
    use bevy::input_focus::{FocusCause, InputFocus};
    use bevy::scene::{CommandsSceneExt, ScenePlugin};
    use bevy::ui::auto_directional_navigation::AutoDirectionalNavigation;
    use bevy::ui_widgets::Button;

    /// The headless composition the widget tests run on: the widget seam
    /// plus the asset and scene stores the `bsn!` mount needs.
    fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins((AssetPlugin::default(), ScenePlugin));
        app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
        app
    }

    /// A keyboard state with the given keys freshly pressed.
    fn pressed(keys: &[KeyCode]) -> ButtonInput<KeyCode> {
        let mut input = ButtonInput::<KeyCode>::default();
        for key in keys {
            input.press(*key);
        }
        input
    }

    #[test]
    fn the_arrow_gesture_asks_for_the_official_compass_octant() {
        assert_eq!(
            arrow_octant(&pressed(&[KeyCode::ArrowUp])),
            Some(CompassOctant::North)
        );
        assert_eq!(
            arrow_octant(&pressed(&[KeyCode::ArrowDown])),
            Some(CompassOctant::South)
        );
        assert_eq!(
            arrow_octant(&pressed(&[KeyCode::ArrowLeft])),
            Some(CompassOctant::West)
        );
        assert_eq!(
            arrow_octant(&pressed(&[KeyCode::ArrowRight])),
            Some(CompassOctant::East)
        );
        assert_eq!(
            arrow_octant(&pressed(&[KeyCode::ArrowUp, KeyCode::ArrowRight])),
            Some(CompassOctant::NorthEast)
        );
        assert_eq!(
            arrow_octant(&pressed(&[KeyCode::ArrowDown, KeyCode::ArrowLeft])),
            Some(CompassOctant::SouthWest)
        );
    }

    #[test]
    fn an_opposing_gesture_is_no_gesture_at_all() {
        assert_eq!(arrow_octant(&pressed(&[])), None);
        assert_eq!(
            arrow_octant(&pressed(&[KeyCode::ArrowUp, KeyCode::ArrowDown])),
            None,
            "opposing keys cancel instead of picking a diagonal"
        );
        assert_eq!(
            arrow_octant(&pressed(&[KeyCode::ArrowLeft, KeyCode::ArrowRight])),
            None
        );
    }

    #[test]
    fn the_official_navigation_is_installed_with_the_repository_configuration() {
        let app = headless_app();
        assert!(
            app.world().contains_resource::<DirectionalNavigationMap>(),
            "the official manual edge map is registered"
        );
        let config = app.world().resource::<AutoNavigationConfig>().clone();
        assert_eq!(config.min_alignment_factor, MIN_ALIGNMENT_FACTOR);
        assert!(config.prefer_aligned);
    }

    #[test]
    fn every_button_is_a_candidate_for_the_official_navigation() {
        let mut app = headless_app();
        app.add_systems(
            Startup,
            |mut commands: Commands, palette: Res<UiPalette>| {
                commands.spawn_scene(button_scene(
                    "run".to_owned(),
                    ButtonVariant::Primary,
                    &palette,
                ));
            },
        );
        app.update();

        let world = app.world_mut();
        let mut buttons = world.query_filtered::<Entity, With<Button>>();
        let button = buttons.iter(world).next().expect("the button is mounted");
        assert!(
            world.get::<AutoDirectionalNavigation>(button).is_some(),
            "a focusable control states that the official navigator may reach it"
        );
    }

    #[test]
    fn the_gesture_is_withheld_from_the_surfaces_that_own_the_arrow_keys() {
        let keys = pressed(&[KeyCode::ArrowRight]);
        assert_eq!(
            arrow_navigation(&keys, Some(ArrowBlock::TextFieldFocused), |_| None),
            ArrowNavigation::Blocked(ArrowBlock::TextFieldFocused),
            "a focused text field keeps the arrow keys for its caret"
        );
        assert_eq!(
            arrow_navigation(&keys, Some(ArrowBlock::TabGroupFocused), |_| None),
            ArrowNavigation::Blocked(ArrowBlock::TabGroupFocused),
            "a tab list or a dialog keeps the arrow keys for its own surface"
        );
        assert_eq!(
            arrow_navigation(&keys, None, |_| None),
            ArrowNavigation::NoNeighbour,
            "an unobstructed gesture that finds no neighbour says so"
        );
        assert_eq!(
            arrow_navigation(&pressed(&[]), None, |_| None),
            ArrowNavigation::NoGesture
        );
    }

    #[test]
    fn the_arrow_gesture_moves_the_focus_through_the_official_navigator() {
        let mut app = headless_app();
        app.add_systems(
            Startup,
            |mut commands: Commands, palette: Res<UiPalette>| {
                commands.spawn_scene(button_scene(
                    "left".to_owned(),
                    ButtonVariant::Primary,
                    &palette,
                ));
                commands.spawn_scene(button_scene(
                    "right".to_owned(),
                    ButtonVariant::Primary,
                    &palette,
                ));
            },
        );
        app.update();

        let world = app.world_mut();
        let mut buttons = world.query_filtered::<Entity, With<Button>>();
        let controls: Vec<Entity> = buttons.iter(world).collect();
        assert_eq!(controls.len(), 2, "the shell mounted two controls");
        let (left, right) = (controls[0], controls[1]);

        // The official map states the hop; the official navigator moves the
        // official input focus along it.
        app.world_mut()
            .resource_mut::<DirectionalNavigationMap>()
            .add_edge(left, right, CompassOctant::East);
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(left, FocusCause::Navigated);

        app.insert_resource(pressed(&[KeyCode::ArrowRight]));
        app.update();

        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(right),
            "the arrow gesture moves the official input focus"
        );
    }
}
