//! The Doctor page's progress indicator: a rotating ring of dots.
//!
//! The repair flow is a typed phase — the shared page model's
//! [`DoctorActionPhase`](infiltrator_presentation::doctor_actions::DoctorActionPhase)
//! — so the indicator's presence is a restatement of that phase and never a
//! second flag. While the phase is `Pending`, the ring turns; the moment the
//! owner answers, the ring disappears with the phase.

use crate::pages::page_models::DoctorPageModelState;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, SceneList, bsn, bsn_list};
use bevy::time::Time;
use bevy::ui::prelude::{BorderRadius, Node, PositionType};
use bevy::ui::{BackgroundColor, Display};
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_presentation::doctor_actions::DoctorActionPhase;
use std::f32::consts::TAU;

/// The number of dots on the ring.
const SPINNER_DOT_COUNT: usize = 8;

/// The ring's radius in baseline pixels.
const SPINNER_RADIUS_PX: f32 = 9.0;

/// One dot's diameter in baseline pixels.
const SPINNER_DOT_PX: f32 = 4.0;

/// The indicator's own square side in baseline pixels (the ring plus a dot).
const SPINNER_SIZE_PX: f32 = 22.0;

/// How long one dot stays lit, in seconds.
const SPINNER_STEP_SECONDS: f32 = 0.12;

/// Marker on the indicator's root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoctorSpinner;

/// Marker on one dot of the ring; its index is its place on the ring.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoctorSpinnerDot(pub usize);

/// One dot: a small disc placed on the ring.
fn dot_scene(index: usize, palette: &UiPalette) -> Box<dyn Scene> {
    let step = index as f32 / SPINNER_DOT_COUNT as f32 * TAU;
    let left = SPINNER_RADIUS_PX + SPINNER_RADIUS_PX * step.cos() - SPINNER_DOT_PX / 2.0;
    let top = SPINNER_RADIUS_PX + SPINNER_RADIUS_PX * step.sin() - SPINNER_DOT_PX / 2.0;
    Box::new(bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: rem(left),
            top: rem(top),
            width: rem(SPINNER_DOT_PX),
            height: rem(SPINNER_DOT_PX),
            border_radius: BorderRadius::MAX,
        }
        BackgroundColor({ palette.ink_dim })
        DoctorSpinnerDot(index)
    })
}

/// The ring's dots as one scene list: a repeated group, so it is authored as
/// a [`SceneList`] instead of an inline enumeration.
fn spinner_dots(palette: &UiPalette) -> impl SceneList + use<> {
    let dots: Vec<Box<dyn Scene>> = (0..SPINNER_DOT_COUNT)
        .map(|index| dot_scene(index, palette))
        .collect();
    bsn_list! { { dots } }
}

/// The indicator: a ring of dots that turns while the action is pending.
pub fn spinner_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            position_type: PositionType::Relative,
            width: rem(SPINNER_SIZE_PX),
            height: rem(SPINNER_SIZE_PX),
            flex_shrink: 0.0,
            display: Display::None,
        }
        DoctorSpinner
        Children [ { spinner_dots(palette) } ]
    }
}

/// Turn the ring while the action is pending.
///
/// The pending phase is the shared model's fact: the indicator shows itself
/// exactly while the owner still owes an answer, and hides itself the moment
/// the answer lands.
pub fn spin(
    model: Res<DoctorPageModelState>,
    time: Res<Time>,
    palette: Res<UiPalette>,
    mut spinners: Query<&mut Node, With<DoctorSpinner>>,
    mut dots: Query<(&DoctorSpinnerDot, &mut BackgroundColor)>,
) {
    let pending = matches!(&model.0.action().phase, DoctorActionPhase::Pending(_));
    for mut node in &mut spinners {
        let display = if pending {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    if !pending {
        return;
    }
    let active = (time.elapsed_secs() / SPINNER_STEP_SECONDS) as usize % SPINNER_DOT_COUNT;
    for (dot, mut fill) in &mut dots {
        let color = if dot.0 == active {
            palette.accent
        } else {
            palette.ink_dim
        };
        if fill.0 != color {
            fill.0 = color;
        }
    }
}
