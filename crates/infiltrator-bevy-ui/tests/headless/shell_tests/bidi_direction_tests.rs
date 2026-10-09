//! BEVY-035: the shell-level BiDi direction resource mirrored into the mounted
//! shell root. RTL reverses the rail side and flips the inline alignment; LTR
//! resolves every mirror to its declared base, so the left-to-right shell is
//! unchanged.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::world::World;
use bevy::scene::ScenePlugin;
use bevy::ui::Node;
use bevy::ui::prelude::{AlignItems, FlexDirection, JustifyContent};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::app::shell_bidi::{ShellDirectionRoot, ShellDirectionRow};
use infiltrator_bevy_widgets::bidi::LayoutDirection;

fn mounted_shell() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.update();
    app
}

fn single_node<T: Component>(world: &mut World) -> Node {
    let mut query = world.query_filtered::<&Node, With<T>>();
    query.single(world).expect("single marked node").clone()
}

#[test]
fn ltr_keeps_the_declared_rail_side_and_alignment() {
    let mut app = mounted_shell();
    let root = single_node::<ShellDirectionRoot>(app.world_mut());
    assert_eq!(root.flex_direction, FlexDirection::Row);

    let row = single_node::<ShellDirectionRow>(app.world_mut());
    assert_eq!(row.justify_content, JustifyContent::FlexStart);
    assert_eq!(row.align_items, AlignItems::Center);
}

#[test]
fn rtl_reverses_the_rail_side_and_flips_the_row_alignment() {
    let mut app = mounted_shell();
    *app.world_mut().resource_mut::<LayoutDirection>() = LayoutDirection::Rtl;
    app.update();

    let root = single_node::<ShellDirectionRoot>(app.world_mut());
    assert_eq!(
        root.flex_direction,
        FlexDirection::RowReverse,
        "RTL places the rail on the right"
    );

    let row = single_node::<ShellDirectionRow>(app.world_mut());
    assert_eq!(row.justify_content, JustifyContent::FlexEnd);
    assert_eq!(row.align_items, AlignItems::Center);
}

#[test]
fn returning_to_ltr_restores_the_declared_layout() {
    let mut app = mounted_shell();
    *app.world_mut().resource_mut::<LayoutDirection>() = LayoutDirection::Rtl;
    app.update();
    *app.world_mut().resource_mut::<LayoutDirection>() = LayoutDirection::Ltr;
    app.update();

    let root = single_node::<ShellDirectionRoot>(app.world_mut());
    assert_eq!(root.flex_direction, FlexDirection::Row);
    let row = single_node::<ShellDirectionRow>(app.world_mut());
    assert_eq!(row.justify_content, JustifyContent::FlexStart);
}
