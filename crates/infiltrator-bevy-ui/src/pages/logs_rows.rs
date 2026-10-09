//! Log row topology is scoped to its container; data updates preserve stable native rows.
use crate::pages::logs::{LogsProjectionUpdated, log_row_scene};
use crate::pages::logs_virtual::LogsVirtualNodes;
use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::CommandsSceneExt;
use infiltrator_bevy_widgets::palette::UiPalette;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogRowsContainer;
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogRowIdentity(pub u64);

pub struct LogsRowsPlugin;
impl Plugin for LogsRowsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(reconcile_rows);
    }
}
fn reconcile_rows(
    update: On<LogsProjectionUpdated>,
    containers: Query<(Entity, Option<&Children>), With<LogRowsContainer>>,
    virtual_containers: Query<(), With<LogsVirtualNodes>>,
    identities: Query<&LogRowIdentity>,
    palette: Res<UiPalette>,
    mut commands: Commands,
) {
    if !virtual_containers.is_empty() {
        // BANDROID-010: the recycler owns the row topology whenever its
        // container is mounted, so the full-mount reconcile must stand down.
        // The per-frame search replay restamps the mounted rows in place.
        return;
    }
    let Ok((root, children)) = containers.single() else {
        return;
    };
    let existing: Vec<_> = children
        .into_iter()
        .flat_map(|children| children.iter())
        .filter_map(|child| identities.get(*child).ok().map(|id| id.0))
        .collect();
    let requested: Vec<_> = update.0.entries.iter().map(|entry| entry.id).collect();
    if existing == requested {
        return;
    }
    commands.entity(root).despawn_children();
    for (index, entry) in update.0.entries.iter().enumerate() {
        commands
            .spawn_scene(log_row_scene(index, entry, &palette))
            .insert(ChildOf(root));
    }
}
