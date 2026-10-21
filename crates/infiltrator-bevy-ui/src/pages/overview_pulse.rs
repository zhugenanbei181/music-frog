//! The Overview hero's status dot: a breathing halo around the live dot.
//!
//! The dot itself carries the state colour (the page's per-frame reskin owns
//! it); the halo is the motion. It grows and fades once per breath, so a
//! running core reads as alive even while its numbers are flat, and the
//! motion follows the host's reduce-motion preference: with reduced motion
//! the halo stays at rest.

use crate::host_capabilities::{HostPreferences, MotionPreference};
use crate::pages::overview::{LastOverviewProjection, StatusDot, status_dot_color};
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::time::{Time, Virtual};
use bevy::ui::prelude::{AlignItems, BackgroundColor, BorderRadius, JustifyContent, Node};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use std::f32::consts::TAU;

/// The halo's resting diameter, in baseline pixels.
const HALO_MIN_SIZE_PX: f32 = 12.0;
/// The halo's largest diameter, in baseline pixels.
const HALO_MAX_SIZE_PX: f32 = 26.0;
/// The hero's status dot diameter, in baseline pixels.
const STATUS_DOT_SIZE_PX: f32 = 10.0;
/// The halo's peak opacity.
const HALO_PEAK_ALPHA: f32 = 0.32;
/// The breaths per second.
const HALO_BREATH_HZ: f32 = 0.5;

/// Marker on the halo that breathes around the hero's status dot.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusDotHalo;

/// The shared breathing phase of the hero's status dot.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct OverviewPulseState {
    pub phase: f32,
}

/// The hero's status dot: the live dot inside its breathing halo.
pub fn status_dot_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                width: rem(HALO_MAX_SIZE_PX),
                height: rem(HALO_MAX_SIZE_PX),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
            }
            Children [
                Node {
                    width: rem(HALO_MIN_SIZE_PX),
                    height: rem(HALO_MIN_SIZE_PX),
                    border_radius: BorderRadius::MAX,
                }
                BackgroundColor({ Color::NONE })
                StatusDotHalo
                --
                Node {
                    width: rem(STATUS_DOT_SIZE_PX),
                    height: rem(STATUS_DOT_SIZE_PX),
                    border_radius: BorderRadius::MAX,
                }
                BackgroundColor({ palette.ink_dim })
                StatusDot
            ]
    }
}

/// Breathe the halo around the hero's status dot. The system only writes a
/// node when the rendered value actually changes, so a page without a live
/// core costs nothing.
pub fn animate_status_dot(
    time: Res<Time<Virtual>>,
    palette: Res<UiPalette>,
    last: Option<Res<LastOverviewProjection>>,
    preferences: Option<Res<HostPreferences>>,
    mut state: Option<ResMut<OverviewPulseState>>,
    mut halos: Query<(&mut Node, &mut BackgroundColor), With<StatusDotHalo>>,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    let motion = preferences
        .as_deref()
        .map_or(MotionPreference::Standard, |prefs| prefs.effective_motion());
    let breathing = motion == MotionPreference::Standard;
    if breathing {
        state.phase = (state.phase + time.delta_secs() * HALO_BREATH_HZ).rem_euclid(1.0);
    } else {
        state.phase = 0.0;
    }
    // A breath runs from "small and invisible" to "wide and faint" and back.
    let breath = 0.5 - 0.5 * (state.phase * TAU).cos();
    let size = HALO_MIN_SIZE_PX + (HALO_MAX_SIZE_PX - HALO_MIN_SIZE_PX) * breath;
    let alpha = HALO_PEAK_ALPHA * (1.0 - breath);
    let ink = status_dot_color(last.as_deref().and_then(|last| last.0.as_ref()), &palette);
    for (mut node, mut fill) in &mut halos {
        let extent = rem(size);
        if node.width != extent {
            node.width = extent;
        }
        if node.height != extent {
            node.height = extent;
        }
        let color = ink.with_alpha(alpha);
        if fill.0 != color {
            fill.0 = color;
        }
    }
}
