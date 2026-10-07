//! Replay proxy facts by stable identities; native controls keep their entity identity.
use crate::pages::proxies::{
    GroupCurrentText, GroupFoldText, GroupNodesContainer, LastProxiesProjection, LatencyText,
    NodeFlagText, NodeNameText, NodeProtoText, ProxiesLine, ProxiesLineKind,
    ProxiesProjectionUpdated, ProxyNodeButton, latency_color,
};
use crate::pages::proxies_filter::format_protocol_chip;
use crate::pages::proxies_identity::{ProxyGroupIdentity, group_for_entity, node_for_entity};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::observer::On;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, BorderColor, Display, Node};
use infiltrator_application::latency_projection::project_proxy_latency;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_shared::country_flags::node_flag_emoji;

#[derive(QueryData)]
#[query_data(mutable)]
pub struct ProxyTextParts {
    entity: Entity,
    text: &'static mut Text,
    color: Option<&'static mut TextColor>,
    copy: Option<&'static mut LocalizedText>,
    line: Option<&'static ProxiesLine>,
    current: Option<&'static GroupCurrentText>,
    fold: Option<&'static GroupFoldText>,
    latency: Option<&'static LatencyText>,
    name: Option<&'static NodeNameText>,
    flag: Option<&'static NodeFlagText>,
    protocol: Option<&'static NodeProtoText>,
}

#[derive(SystemParam)]
pub struct ProxyIdentityQueries<'w, 's> {
    parents: Query<'w, 's, &'static ChildOf>,
    groups: Query<'w, 's, &'static ProxyGroupIdentity>,
    nodes: Query<'w, 's, &'static ProxyNodeButton>,
}

#[derive(SystemParam)]
pub struct ProxyRenderContext<'w> {
    palette: Res<'w, UiPalette>,
    locale: Res<'w, UiLocale>,
}

pub fn apply_proxies_projection(
    update: On<ProxiesProjectionUpdated>,
    context: ProxyRenderContext,
    mut last: ResMut<LastProxiesProjection>,
    mut labels: Query<ProxyTextParts>,
    identities: ProxyIdentityQueries,
    mut containers: Query<(Entity, &mut Node, &GroupNodesContainer)>,
    mut visuals: Query<(
        &ProxyNodeButton,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut ControlVisual,
    )>,
) {
    let projection = &update.0;
    for mut label in &mut labels {
        if let Some(line) = label.line {
            match line.0 {
                ProxiesLineKind::ActiveExit => label.text.0 = projection.active_exit.clone(),
                ProxiesLineKind::Summary => {
                    if let Some(ref mut copy) = label.copy {
                        copy.params = vec![
                            ("groups", projection.groups.len().to_string()),
                            ("nodes", projection.total_nodes().to_string()),
                        ];
                    }
                }
                ProxiesLineKind::TestStatus => {
                    if let Some(ref mut copy) = label.copy {
                        copy.key = if projection.testing {
                            "proxies_testing"
                        } else {
                            "proxies_test_ready"
                        };
                    }
                }
            }
        } else if label.current.is_some() || label.fold.is_some() {
            if let Some(group) = group_for_entity(
                label.entity,
                &identities.parents,
                &identities.groups,
                projection,
            ) && let Some(ref mut copy) = label.copy
            {
                if label.current.is_some() {
                    copy.params = vec![("node", group.current.clone())];
                } else {
                    copy.key = if group.expanded {
                        "rules_collapse"
                    } else {
                        "rules_expand"
                    };
                }
            }
        } else if let Some(node) = node_for_entity(
            label.entity,
            &identities.parents,
            &identities.nodes,
            projection,
        ) {
            if label.name.is_some() {
                label.text.0.clear();
            } else if label.flag.is_some() {
                label.text.0 = node_flag_emoji(&node.name).into();
            } else if label.protocol.is_some() {
                label.text.0 = format_protocol_chip(&node.node_type);
            } else if label.latency.is_some() {
                let latency = project_proxy_latency(node.delay_ms);
                let copy = LocalizedText::new(latency.key, latency.params);
                label.text.0 = copy.render(&context.locale);
                if let Some(ref mut current) = label.copy {
                    **current = copy;
                }
                if let Some(ref mut color) = label.color {
                    color.0 = latency_color(latency.band, &context.palette);
                }
            }
        }
    }
    for (entity, mut layout, _) in &mut containers {
        layout.display =
            if group_for_entity(entity, &identities.parents, &identities.groups, projection)
                .is_some_and(|group| group.expanded)
            {
                Display::Flex
            } else {
                Display::None
            };
    }
    for (identity, mut fill, mut border, mut visual) in &mut visuals {
        if let Some(node) = projection
            .groups
            .iter()
            .find(|group| group.name == identity.group_name)
            .and_then(|group| {
                group
                    .proxies
                    .iter()
                    .find(|node| node.name == identity.node_name)
            })
        {
            visual.0 = node.selected;
            fill.0 = if node.selected {
                context.palette.accent_container
            } else {
                context.palette.surface_elevated
            };
            let color = if node.selected {
                context.palette.accent
            } else {
                context.palette.border
            };
            border.top = color;
            border.bottom = color;
            border.left = color;
            border.right = color;
        }
    }
    last.0 = Some(projection.clone());
}
