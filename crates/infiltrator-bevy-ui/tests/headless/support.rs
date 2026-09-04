use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::scene::ScenePlugin;
use bevy::ui::widget::Text;
use infiltrator_bevy_ui::app::ContentSlot;
use infiltrator_bevy_ui::route::{PageRoot, Route};

pub fn headless_plugins(app: &mut App) {
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(ScenePlugin);
    app.init_asset::<Image>();
}

pub fn page_root(world: &mut World) -> (Entity, Route) {
    let mut query = world.query::<(Entity, &PageRoot)>();
    let (entity, root) = query.single(world).expect("single page root mounted");
    (entity, root.0)
}

pub fn content_slot(world: &mut World) -> Entity {
    let mut query = world.query_filtered::<Entity, bevy::ecs::query::With<ContentSlot>>();
    query.single(world).expect("single ContentSlot in shell")
}

pub fn subtree_has_text(world: &World, root: Entity, needle: &str) -> bool {
    if let Some(text) = world.get::<Text>(root)
        && text.0.contains(needle)
    {
        return true;
    }
    if let Some(children) = world.get::<Children>(root) {
        for child in children.iter() {
            if subtree_has_text(world, *child, needle) {
                return true;
            }
        }
    }
    false
}
