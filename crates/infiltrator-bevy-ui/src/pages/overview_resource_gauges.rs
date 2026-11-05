//! The Overview resource gauge card: authoritative memory and CPU segments.

use crate::pages::overview::LastOverviewProjection;
use crate::projection::OverviewProjection;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::Without;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::Overflow;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, percent,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::design_resources;
use infiltrator_contract::resources::ResourceAvailability;
use infiltrator_presentation::dashboard_hero_projection::ResourceGaugeProjection;

/// The height of one segmented gauge track, in baseline pixels.
const GAUGE_TRACK_HEIGHT_PX: f32 = 8.0;

/// Marker on one resource gauge's displayed value.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceGaugeValue(pub ResourceGaugeKind);

/// Marker on one resource gauge's filled segment width.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceGaugeFill(pub ResourceGaugeKind);

/// Which authoritative observation one gauge projects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResourceGaugeKind {
    #[default]
    Memory,
    Cpu,
}

/// The resource gauge card: two authoritative segmented gauges and values.
pub fn resource_gauges_scene(
    projection: &ResourceGaugeProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        @{ icon_scene(IconId::Zap, 16.0, palette.accent) }
                        --
                        LocalizedText::plain("overview_resource_usage") TextRole(design_resources::TextRole::Heading)
                    ]
                ]
            }),
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S12),
                    }
                    Children [
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: rem(space::S6),
                        }
                        Children [
                            Node {
                                width: percent(100),
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                            }
                            Children [
                                LocalizedText::plain("overview_memory") TextRole(design_resources::TextRole::Caption)
                                --
                                Text({ projection.memory_text.clone() })
                                ResourceGaugeValue(ResourceGaugeKind::Memory)
                                TextRole(design_resources::TextRole::Mono)
                            ]
                            --
                            @{ gauge_track(projection.memory_fraction, palette.accent, ResourceGaugeKind::Memory, palette) }
                        ]
                        --
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: rem(space::S6),
                        }
                        Children [
                            Node {
                                width: percent(100),
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                            }
                            Children [
                                LocalizedText::plain("overview_cpu") TextRole(design_resources::TextRole::Caption)
                                --
                                Text({ projection.cpu_text.clone() })
                                ResourceGaugeValue(ResourceGaugeKind::Cpu)
                                TextRole(design_resources::TextRole::Mono)
                            ]
                            --
                            @{ gauge_track(projection.cpu_fraction, palette.success, ResourceGaugeKind::Cpu, palette) }
                        ]
                    ]
            }),
        ],
        palette,
    )
}

/// Keep the passive gauge card aligned with the shared projection.
pub(crate) fn sync_resource_gauges(
    last: Option<Res<LastOverviewProjection>>,
    mut values: Query<(&ResourceGaugeValue, &mut Text)>,
    mut fills: Query<(&ResourceGaugeFill, &mut Node), Without<ResourceGaugeValue>>,
) {
    let Some(projection) = last.as_deref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    let observed = resource_projection(projection);
    for (kind, mut text) in &mut values {
        let value = match kind.0 {
            ResourceGaugeKind::Memory => observed.memory_text.clone(),
            ResourceGaugeKind::Cpu => observed.cpu_text.clone(),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    for (kind, mut node) in &mut fills {
        let fraction = match kind.0 {
            ResourceGaugeKind::Memory => observed.memory_fraction,
            ResourceGaugeKind::Cpu => observed.cpu_fraction,
        } * 100.0;
        let width = percent(fraction);
        if node.width != width {
            node.width = width;
        }
    }
}

fn resource_projection(projection: &OverviewProjection) -> ResourceGaugeProjection {
    ResourceGaugeProjection::new(
        match &projection.memory_bytes {
            ResourceAvailability::Available(bytes) => Some(*bytes),
            _ => None,
        },
        projection.memory_limit_bytes,
        match &projection.cpu_percent {
            ResourceAvailability::Available(percent) => Some(*percent),
            _ => None,
        },
    )
}

fn gauge_track(
    fraction: f32,
    color: Color,
    kind: ResourceGaugeKind,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                height: rem(GAUGE_TRACK_HEIGHT_PX),
                border_radius: BorderRadius::MAX,
                overflow: Overflow::clip(),
            }
            BackgroundColor({ palette.border })
            Children [
                Node {
                    width: percent(fraction.clamp(0.0, 1.0) * 100.0),
                    height: percent(100),
                    border_radius: BorderRadius::MAX,
                }
                BackgroundColor({ color })
                ResourceGaugeFill(kind)
            ]
    }
}
