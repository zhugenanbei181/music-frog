//! Stable identities resolve facts through mounted ancestors instead of projection positions.
use crate::pages::proxies::{ProxiesProjection, ProxyGroup, ProxyNode, ProxyNodeButton};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::system::Query;

#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyGroupIdentity(pub String);
#[derive(Component, Clone, Copy, Default)]
pub struct ProxyCardsContainer;

pub fn group_for_entity<'a>(
    mut entity: Entity,
    parents: &Query<&ChildOf>,
    identities: &Query<&ProxyGroupIdentity>,
    projection: &'a ProxiesProjection,
) -> Option<&'a ProxyGroup> {
    loop {
        if let Ok(identity) = identities.get(entity) {
            return projection
                .groups
                .iter()
                .find(|group| group.name == identity.0);
        }
        entity = parents.get(entity).ok()?.parent();
    }
}

pub fn node_for_entity<'a>(
    mut entity: Entity,
    parents: &Query<&ChildOf>,
    identities: &Query<&ProxyNodeButton>,
    projection: &'a ProxiesProjection,
) -> Option<&'a ProxyNode> {
    loop {
        if let Ok(identity) = identities.get(entity) {
            return projection
                .groups
                .iter()
                .find(|group| group.name == identity.group_name)
                .and_then(|group| {
                    group
                        .proxies
                        .iter()
                        .find(|node| node.name == identity.node_name)
                });
        }
        entity = parents.get(entity).ok()?.parent();
    }
}
