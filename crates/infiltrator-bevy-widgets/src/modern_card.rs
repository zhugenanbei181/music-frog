//! ModernCard: the shared modern dark card base (EFFECTS-014 #51~#52).
//!
//! **Pure Core**: [`ModernCardState`] and [`modern_card_paint`] — one typed
//! enum (the pointer fact and the owner's marked fact never collapse into a
//! bag of booleans) resolves one exhaustive paint ladder: a hairline inner
//! stroke and a Level 1 soft shadow at rest, a deeper shadow plus a -1px
//! micro-lift and a highlight wash under the pointer. Pure data, so the
//! ladder is headless-testable without an app.
//!
//! **Scene Adapter**: [`modern_card_scene`] builds the declarative card with
//! the token chrome; [`sync_modern_card_paint`] repaints every mounted card
//! in place (compare-before-write, no rebuild flicker). The lift rides
//! [`UiTransform`], so hovering never triggers a relayout.

use crate::button::ButtonDisabled;
use crate::palette::UiPalette;
use crate::palette::rem;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::picking::hover::PickingInteraction;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    BackgroundColor, BorderColor, BorderRadius, FlexDirection, Node, UiRect, Val, percent,
};
use bevy::ui::{BoxShadow, UiTransform, Val2};

/// The hover micro-lift: how far the card rises under the pointer (EFFECTS-014 #52).
pub const MODERN_CARD_HOVER_LIFT_PX: f32 = 1.0;

/// The resting Level 1 shadow's blur radius in baseline pixels (EFFECTS-014 #51).
const RESTING_SHADOW_BLUR_PX: f32 = 6.0;

/// The resting Level 1 shadow's vertical offset in baseline pixels.
const RESTING_SHADOW_OFFSET_PX: f32 = 2.0;

/// The hovered shadow's blur radius in baseline pixels.
const HOVER_SHADOW_BLUR_PX: f32 = 10.0;

/// The hovered shadow's vertical offset in baseline pixels.
const HOVER_SHADOW_OFFSET_PX: f32 = 3.0;

/// Marker of one shared modern card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModernCard;

/// The owner fact a card carries: this card is the marked one of its group
/// (the group's current outbound, the active entry, …). A real binary fact.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModernCardMarked(pub bool);

/// The card's painted state.
///
/// One typed enum, one exhaustive resolver: the owner fact (marked) and the
/// pointer fact (hover / press) never collapse into a bag of booleans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModernCardState {
    /// Resting, and not marked.
    Resting,
    /// Resting, with the pointer over the card.
    Hovered,
    /// The pointer is pressing the card.
    Pressed,
    /// The marked card of its group.
    Marked,
    /// The marked card, with the pointer over it.
    MarkedHovered,
    /// The marked card, with the pointer pressing it.
    MarkedPressed,
}

/// Resolve the card's painted state from the two authoritative facts.
pub fn resolve_modern_card_state(
    marked: bool,
    interaction: Option<PickingInteraction>,
) -> ModernCardState {
    match (marked, interaction) {
        (false, Some(PickingInteraction::Pressed)) => ModernCardState::Pressed,
        (false, Some(PickingInteraction::Hovered)) => ModernCardState::Hovered,
        (false, _) => ModernCardState::Resting,
        (true, Some(PickingInteraction::Pressed)) => ModernCardState::MarkedPressed,
        (true, Some(PickingInteraction::Hovered)) => ModernCardState::MarkedHovered,
        (true, _) => ModernCardState::Marked,
    }
}

/// The paint one card state resolves to. Pure data, so the ladder is
/// headless-testable without an app.
pub struct ModernCardPaint {
    /// The card's own surface.
    pub fill: Color,
    /// The card's 1px inner stroke.
    pub border: Color,
    /// The card's drop shadow (never `None`: the base carries a Level 1
    /// shadow, the hovered states a deeper one).
    pub shadow: BoxShadow,
    /// The card's hover micro-lift, if the state carries one.
    pub lift: Option<Val2>,
}

