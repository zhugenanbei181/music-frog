//! Headless tests for Toast: queue capacity, dismiss, timed expiration, and stack scene.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;
use infiltrator_bevy_widgets::toast::{
    ToastCard, ToastContainer, ToastKind, ToastMessage, ToastQueue, toast_stack_scene,
};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn toast_queue_capacity_and_timer_expiration() {
    let mut queue = ToastQueue::new(3);

    // Push 3 toasts
    let _id1 = queue.push("Toast 1", ToastKind::Info, 2.0);
    let id2 = queue.push("Toast 2", ToastKind::Success, 5.0);
    let id3 = queue.push("Toast 3", ToastKind::Warning, 10.0);
    assert_eq!(queue.len(), 3);

    // Push 4th toast -> evicts id1
    let id4 = queue.push("Toast 4", ToastKind::Danger, 1.0);
    assert_eq!(queue.len(), 3);
    assert_eq!(queue.items()[0].id, id2);
    assert_eq!(queue.items()[2].id, id4);

    // Advance 1.5 seconds -> id4 expires
    let expired = queue.tick(1.5);
    assert_eq!(expired, vec![id4]);
    assert_eq!(queue.len(), 2);

    // Dismiss id2 explicitly
    assert!(queue.dismiss(id2));
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.items()[0].id, id3);
}

#[test]
fn toast_stack_scene_spawns_on_token_layers() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let toasts = vec![
                ToastMessage::new(1, "Success notification", ToastKind::Success, 5.0),
                ToastMessage::new(2, "Warning alert", ToastKind::Warning, 5.0),
            ];
            commands.spawn_scene(toast_stack_scene(&toasts, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut containers = world.query::<&ToastContainer>();
    assert_eq!(containers.iter(world).count(), 1);

    let mut cards = world.query::<&ToastCard>();
    assert_eq!(cards.iter(world).count(), 2);
}
