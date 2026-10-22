//! Status badge: the small uppercase pill that announces a state
//! (EFFECTS-017 #63).
//!
//! The badge's height is its own token, not the row's: the label's height
//! is centred in the badge's own height, so the badge reads the same in a
//! short table row and in a tall one. The label is written in upper case,
//! which is the badge's voice.
//!
//! The badge announces a *state*, so its fill is the state's own ink as a
//! soft wash and its label is written in the state's own ink. The wash is
//! derived in the palette (the one place alpha derivations live) and the
//! label's ink is a theme token, so a skin switch repaints both.

use crate::feathers_theme::Token;
use crate::palette::UiPalette;
use crate::palette::rem;
use crate::text::{Role, TextRole};
use crate::theme::space;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::feathers::theme::ThemeTextColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, BackgroundColor, BorderRadius, JustifyContent, Node, UiRect};
use bevy::ui::widget::Text;

/// The badge's height in baseline pixels: the pill the label is centred in.
///
/// The height is a token of its own so the label's centring never depends
/// on the row the badge happens to sit in.
pub const STATUS_BADGE_HEIGHT_PX: f32 = 20.0;

/// The status a badge announces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatusBadgeTone {
    /// A neutral fact: neither good nor bad.
    #[default]
    Neutral,
    /// The action colour: something in flight or selected.
    Accent,
    /// A healthy fact.
    Success,
    /// A fact that wants attention.
    Warning,
    /// A failure.
    Danger,
}

/// How the badge writes its label.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatusBadgeCase {
    /// The label is written the way the copy gives it.
    #[default]
    AsWritten,
    /// The label is written in upper case: the badge's own voice.
    Upper,
}

/// Marker on a status badge.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusBadge;

/// Component wrapper for the status a badge announces.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusBadgeToneStyle(pub StatusBadgeTone);

/// The fill a status badge paints with: the status's own ink as a soft
/// wash over the surface the badge sits on. Pure function.
pub fn status_badge_fill(tone: StatusBadgeTone, palette: &UiPalette) -> Color {
    match tone {
        StatusBadgeTone::Neutral => palette.surface_elevated,
        StatusBadgeTone::Accent => palette.ink_wash(palette.accent),
        StatusBadgeTone::Success => palette.ink_wash(palette.success),
        StatusBadgeTone::Warning => palette.ink_wash(palette.warning),
        StatusBadgeTone::Danger => palette.ink_wash(palette.danger),
    }
}

/// The theme token a badge's label is written in. Pure function.
pub fn status_badge_ink_token(tone: StatusBadgeTone) -> Token {
    match tone {
        StatusBadgeTone::Neutral => Token::InkDim,
        StatusBadgeTone::Accent => Token::Accent,
        StatusBadgeTone::Success => Token::Success,
        StatusBadgeTone::Warning => Token::Warning,
        StatusBadgeTone::Danger => Token::Danger,
    }
}

/// A status badge: a small pill whose label announces a state.
///
/// The label is centred in the badge's own height and the badge is a
/// capsule, so the pill reads as a chip rather than a rectangle.
pub fn status_badge_scene(
    label: String,
    tone: StatusBadgeTone,
    case: StatusBadgeCase,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let text = match case {
        StatusBadgeCase::AsWritten => label,
        StatusBadgeCase::Upper => label.to_uppercase(),
    };
    let fill = status_badge_fill(tone, palette);
    let ink = status_badge_ink_token(tone);
    Box::new(bsn! {
        Node {
            height: rem(STATUS_BADGE_HEIGHT_PX),
            min_width: rem(STATUS_BADGE_HEIGHT_PX),
            padding: UiRect::horizontal(rem(space::S6)),
            border_radius: BorderRadius::MAX,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor({ fill })
        StatusBadge
        StatusBadgeToneStyle(tone)
        Children [
            Text({ text }) TextRole(Role::Caption) ThemeTextColor({ ink.theme_token() })
        ]
    })
}

/// Repaint every status badge from the live palette.
///
/// Compare-and-set: an unchanged frame costs nothing, and a skin switch
/// re-projects the wash without a tree rebuild.
pub fn sync_status_badge_visuals(
    palette: Res<UiPalette>,
    mut badges: Query<(&StatusBadgeToneStyle, &mut BackgroundColor), With<StatusBadge>>,
) {
    for (tone, mut fill) in &mut badges {
        let wanted = status_badge_fill(tone.0, &palette);
        if fill.0 != wanted {
            fill.0 = wanted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn every_tone_paints_its_own_ink_as_a_wash() {
        let palette = UiPalette::new(&Theme::dark());
        for (tone, ink) in [
            (StatusBadgeTone::Accent, palette.accent),
            (StatusBadgeTone::Success, palette.success),
            (StatusBadgeTone::Warning, palette.warning),
            (StatusBadgeTone::Danger, palette.danger),
        ] {
            let wash = status_badge_fill(tone, &palette).to_srgba();
            let source = ink.to_srgba();
            assert_eq!(wash.alpha, 0.16, "the wash is the ink at a low alpha");
            assert_eq!(
                (wash.red, wash.green, wash.blue),
                (source.red, source.green, source.blue)
            );
        }
    }

    #[test]
    fn the_neutral_badge_is_the_quiet_card_wash() {
        let palette = UiPalette::new(&Theme::dark());
        assert_eq!(
            status_badge_fill(StatusBadgeTone::Neutral, &palette),
            palette.surface_elevated
        );
    }

    #[test]
    fn every_tone_writes_its_label_in_its_own_token() {
        assert_eq!(
            status_badge_ink_token(StatusBadgeTone::Neutral),
            Token::InkDim
        );
        assert_eq!(
            status_badge_ink_token(StatusBadgeTone::Accent),
            Token::Accent
        );
        assert_eq!(
            status_badge_ink_token(StatusBadgeTone::Success),
            Token::Success
        );
        assert_eq!(
            status_badge_ink_token(StatusBadgeTone::Warning),
            Token::Warning
        );
        assert_eq!(
            status_badge_ink_token(StatusBadgeTone::Danger),
            Token::Danger
        );
    }
}
