//! The live theme-switch seam: retheme the whole mounted tree in place.
//!
//! A host triggers `ThemeSwitch(mode)` (e.g. from a settings control) and
//! the [`apply_theme`] observer re-resolves the [`UiPalette`] resource from
//! the theme tokens for that mode and restamps the two component families
//! that carry theme colors on already-mounted scenes — text roles and
//! control fills. The scene tree is never remounted: every entity keeps its
//! identity, only component values change (charter law: observers change
//! components, never rebuild trees).
//!
//! The switch carries a [`ThemeSkin`] (never a [`Theme`] value): callers can
//! only pick between the four token sets defined in [`crate::theme`], so an
//! off-token color has no path into the palette.
//!
//! # The toggle switch: the shared on/off control geometry
//!
//! One capsule track and one sliding knob back four product surfaces — the
//! preference toggles, the DNS core switches, the "activate after import"
//! toggle and the GEOIP fallback toggle. That geometry is one fact, so this
//! module owns it once: the baseline-pixel metrics, the knob's typed rest
//! position ([`SwitchPosition`]), the scene builders that paint the track
//! ([`switch_track_scene`]) and the knob ([`switch_knob_scene`]), and the
//! one control that mounts both ([`switch_control_scene`]).
//!
//! Callers keep their own markers, labels, colors and behavior: they compose
//! the shared control and add their page-specific components to the very same
//! nodes, so a page never re-declares the track or knob metrics and never
//! computes a knob offset — the surface flips the typed [`SwitchKnob`]
//! position and [`sync_switch_knobs`] re-projects the geometry.

use crate::button::{ControlVisual, control_fill};
use crate::fonts::FontSources;
use crate::palette::{UiPalette, rem};
use crate::text::{TextRole, role_typography};
use crate::theme::{Theme, ThemeSkin};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::picking::hover::PickingInteraction;
use bevy::scene::{Scene, bsn};
use bevy::text::{TextColor, TextFont};
use bevy::ui::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Node, PositionType, UiRect, Val,
};

/// Pick one of the token sets defined in [`crate::theme`]. Observe with
/// [`apply_theme`], registered by [`crate::WidgetsPlugin`].
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeSwitch(pub ThemeSkin);

/// Re-resolve the palette for the switched mode and restamp the mounted
/// tree in place. Text roles re-project size/face/ink; pill fills
/// re-project the token ladder (the idle fill a pill was spawned with is
/// stale the moment the palette swaps). Checkbox / radio / slider visuals
/// are repainted by their own per-frame sync systems, which compare against
/// the current palette every pass and self-heal without any switch hook.
pub fn apply_theme(
    switch: On<ThemeSwitch>,
    mut palette: ResMut<UiPalette>,
    fonts: Option<Res<FontSources>>,
    mut texts: Query<(&TextRole, &mut TextFont, &mut TextColor)>,
    mut pills: Query<(
        &ControlVisual,
        Option<&PickingInteraction>,
        &mut BackgroundColor,
    )>,
) {
    let theme = Theme::for_mode(switch.0);
    *palette = UiPalette::new(&theme);

    let sources = fonts.as_deref();
    for (role, mut font, mut ink) in &mut texts {
        let typography = role_typography(role.0, &palette, sources);
        font.font_size = typography.size;
        font.font = typography.font;
        ink.0 = typography.ink;
    }

    for (visual, interaction, mut fill) in &mut pills {
        let (hovered, pressed) = match interaction {
            Some(PickingInteraction::Hovered) => (true, false),
            Some(PickingInteraction::Pressed) => (true, true),
            _ => (false, false),
        };
        fill.0 = control_fill(visual.0, hovered, pressed, &palette);
    }
}

/// The switch track's fixed width in baseline pixels.
pub const SWITCH_TRACK_WIDTH_PX: f32 = 38.0;

/// The switch track's fixed height in baseline pixels.
pub const SWITCH_TRACK_HEIGHT_PX: f32 = 22.0;

/// The switch knob's square side in baseline pixels.
pub const SWITCH_KNOB_SIZE_PX: f32 = 16.0;

/// The knob's left offset inside the track while the switch is on.
pub const SWITCH_KNOB_ON_OFFSET_PX: f32 = 18.0;

/// The knob's left offset inside the track while the switch is off.
pub const SWITCH_KNOB_OFF_OFFSET_PX: f32 = 2.0;

