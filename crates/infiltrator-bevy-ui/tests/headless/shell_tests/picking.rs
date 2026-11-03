//! Behavior cases for the shell's picking policy.
//! test-intent: behavior
//!
//! A layer that spans the whole surface must not take the pointer: Bevy's UI
//! picking backend blocks every hit below a node that carries no picking
//! policy, so a full-screen decorative container that stays silent swallows
//! every touch on the surface underneath it. The Android host is where the
//! failure shows — the tap lands on the overlay and the control below it
//! never sees it.

use super::*;
use bevy::ecs::hierarchy::ChildOf;
use bevy::picking::Pickable;
use bevy::ui::Val;
use bevy::ui_widgets::ModalDialogBarrier;
use infiltrator_bevy_ui::shell_scene::nav::ShellTooltipOverlay;
use infiltrator_bevy_widgets::picking;

/// A node that covers the whole surface in both axes.
fn covers_the_surface(node: &Node) -> bool {
    matches!(node.width, Val::Percent(100.0))
        && matches!(node.height, Val::Percent(100.0))
        && node.display != Display::None
}

#[test]
fn the_tooltip_overlay_does_not_take_the_pointer() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    let mut overlays = world.query::<(&ShellTooltipOverlay, &Pickable)>();
    let (_, pickable) = overlays.single(world).expect("exactly one tooltip overlay");
    assert_eq!(
        *pickable,
        picking::NON_INTERACTIVE,
        "the tooltip overlay spans the whole surface, so it must use the shared \
         non-interactive policy instead of taking the pointer from the controls below it"
    );
}

fn has_modal_ancestor(world: &World, entity: Entity) -> bool {
    let mut current = entity;
    while let Some(child_of) = world.get::<ChildOf>(current) {
        current = child_of.parent();
        if world.get::<ModalDialogBarrier>(current).is_some() {
            return true;
        }
    }
    false
}

#[test]
fn no_decorative_full_surface_layer_blocks_the_pointer() {
    let mut app = mounted_shell();
    let world = app.world_mut();

    // A modal layer owns the surface while it is open — that is what its
    // barrier is for. The shell root is the bottom of the stack, so nothing
    // is under it to block.
    let mut blockers: Vec<String> = Vec::new();
    let mut full_surface = world.query::<(Entity, &Node)>();
    for (entity, node) in full_surface.iter(world) {
        if !covers_the_surface(node) {
            continue;
        }
        if world.get::<ModalDialogBarrier>(entity).is_some() {
            continue;
        }
        if has_modal_ancestor(world, entity) {
            continue;
        }
        if world.get::<ShellRoot>(entity).is_some() {
            continue;
        }
        let policy = world.get::<Pickable>(entity);
        match policy {
            Some(pickable) if pickable.should_block_lower => {
                blockers.push(format!("{entity} blocks the pointer"));
            }
            None => {
                let mut names = Vec::new();
                if let Ok(iter) = world.inspect_entity(entity) {
                    for (_id, info) in iter {
                        names.push(info.name().to_string());
                    }
                }
                let parent = world.get::<ChildOf>(entity).map(|c| c.parent());
                let parent_names: Vec<String> = parent
                    .map(|p| {
                        world
                            .inspect_entity(p)
                            .ok()
                            .map(|iter| iter.map(|(_, info)| info.name().to_string()).collect())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                blockers.push(format!("{entity} (parent: {parent:?}, parent components: {parent_names:?}) carries no picking policy (components: {names:?})"));
            }
            Some(_) => {}
        }
    }
    assert!(
        blockers.is_empty(),
        "these full-surface layers swallow every touch below them: {blockers:?}"
    );
}
