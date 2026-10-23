//! The proxy card's two overlays: the name's pixel fade and the latency
//! sparkline.
//!
//! Both read the owner's facts and invent none of their own:
//!
//! * the **fade** covers the tail of a name the card cannot show, so it is
//!   painted in the card's own colour. The card's fill is translucent, and
//!   painting that fill again over the card would leave a light band, so
//!   the fill is resolved over the surface into the opaque colour a reader
//!   actually sees;
//! * the **sparkline** is the node's own recorded latency history: a
//!   sample's height is its share of the window, so a rising line is a
//!   rising delay.

use crate::pages::proxies::{
    LastProxiesProjection, LatencyTrendIcon, ProxyNodeButton, latency_color,
};
use crate::pages::proxies_identity::node_for_entity;
use bevy::asset::Assets;
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Local, Query, Res, ResMut, SystemParam};
use bevy::image::Image;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    BackgroundColor, BackgroundGradient, ColorStop, Gradient, LinearGradient, Node, PositionType,
    px,
};
use bevy::ui::widget::ImageNode;
use infiltrator_bevy_widgets::chart::sparkline_image;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_presentation::latency_projection::project_proxy_latency;
use lurker_core_contract::outbound::details::OutboundLatencySample;
use std::collections::HashMap;

/// The width of the name's pixel fade, in baseline pixels.
const NAME_FADE_WIDTH_PX: u32 = 28;

/// The latency sparkline's raster width, in baseline pixels.
const LATENCY_SPARKLINE_WIDTH_PX: u32 = 28;

/// The latency sparkline's raster height, in baseline pixels.
const LATENCY_SPARKLINE_HEIGHT_PX: u32 = 14;

/// Marker on the name's pixel fade: the overlay that dissolves a long
/// name into the card instead of cutting it mid-glyph.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeNameFade;

/// The name's pixel fade, painted in the card's own colour.
///
/// The fade lives at the tail of the name row, so a name the card can
/// show is never touched: the gradient only covers the pixels the card
/// could not show anyway.
pub fn name_fade_scene(fill: Color) -> impl Scene + use<> {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            right: px(0.0),
            top: px(0.0),
            bottom: px(0.0),
            width: rem(NAME_FADE_WIDTH_PX),
        }
        BackgroundGradient({ fade_gradient(fill) })
        NodeNameFade
    }
}
/// The latency sparkline: the node's recorded latency history, drawn.
pub fn latency_sparkline_scene(g_idx: usize, n_idx: usize) -> impl Scene + use<> {
    bsn! {
        Node {
            width: rem(LATENCY_SPARKLINE_WIDTH_PX),
            height: rem(LATENCY_SPARKLINE_HEIGHT_PX),
            flex_shrink: 0.0,
        }
        LatencyTrendIcon { group_idx: g_idx, node_idx: n_idx }
    }
}

/// The fade's gradient: the card's own colour, from nothing to the fill.
fn fade_gradient(fill: Color) -> Vec<Gradient> {
    vec![Gradient::Linear(LinearGradient::to_right(vec![
        ColorStop::percent(fill.with_alpha(0.0), 0.0),
        ColorStop::percent(fill, 100.0),
    ]))]
}

/// Paint the name's pixel fade from the card's own fill.
///
/// The card's fill is translucent, and the fade covers the card's own
/// surface: painting the translucent fill again would composite it a
/// second time and leave a light band. The fill is therefore resolved
/// over the surface into the opaque colour the reader actually sees.
pub fn sync_proxy_name_fades(
    palette: Res<UiPalette>,
    parents: Query<&ChildOf>,
    cards: Query<&BackgroundColor, With<ProxyNodeButton>>,
    mut fades: Query<(Entity, &mut BackgroundGradient), With<NodeNameFade>>,
) {
    for (entity, mut gradient) in &mut fades {
        let Some(fill) = owning_card_fill(entity, &parents, &cards) else {
            continue;
        };
        let opaque = palette.over_surface(fill);
        if fade_ends_with(&gradient, opaque) {
            continue;
        }
        *gradient = BackgroundGradient(fade_gradient(opaque));
    }
}

/// The fill of the card that owns one node: the card is the nearest
/// ancestor that carries the node's identity.
fn owning_card_fill(
    entity: Entity,
    parents: &Query<&ChildOf>,
    cards: &Query<&BackgroundColor, With<ProxyNodeButton>>,
) -> Option<Color> {
    let mut current = entity;
    loop {
        if let Ok(card) = cards.get(current) {
            return Some(card.0);
        }
        let Ok(parent) = parents.get(current) else {
            return None;
        };
        current = parent.parent();
    }
}

