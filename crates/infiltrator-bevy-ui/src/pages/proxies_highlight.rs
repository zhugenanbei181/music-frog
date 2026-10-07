//! Replay shared proxy runs; the widget layer owns native span lifecycle and palette updates.
use crate::pages::proxies::{LastProxiesProjection, NodeNameText, ProxyNodeButton};
use crate::pages::proxies_identity::node_for_entity;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res};
use infiltrator_bevy_widgets::text_runs::TextRuns;
use infiltrator_contract::search_text::SearchTextRun;

pub fn sync_name_highlights(
    mut commands: Commands,
    last: Option<Res<LastProxiesProjection>>,
    names: Query<(Entity, Option<&TextRuns>), With<NodeNameText>>,
    parents: Query<&ChildOf>,
    identities: Query<&ProxyNodeButton>,
) {
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    for (entity, current) in &names {
        let Some(node) = node_for_entity(entity, &parents, &identities, projection) else {
            continue;
        };
        let fallback = [SearchTextRun {
            text: node.name.clone(),
            highlighted: false,
        }];
        let runs = projection
            .name_runs
            .get(&node.name)
            .map(Vec::as_slice)
            .unwrap_or(&fallback);
        if current.is_none_or(|value| value.0 != runs) {
            commands.entity(entity).insert(TextRuns(runs.to_vec()));
        }
    }
}