/// Where the knob rests inside the track: the switch's single on/off fact.
///
/// The two offsets are one typed decision, so no page can rest the knob at a
/// third place or derive the offset from a second flag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwitchPosition {
    /// The switch is off: the knob rests at the track's left edge.
    #[default]
    Off,
    /// The switch is on: the knob rests at the track's right edge.
    On,
}

impl SwitchPosition {
    /// The knob's left offset inside the track, in baseline pixels.
    pub const fn knob_offset_px(self) -> f32 {
        match self {
            Self::Off => SWITCH_KNOB_OFF_OFFSET_PX,
            Self::On => SWITCH_KNOB_ON_OFFSET_PX,
        }
    }

    /// The rest position of a switch that is on (`true`) or off (`false`).
    pub const fn from_enabled(enabled: bool) -> Self {
        if enabled { Self::On } else { Self::Off }
    }
}

/// The switch track: the hairline-edged capsule the knob slides inside.
///
/// The geometry (38×22 baseline pixels, capsule radius, relative positioning)
/// belongs to this module; the caller owns the fill and the edge colors and
/// adds its own markers to the same node.
pub fn switch_track_scene(palette: &UiPalette, fill: Color, edge: Color) -> impl Scene + use<> {
    bsn! {
        Node {
            width: rem(SWITCH_TRACK_WIDTH_PX),
            height: rem(SWITCH_TRACK_HEIGHT_PX),
            // The hairline is a physical quantity: it never scales with the
            // root font size, so it stays on the px whitelist.
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::MAX,
            position_type: PositionType::Relative,
            align_items: AlignItems::Center,
        }
        BackgroundColor({ fill })
        BorderColor {
            top: { edge },
            right: { edge },
            bottom: { edge },
            left: { edge },
        }
    }
}

/// The switch knob: the disc that slides across the track.
///
/// The knob is a 16×16 baseline-pixel capsule whose absolute left offset is
/// the typed rest position, so the on/off geometry is decided once.
pub fn switch_knob_scene(knob: Color, position: SwitchPosition) -> impl Scene + use<> {
    let left = rem(position.knob_offset_px());
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: { left },
            width: rem(SWITCH_KNOB_SIZE_PX),
            height: rem(SWITCH_KNOB_SIZE_PX),
            border_radius: BorderRadius::MAX,
        }
        BackgroundColor({ knob })
    }
}

/// The colors one switch control paints with.
///
/// The geometry belongs to this module; a surface contributes only its
/// colors, so no page re-declares a size or an offset next to them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwitchColors {
    /// The track's fill.
    pub track_fill: Color,
    /// The track's hairline edge.
    pub track_edge: Color,
    /// The knob's fill.
    pub knob_fill: Color,
}

/// Marker on the shared switch control's track node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwitchTrack;

/// The knob's typed rest position: the shared control's own state.
///
/// A surface flips this value; the shared control owns the geometry and
/// re-projects the knob's offset from it ([`sync_switch_knobs`]). The knob's
/// position is therefore never re-derived by a page.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwitchKnob(pub SwitchPosition);

/// The shared switch control: the capsule track with its sliding knob.
///
/// One control owns the whole geometry — the track's size, its hairline edge,
/// the capsule radius and the knob's typed rest position. A surface
/// contributes its colors ([`SwitchColors`]) and its own markers; it never
/// re-declares a size and never computes an offset.
pub fn switch_control_scene(
    palette: &UiPalette,
    colors: SwitchColors,
    position: SwitchPosition,
) -> impl Scene + use<> {
    bsn! {
        @{ switch_track_scene(palette, colors.track_fill, colors.track_edge) }
        SwitchTrack
        Children [
            @{ switch_knob_scene(colors.knob_fill, position) }
            SwitchKnob({ position })
        ]
    }
}

/// Re-project every shared switch knob's rest offset from its typed position.
///
/// The knob's geometry is this module's fact: the surface only flips
/// [`SwitchKnob`]'s typed position, and this system moves the knob to the
/// shared offset. Compare-and-set: a knob already at its rest position is
/// left untouched.
pub fn sync_switch_knobs(mut knobs: Query<(&SwitchKnob, &mut Node)>) {
    for (knob, mut node) in &mut knobs {
        let left = rem(knob.0.knob_offset_px());
        if node.left != left {
            node.left = left;
        }
    }
}