/// Whether the fade already ends in this colour.
fn fade_ends_with(gradient: &BackgroundGradient, fill: Color) -> bool {
    gradient.0.first().and_then(|gradient| match gradient {
        Gradient::Linear(linear) => linear.stops.last().map(|stop| stop.color),
        Gradient::Radial(_) | Gradient::Conic(_) => None,
    }) == Some(fill)
}

#[derive(SystemParam)]
pub struct ProxyLatencySparklineParams<'w, 's> {
    palette: Res<'w, UiPalette>,
    last: Option<Res<'w, LastProxiesProjection>>,
    images: Option<ResMut<'w, Assets<Image>>>,
    parents: Query<'w, 's, &'static ChildOf>,
    identities: Query<'w, 's, &'static ProxyNodeButton>,
    slots: Query<'w, 's, (Entity, Option<&'static ImageNode>), With<LatencyTrendIcon>>,
    commands: Commands<'w, 's>,
}

/// Paint the latency sparkline from the owner's recorded history.
///
/// The strip shows the node's own samples: the height of a sample is its
/// share of the window, so the slowest sample of the window sits at the
/// top and a flat history sits in the middle. A node the owner never
/// measured carries no sample and therefore no line.
pub fn sync_proxy_latency_sparklines(
    params: ProxyLatencySparklineParams,
    mut painted: Local<PaintedSparklines>,
) {
    let ProxyLatencySparklineParams {
        palette,
        last,
        images,
        parents,
        identities,
        slots,
        mut commands,
    } = params;
    let Some(mut images) = images else {
        return;
    };
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    let mut next = HashMap::new();
    for (entity, image) in &slots {
        let Some(node) = node_for_entity(entity, &parents, &identities, projection) else {
            continue;
        };
        let heights = normalized_latency_heights(&node.latency_samples);
        if heights.is_empty() {
            continue;
        }
        let ink = latency_color(project_proxy_latency(node.delay_ms).band, &palette);
        let signature = (latency_delays(&node.latency_samples), ink);
        if painted.get(&entity) == Some(&signature) {
            next.insert(entity, signature);
            continue;
        }
        next.insert(entity, signature);
        let raster = sparkline_image(
            &heights,
            LATENCY_SPARKLINE_WIDTH_PX,
            LATENCY_SPARKLINE_HEIGHT_PX,
            ink,
            None,
        );
        match image {
            Some(image) => {
                if let Some(mut existing) = images.get_mut(&image.image) {
                    *existing = raster;
                }
            }
            None => {
                let handle = images.add(raster);
                commands.entity(entity).insert(ImageNode {
                    image: handle,
                    ..ImageNode::default()
                });
            }
        }
    }
    *painted = next;
}

/// The painted state of one sparkline slot: the samples it carries and
/// the ink it was painted with. An unchanged slot costs nothing.
type PaintedSparklines = HashMap<Entity, (Vec<u32>, Color)>;

/// The recorded delays of one node, oldest first.
fn latency_delays(samples: &[OutboundLatencySample]) -> Vec<u32> {
    samples.iter().map(|sample| sample.delay_ms).collect()
}

/// The normalized heights of a latency history: a sample's height is its
/// share of the window, so the slowest sample sits at the top and a flat
/// history sits in the middle.
pub fn normalized_latency_heights(samples: &[OutboundLatencySample]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    let delays: Vec<u32> = samples.iter().map(|sample| sample.delay_ms).collect();
    let min = delays.iter().copied().min().unwrap_or_default();
    let max = delays.iter().copied().max().unwrap_or_default();
    let span = max.saturating_sub(min);
    if span == 0 {
        return vec![0.0; delays.len()];
    }
    delays
        .iter()
        .map(|delay| (*delay - min) as f32 / span as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(delay_ms: u32) -> OutboundLatencySample {
        OutboundLatencySample {
            observed_at: "test".to_owned(),
            delay_ms,
        }
    }

    #[test]
    fn a_flat_history_sits_in_the_middle() {
        let heights = normalized_latency_heights(&[sample(38), sample(38), sample(38)]);
        assert_eq!(heights, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn the_slowest_sample_of_the_window_is_the_tallest() {
        let heights = normalized_latency_heights(&[sample(90), sample(30), sample(60)]);
        assert_eq!(heights, vec![1.0, 0.0, 0.5]);
    }

    #[test]
    fn an_unmeasured_node_has_no_line() {
        assert!(normalized_latency_heights(&[]).is_empty());
    }
}