/// Resolve one card state into its paint.
///
/// The marked card keeps its reading surface: the accent rides the stroke,
/// never the whole card. The highlight under the pointer is a wash of the
/// palette's hover colour composited over the card surface in linear space,
/// not a second surface colour a page may guess.
pub fn modern_card_paint(state: ModernCardState, palette: &UiPalette) -> ModernCardPaint {
    let resting_shadow = BoxShadow::new(
        palette.border,
        Val::ZERO,
        rem(RESTING_SHADOW_OFFSET_PX),
        Val::ZERO,
        rem(RESTING_SHADOW_BLUR_PX),
    );
    let hovered_shadow = BoxShadow::new(
        palette.border,
        Val::ZERO,
        rem(HOVER_SHADOW_OFFSET_PX),
        Val::ZERO,
        rem(HOVER_SHADOW_BLUR_PX),
    );
    let lift = Val2::new(Val::ZERO, rem(-MODERN_CARD_HOVER_LIFT_PX));
    match state {
        ModernCardState::Resting => ModernCardPaint {
            fill: palette.surface_elevated,
            border: palette.border,
            shadow: resting_shadow,
            lift: None,
        },
        ModernCardState::Hovered => ModernCardPaint {
            fill: palette.over_surface(palette.hover_bg),
            border: palette.border,
            shadow: hovered_shadow,
            lift: Some(lift),
        },
        ModernCardState::Pressed => ModernCardPaint {
            fill: palette.pressed_bg,
            border: palette.border,
            shadow: resting_shadow,
            lift: Some(lift),
        },
        ModernCardState::Marked => ModernCardPaint {
            fill: palette.surface_elevated,
            border: palette.accent,
            shadow: resting_shadow,
            lift: None,
        },
        ModernCardState::MarkedHovered => ModernCardPaint {
            fill: palette.over_surface(palette.hover_bg),
            border: palette.accent,
            shadow: hovered_shadow,
            lift: Some(lift),
        },
        ModernCardState::MarkedPressed => ModernCardPaint {
            fill: palette.pressed_bg,
            border: palette.accent,
            shadow: resting_shadow,
            lift: Some(lift),
        },
    }
}

/// Build one shared modern card: the token chrome (hairline inner stroke,
/// card radius, elevated surface) around the caller's content.
pub fn modern_card_scene(content: Box<dyn Scene>, palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;

    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(rem(palette.card_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        ModernCard
        ModernCardMarked(false)
        UiTransform
        Children [
            @{ content }
        ]
    }
}

/// Paint every mounted modern card from the owner's marked bit, the disabled
/// fact and the pointer's interaction.
///
/// The card is the page's own surface, so this system owns the fill, the
/// stroke, the shadow and the lift; every write is compare-before-write so a
/// settled card costs nothing and never flickers.
type ModernCardQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ModernCardMarked,
        Option<&'static ButtonDisabled>,
        Option<&'static PickingInteraction>,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
        Option<&'static mut BoxShadow>,
        &'static mut UiTransform,
    ),
    With<ModernCard>,
>;

pub fn sync_modern_card_paint(
    palette: Res<UiPalette>,
    mut commands: Commands,
    mut cards: ModernCardQuery,
) {
    for (entity, marked, disabled, interaction, mut fill, mut border, mut shadow, mut transform) in
        &mut cards
    {
        if disabled.is_some_and(|disabled| disabled.0) {
            continue;
        }
        let paint = modern_card_paint(
            resolve_modern_card_state(marked.0, interaction.copied()),
            &palette,
        );
        if fill.0 != paint.fill {
            fill.0 = paint.fill;
        }
        if border.top != paint.border {
            border.set_all(paint.border);
        }
        let shadow_styles = paint.shadow.0;
        match shadow.as_deref_mut() {
            Some(shadow) => {
                if shadow.0 != shadow_styles {
                    shadow.0 = shadow_styles;
                }
            }
            None => {
                if !shadow_styles.is_empty() {
                    commands.entity(entity).insert(BoxShadow(shadow_styles));
                }
            }
        }
        let translation = paint.lift.unwrap_or(Val2::ZERO);
        if transform.translation != translation {
            transform.translation = translation;
        }
    }
}
