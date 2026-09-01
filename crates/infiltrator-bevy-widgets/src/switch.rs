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
//! The switch carries a [`LightDark`] mode, not a [`Theme`] value: callers
//! can only pick between the two token sets defined in [`crate::theme`], so
//! an off-token color has no path into the palette.

use bevy::ecs::event::Event;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::picking::hover::PickingInteraction;
use bevy::text::{TextColor, TextFont};
use bevy::ui::BackgroundColor;

use crate::button::{ControlVisual, control_fill};
use crate::fonts::FontSources;
use crate::palette::UiPalette;
use crate::text::{TextRole, role_typography};
use crate::theme::{LightDark, Theme};

/// Pick one of the token sets defined in [`crate::theme`]. Observe with
/// [`apply_theme`], registered by [`crate::WidgetsPlugin`].
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeSwitch(pub LightDark);

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
