//! Headless tests for Tabs & SegmentedControl: capsule percentage metrics,
//! pixel bounds, and the official TabList selection protocol (norm ③) —
//! declarative seeding, selection-driven capsule geometry, and token-swapped
//! label inks. Keyboard navigation, roving tabindex and a11y roles are
//! owned by the official widgets and pinned by their own test suite.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::feathers::theme::ThemeTextColor;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::Selected;
use bevy::ui::prelude::{Node, Val};
use bevy::ui_widgets::{SelectedTab, Tab, ValueChange};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::tabs::{
    SegmentedPillIndicator, SegmentedTab, capsule_metrics, capsule_px_bounds,
    segmented_control_scene,
};
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

/// Mount a 3-tab strip and return (list, tabs-in-index-order) after the
/// declarative seed has landed.
fn mount_strip(selected_index: usize) -> (App, Entity, Vec<Entity>) {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        move |mut commands: Commands, palette: Res<UiPalette>| {
            let tabs = vec!["Rule".to_owned(), "Global".to_owned(), "Direct".to_owned()];
            commands.spawn_scene(segmented_control_scene(tabs, selected_index, &palette));
        },
    );
    app.update();
    app.update();
    app.update();

    let world = app.world_mut();
    let mut found = None;
    {
        let mut strips = world.query::<(Entity, &Children)>();
        for (list, children) in strips.iter(world) {
            let tabs: Vec<Entity> = children
                .iter()
                .copied()
                .filter(|c| world.get::<Tab>(*c).is_some())
                .collect();
            if !tabs.is_empty() {
                found = Some((list, tabs));
            }
        }
    }
    let (list, tabs) = found.expect("strip with direct-child tabs");
    (app, list, tabs)
}

#[test]
fn capsule_metrics_and_pixel_bounds_math() {
    // 3 tabs: width is 33.333%, index 0 at 0%, index 1 at 33.333%, index 2 at 66.667%
    let (l0, w0) = capsule_metrics(0, 3);
    assert_eq!(l0, 0.0);
    assert!((w0 - 33.3333).abs() < 0.01);

    let (l1, w1) = capsule_metrics(1, 3);
    assert!((l1 - 33.3333).abs() < 0.01);
    assert!((w1 - 33.3333).abs() < 0.01);

    let (l2, _w2) = capsule_metrics(2, 3);
    assert!((l2 - 66.6666).abs() < 0.01);

    // Pixel bounds: 300px total, 4px padding -> 292px usable / 4 tabs = 73px per tab
    let (px_l0, px_w0) = capsule_px_bounds(0, 4, 300.0, 4.0);
    assert_eq!(px_l0, 4.0);
    assert_eq!(px_w0, 73.0);

    let (px_l2, px_w2) = capsule_px_bounds(2, 4, 300.0, 4.0);
    assert_eq!(px_l2, 4.0 + 73.0 * 2.0);
    assert_eq!(px_w2, 73.0);
}

#[test]
fn fresh_strip_seeds_selection_and_marks_the_selected_tab() {
    let (app, list, tabs) = mount_strip(1);

    let world = app.world();
    assert_eq!(
        world.get::<SelectedTab>(list).and_then(|s| s.0),
        Some(tabs[1]),
        "the declarative seed selected the second tab"
    );
    assert!(
        world.get::<Selected>(tabs[1]).is_some(),
        "official derived state marks the seeded tab Selected"
    );
    assert!(
        world.get::<Selected>(tabs[0]).is_none(),
        "unseeded tabs stay unmarked"
    );
}

#[test]
fn official_value_change_moves_capsule_and_swaps_label_inks() {
    let (mut app, list, tabs) = mount_strip(0);

    // The official selection request path: ValueChange from the list entity,
    // applied by the official tablist_self_update observer.
    app.world_mut().trigger(ValueChange::<Option<Entity>> {
        source: list,
        value: Some(tabs[2]),
        is_final: true,
    });
    app.update();
    app.update();

    let world = app.world();
    assert_eq!(
        world.get::<SelectedTab>(list).and_then(|s| s.0),
        Some(tabs[2]),
        "selection follows the official ValueChange"
    );

    let world = app.world_mut();
    let mut indicators = world.query::<(&SegmentedPillIndicator, &Node)>();
    let (_, ind_node) = indicators.iter(world).next().expect("indicator exists");
    if let Val::Percent(left_pct) = ind_node.left {
        assert!((left_pct - 66.666).abs() < 0.1, "capsule follows index 2");
    } else {
        panic!("indicator left must be percentage");
    }

    // Label ink: selected tab label rides on_accent, the rest ink_dim.
    let mut inks: Vec<(usize, Token)> = Vec::new();
    {
        let mut labels = world.query::<(&SegmentedTab, &Children)>();
        for (tab, children) in labels.iter(world) {
            for child in children.iter() {
                if let Some(ink) = world.get::<ThemeTextColor>(*child) {
                    let token = if ink.0 == Token::OnAccent.theme_token() {
                        Token::OnAccent
                    } else if ink.0 == Token::InkDim.theme_token() {
                        Token::InkDim
                    } else {
                        panic!("unexpected label token {:?}", ink.0);
                    };
                    inks.push((tab.0, token));
                }
            }
        }
    }
    inks.sort_by_key(|(idx, _)| *idx);
    assert_eq!(
        inks,
        vec![(0, Token::InkDim), (1, Token::InkDim), (2, Token::OnAccent),],
        "ink follows the official selection"
    );
}
