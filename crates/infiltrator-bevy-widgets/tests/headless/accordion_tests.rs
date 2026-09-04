//! Headless tests for Accordion & Collapse: Single/Multiple modes,
//! section toggling, and in-place display sync.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin, bsn};
use bevy::ui::prelude::{Display, Node};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::accordion::{
    AccordionContent, AccordionMode, AccordionState, AccordionStateComp, AccordionToggleEvent,
    collapse_scene,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn accordion_state_machine_single_and_multiple_modes() {
    // Single mode: only 1 open at a time
    let mut single = AccordionState::new(
        vec![
            ("Section 1".to_owned(), true),
            ("Section 2".to_owned(), false),
        ],
        AccordionMode::Single,
    );

    assert!(single.is_expanded(0));
    assert!(!single.is_expanded(1));

    // Open section 2 -> section 1 collapses
    single.toggle(1);
    assert!(!single.is_expanded(0));
    assert!(single.is_expanded(1));

    // Multiple mode: both can be open
    let mut multiple = AccordionState::new(
        vec![
            ("Section 1".to_owned(), true),
            ("Section 2".to_owned(), false),
        ],
        AccordionMode::Multiple,
    );
    multiple.toggle(1);
    assert!(multiple.is_expanded(0));
    assert!(multiple.is_expanded(1));
}

#[test]
fn collapse_scene_mounts_and_toggles_content_display() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let body = Box::new(bsn! { ( Text({ "Inner content".to_owned() }) ) });
            commands.spawn_scene(collapse_scene(
                "Advanced Settings".to_owned(),
                false,
                body,
                &palette,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut roots = world.query::<(Entity, &AccordionStateComp)>();
    let (root_entity, state) = roots.iter(world).next().expect("accordion mounted");
    assert!(!state.0.is_expanded(0));

    let mut contents = world.query::<(&AccordionContent, &Node)>();
    let (_, node) = contents.iter(world).next().expect("content mounted");
    assert_eq!(node.display, Display::None);

    // Send toggle event
    app.world_mut().write_message(AccordionToggleEvent {
        accordion: root_entity,
        index: 0,
    });
    app.update();

    let world = app.world();
    let state_after = world
        .get::<AccordionStateComp>(root_entity)
        .expect("state exists");
    assert!(state_after.0.is_expanded(0));

    let world = app.world_mut();
    let mut contents = world.query::<(&AccordionContent, &Node)>();
    let (_, node_after) = contents.iter(world).next().expect("content exists");
    assert_eq!(node_after.display, Display::Flex);
}
