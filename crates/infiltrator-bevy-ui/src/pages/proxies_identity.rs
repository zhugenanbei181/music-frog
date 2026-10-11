//! Stable identities resolve facts through mounted ancestors instead of projection positions.
use crate::pages::proxies::{ProxiesProjection, ProxyGroup, ProxyNode, ProxyNodeButton};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::system::Query;
use infiltrator_contract::outbound::identity::OutboundId;

#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyGroupIdentity(pub String);
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyGroupOutboundIdentity(pub Option<OutboundId>);
#[derive(Component, Clone, Copy, Default)]
pub struct ProxyCardsContainer;

pub fn group_for_entity<'a>(
    mut entity: Entity,
    parents: &Query<&ChildOf>,
    identities: &Query<&ProxyGroupIdentity>,
    outbound_identities: &Query<&ProxyGroupOutboundIdentity>,
    projection: &'a ProxiesProjection,
) -> Option<&'a ProxyGroup> {
    loop {
        if let Ok(identity) = identities.get(entity) {
            let outbound_id = outbound_identities
                .get(entity)
                .ok()
                .and_then(|identity| identity.0.as_ref());
            return projection.groups.iter().find(|group| {
                outbound_id.map_or(group.name == identity.0, |id| {
                    group.collection_id.as_ref() == Some(id)
                })
            });
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
                .find(|group| {
                    group
                        .collection_id
                        .as_ref()
                        .map_or(group.name == identity.group_name, |id| {
                            Some(id) == identity.collection_id.as_ref()
                        })
                })
                .and_then(|group| {
                    group.proxies.iter().find(|node| {
                        identity
                            .outbound_id
                            .as_ref()
                            .map_or(node.name == identity.node_name, |id| {
                                node.outbound_id.as_ref() == Some(id)
                            })
                    })
                });
        }
        entity = parents.get(entity).ok()?.parent();
    }
}
