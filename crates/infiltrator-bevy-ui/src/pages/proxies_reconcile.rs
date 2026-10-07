//! Reconcile only added/removed identities and child order; preserve surviving native controls.
use crate::pages::proxies::{GroupNodesContainer, LastProxiesProjection, ProxyNodeButton};
use crate::pages::proxies_card::{group_card_scene, proxy_node_scene};
use crate::pages::proxies_identity::{ProxyCardsContainer, ProxyGroupIdentity};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::{QueryData, QueryFilter, With, Without};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::CommandsSceneExt;
use infiltrator_bevy_widgets::palette::UiPalette;
use std::collections::HashMap;

#[derive(QueryData)]
#[query_data(mutable)]
pub struct ProxyChildren {
    children: &'static mut Children,
    identity: &'static ProxyGroupIdentity,
}

fn group_name<'a, F: QueryFilter>(
    entity: Entity,
    children: &Query<&Children, F>,
    groups: &'a Query<&ProxyGroupIdentity>,
) -> Option<&'a str> {
    if let Ok(group) = groups.get(entity) {
        return Some(&group.0);
    }
    children
        .get(entity)
        .ok()?
        .iter()
        .copied()
        .find_map(|child| group_name(child, children, groups))
}

pub fn sort_proxy_children(
    last: Option<Res<LastProxiesProjection>>,
    mut roots: Query<&mut Children, (With<ProxyCardsContainer>, Without<GroupNodesContainer>)>,
    mut containers: Query<ProxyChildren, (With<GroupNodesContainer>, Without<ProxyCardsContainer>)>,
    other_children: Query<&Children, (Without<ProxyCardsContainer>, Without<GroupNodesContainer>)>,
    groups: Query<&ProxyGroupIdentity>,
    nodes: Query<&ProxyNodeButton>,
) {
    if !last.as_ref().is_some_and(|last| last.is_changed()) {
        return;
    }
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    let ranks: HashMap<_, _> = projection
        .groups
        .iter()
        .enumerate()
        .map(|(rank, group)| (group.name.as_str(), rank))
        .collect();
    for mut children in &mut roots {
        children.sort_by_key(|entity| {
            group_name(*entity, &other_children, &groups)
                .and_then(|name| ranks.get(name).copied())
                .unwrap_or(usize::MAX)
        });
    }
    for mut container in &mut containers {
        let Some(group) = projection
            .groups
            .iter()
            .find(|group| group.name == container.identity.0)
        else {
            continue;
        };
        let ranks: HashMap<_, _> = group
            .proxies
            .iter()
            .enumerate()
            .map(|(rank, node)| (node.name.as_str(), rank))
            .collect();
        container.children.sort_by_key(|entity| {
            nodes
                .get(*entity)
                .ok()
                .and_then(|node| ranks.get(node.node_name.as_str()).copied())
                .unwrap_or(usize::MAX)
        });
    }
}

pub fn reconcile_proxy_cards(
    mut commands: Commands,
    last: Option<Res<LastProxiesProjection>>,
    palette: Res<UiPalette>,
    root: Query<(Entity, Option<&Children>), With<ProxyCardsContainer>>,
    children: Query<&Children, Without<ProxyCardsContainer>>,
    groups: Query<&ProxyGroupIdentity>,
) {
    let Some(last) = last else {
        return;
    };
    if !last.is_changed() {
        return;
    }
    let Some(projection) = &last.0 else {
        return;
    };
    let Ok((root, current)) = root.single() else {
        return;
    };
    let existing: HashMap<_, _> = current
        .into_iter()
        .flat_map(|children| children.iter().copied())
        .filter_map(|entity| group_name(entity, &children, &groups).map(|name| (name, entity)))
        .collect();
    for (index, group) in projection.groups.iter().enumerate() {
        if !existing.contains_key(group.name.as_str()) {
            commands
                .spawn_scene(group_card_scene(index, group, &palette))
                .insert(ChildOf(root));
        }
    }
    for (name, entity) in existing {
        if !projection.groups.iter().any(|group| group.name == name) {
            commands.entity(entity).despawn();
        }
    }
}

pub fn reconcile_proxy_nodes(
    mut commands: Commands,
    last: Option<Res<LastProxiesProjection>>,
    palette: Res<UiPalette>,
    containers: Query<(Entity, Option<&Children>, &ProxyGroupIdentity), With<GroupNodesContainer>>,
    identities: Query<&ProxyNodeButton>,
) {
    let Some(last) = last else {
        return;
    };
    if !last.is_changed() {
        return;
    }
    let Some(projection) = &last.0 else {
        return;
    };
    for (container, current, identity) in &containers {
        let Some((group_index, group)) = projection
            .groups
            .iter()
            .enumerate()
            .find(|(_, group)| group.name == identity.0)
        else {
            continue;
        };
        let existing: HashMap<_, _> = current
            .into_iter()
            .flat_map(|children| children.iter().copied())
            .filter_map(|entity| {
                identities
                    .get(entity)
                    .ok()
                    .map(|node| (node.node_name.as_str(), entity))
            })
            .collect();
        for (index, node) in group.proxies.iter().enumerate() {
            if !existing.contains_key(node.name.as_str()) {
                commands
                    .spawn_scene(proxy_node_scene(
                        group_index,
                        index,
                        &group.name,
                        node,
                        &palette,
                    ))
                    .insert(ChildOf(container));
            }
        }
        for (name, entity) in existing {
            if !group.proxies.iter().any(|node| node.name == name) {
                commands.entity(entity).despawn();
            }
        }
    }
}
