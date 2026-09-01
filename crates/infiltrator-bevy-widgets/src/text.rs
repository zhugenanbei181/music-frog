//! Typographic roles: the one seam between scene text and theme typography.
//!
//! Scenes emit `TextRole(Role::…)` next to a `Text` node and never touch
//! font sizes, faces or ink literals; [`role_typography`] is the pure
//! projection from role to (size, face, ink), and the [`style_text_roles`]
//! observer stamps it the moment the role lands. This is the
//! taskmanager-proven observer idiom: runtime changes never rebuild the
//! tree, they restamp components in place. The same projection re-stamps
//! every role on a theme switch — see [`crate::switch`].

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::lifecycle::Add;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Query, Res};
use bevy::text::{FontSize, FontSource, TextColor, TextFont};

use crate::fonts::FontSources;
use crate::palette::UiPalette;

/// Which typographic role a text node plays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    /// One rung above [`Role::Heading`]: the status banner's state word
    /// (the iced reference draws it ~22px, larger than a panel title).
    /// Display size, full ink, SemiBold face.
    Display,
    /// Page/panel titles: heading size, full ink, SemiBold face.
    Heading,
    /// Primary reading text and control labels: body size, full ink,
    /// Regular face.
    #[default]
    Body,
    /// Emphasized reading text (identity titles, stop-button copy): body
    /// size, full ink, SemiBold face.
    BodyStrong,
    /// Captions and idle labels: caption size, dim ink, Medium face.
    Caption,
    /// Aligned telemetry values: mono size, full ink, JetBrains Mono face.
    Mono,
}

/// Marker stamped onto text nodes inside `bsn!` trees. The `Default` seed
/// only exists for the bsn! template mechanism; spawned values always carry
/// an explicit role.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextRole(pub Role);

/// The full typography one role resolves to: scale size, face source and
/// ink. Pure data — headless-testable without any app.
pub struct Typography {
    pub size: FontSize,
    pub font: FontSource,
    pub ink: Color,
}

/// Project one role onto the palette and the embedded faces. With no
/// [`FontSources`] in scope the face falls back to the default
/// [`FontSource::Handle`] — the cosmic-text system fallback — so a host
/// that skips font embedding degrades instead of panicking.
pub fn role_typography(role: Role, palette: &UiPalette, fonts: Option<&FontSources>) -> Typography {
    let (size, ink) = match role {
        Role::Display => (palette.display_font_px, palette.ink),
        Role::Heading => (palette.heading_font_px, palette.ink),
        Role::Body | Role::BodyStrong => (palette.body_font_px, palette.ink),
        Role::Caption => (palette.caption_font_px, palette.ink_dim),
        Role::Mono => (palette.mono_font_px, palette.ink),
    };
    Typography {
        size: FontSize::Px(size),
        font: fonts
            .map(|sources| FontSource::Handle(sources.face(role)))
            .unwrap_or_default(),
        ink,
    }
}

/// Stamp palette metrics, the embedded face and ink onto every text node as
/// its role lands. Runs for the shell, every remount and every future
/// widget insert — the single place typography becomes bevy values.
pub fn style_text_roles(
    trigger: On<Add, TextRole>,
    mut texts: Query<(&TextRole, &mut TextFont, &mut TextColor)>,
    palette: Res<UiPalette>,
    fonts: Option<Res<FontSources>>,
) {
    let Ok((role, mut metrics, mut ink)) = texts.get_mut(trigger.event().entity) else {
        return;
    };
    let typography = role_typography(role.0, &palette, fonts.as_deref());
    metrics.font_size = typography.size;
    metrics.font = typography.font;
    ink.0 = typography.ink;
}
