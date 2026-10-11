//! The official two-stage theme bridge: [`Token`] → [`SemanticToken`] → color.
//!
//! [`UiPalette`] remains the single token authority; this module is the one
//! place that projects it onto bevy_feathers' [`UiTheme`] so the official
//! `update_theme` machinery paints `ThemeBackgroundColor`, `ThemeBorderColor`
//! and `ThemeTextColor` sites, and so overlays can
//! declare a [`ThemeContext`] surface level. Overwriting the [`UiTheme`]
//! resource is the sanctioned theme switch; see
//! `crates/infiltrator-bevy-ui/docs/BEVY_020_UI_CAPABILITIES.md` ②.

use crate::palette::UiPalette;
use bevy::color::Color;
use bevy::ecs::change_detection::DetectChanges;
use bevy::feathers::theme::{SemanticToken, SurfaceLevel, ThemeProps, ThemeToken, UiTheme};
use bevy::platform::collections::HashMap;

/// One design token of this widget layer, as stamped by the `Theme*` paint
/// components. The variant names mirror [`UiPalette`] fields one-to-one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    WindowClear,
    Surface,
    SurfaceElevated,
    Ink,
    InkDim,
    Accent,
    OnAccent,
    AccentContainer,
    Sidebar,
    IconTile,
    HoverBg,
    PressedBg,
    Scrim,
    FocusRing,
    DisabledInk,
    Border,
    Success,
    Warning,
    Danger,
    /// Palette-derived washes: the ONLY alpha derivations the palette layer
    /// performs, re-expressed as tokens so overlays can paint them too.
    SelectionFill,
    ChartFillUp,
    ChartFillDown,
}

/// The bridge from our token vocabulary to the official token type. `bsn!`
/// sites stamp `ThemeBackgroundColor(Token::Surface)` etc. and rely on this
/// conversion; systems use the explicit [`Token::theme_token`].
impl From<Token> for ThemeToken {
    fn from(token: Token) -> Self {
        token.theme_token()
    }
}

impl Token {
    /// The design-token name this variant is stamped with (`mf.*` namespace).
    pub fn theme_token(self) -> ThemeToken {
        ThemeToken::new_static(self.name())
    }

    /// The semantic token the design token assigns to. One semantic token per
    /// variant keeps the mapping auditable; contextual overrides happen at the
    /// semantic level in [`build_ui_theme`].
    pub fn semantic_token(self) -> SemanticToken {
        SemanticToken::new_static(self.name())
    }

    fn name(self) -> &'static str {
        match self {
            Token::WindowClear => "mf.window_clear",
            Token::Surface => "mf.surface",
            Token::SurfaceElevated => "mf.surface_elevated",
            Token::Ink => "mf.ink",
            Token::InkDim => "mf.ink_dim",
            Token::Accent => "mf.accent",
            Token::OnAccent => "mf.on_accent",
            Token::AccentContainer => "mf.accent_container",
            Token::Sidebar => "mf.sidebar",
            Token::IconTile => "mf.icon_tile",
            Token::HoverBg => "mf.hover_bg",
            Token::PressedBg => "mf.pressed_bg",
            Token::Scrim => "mf.scrim",
            Token::FocusRing => "mf.focus_ring",
            Token::DisabledInk => "mf.disabled_ink",
            Token::Border => "mf.border",
            Token::Success => "mf.success",
            Token::Warning => "mf.warning",
            Token::Danger => "mf.danger",
            Token::SelectionFill => "mf.selection_fill",
            Token::ChartFillUp => "mf.chart_fill_up",
            Token::ChartFillDown => "mf.chart_fill_down",
        }
    }

    /// The color a token resolves to on the base surface, from the palette.
    fn base_color(self, palette: &UiPalette) -> Color {
        match self {
            Token::WindowClear => palette.window_clear,
            Token::Surface => palette.surface,
            Token::SurfaceElevated => palette.surface_elevated,
            Token::Ink => palette.ink,
            Token::InkDim => palette.ink_dim,
            Token::Accent => palette.accent,
            Token::OnAccent => palette.on_accent,
            Token::AccentContainer => palette.accent_container,
            Token::Sidebar => palette.sidebar,
            Token::IconTile => palette.icon_tile,
            Token::HoverBg => palette.hover_bg,
            Token::PressedBg => palette.pressed_bg,
            Token::Scrim => palette.scrim,
            Token::FocusRing => palette.focus_ring,
            Token::DisabledInk => palette.disabled_ink,
            Token::Border => palette.border,
            Token::Success => palette.success,
            Token::Warning => palette.warning,
            Token::Danger => palette.danger,
            Token::SelectionFill => palette.selection_fill(),
            Token::ChartFillUp => palette.chart_fill_up(),
            Token::ChartFillDown => palette.chart_fill_down(),
        }
    }
}

/// All palette-backed tokens, in palette field order (drives the builder and
/// the headless round-trip test).
pub const ALL_TOKENS: [Token; 22] = [
    Token::WindowClear,
    Token::Surface,
    Token::SurfaceElevated,
    Token::Ink,
    Token::InkDim,
    Token::Accent,
    Token::OnAccent,
    Token::AccentContainer,
    Token::Sidebar,
    Token::IconTile,
    Token::HoverBg,
    Token::PressedBg,
    Token::Scrim,
    Token::FocusRing,
    Token::DisabledInk,
    Token::Border,
    Token::Success,
    Token::Warning,
    Token::Danger,
    Token::SelectionFill,
    Token::ChartFillUp,
    Token::ChartFillDown,
];

/// Build the feathers [`UiTheme`] from the resolved palette. Every token gets
/// a one-stage assignment plus a base color; floating surfaces (menus, modals,
/// toasts, popovers) lift surface-class tokens so overlays visibly sit above
/// the base, the feathers [`SurfaceLevel`] mechanism applied to our ladder.
pub fn build_ui_theme(palette: &UiPalette) -> UiTheme {
    let mut token_assignments = HashMap::default();
    let mut semantic_base = HashMap::default();
    let mut semantic_overrides: HashMap<SurfaceLevel, HashMap<SemanticToken, Color>> =
        HashMap::default();

    for token in ALL_TOKENS {
        token_assignments.insert(token.theme_token(), token.semantic_token());
        semantic_base.insert(token.semantic_token(), token.base_color(palette));
    }

    // Contextual level: surface-class tokens lift one rung per level using the
    // palette's own elevation vocabulary (surface → surface_elevated →
    // icon_tile rung); ink-class tokens stay shared across levels.
    let mut floating = HashMap::default();
    floating.insert(Token::Surface.semantic_token(), palette.surface_elevated);
    floating.insert(Token::SurfaceElevated.semantic_token(), palette.icon_tile);
    let mut higher = HashMap::default();
    higher.insert(Token::Surface.semantic_token(), palette.surface_elevated);
    let mut highest = HashMap::default();
    highest.insert(Token::Surface.semantic_token(), palette.icon_tile);

    semantic_overrides.insert(SurfaceLevel::Higher, higher);
    semantic_overrides.insert(SurfaceLevel::Highest, highest);
    semantic_overrides.insert(SurfaceLevel::Floating, floating);

    UiTheme(ThemeProps {
        token_assignments,
        semantic_base,
        semantic_overrides,
    })
}

/// Rebuild the [`UiTheme`] whenever the palette resource changes (skin
/// switch, tonal ladder refresh). Overwriting the resource is the official
/// repaint trigger.
pub fn sync_ui_theme(palette: Res<UiPalette>, mut theme: ResMut<UiTheme>) {
    if palette.is_changed() {
        *theme = build_ui_theme(&palette);
    }
}

use bevy::ecs::system::{Res, ResMut};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use bevy::feathers::theme::ThemeBackgroundColor;

    #[test]
    fn every_token_assigns_to_itself_and_resolves_base_color() {
        let palette = UiPalette::new(&Theme::dark());
        let theme = build_ui_theme(&palette);
        for token in ALL_TOKENS {
            assert_eq!(
                theme.0.token_assignments.get(&token.theme_token()),
                Some(&token.semantic_token()),
                "{token:?} must be assigned"
            );
        }
    }

    #[test]
    fn context_color_matches_palette_base_and_floating_lift() {
        let palette = UiPalette::new(&Theme::dark());
        let theme = build_ui_theme(&palette);
        assert_eq!(theme.color(&Token::Accent.theme_token()), palette.accent);
        assert_eq!(
            theme.context_color(&Token::Surface.theme_token(), SurfaceLevel::Base),
            palette.surface
        );
        assert_eq!(
            theme.context_color(&Token::Surface.theme_token(), SurfaceLevel::Floating),
            palette.surface_elevated,
            "floating overlays lift surface to the elevated rung"
        );
    }

    #[test]
    fn theme_background_component_stamps_the_token() {
        let bg = ThemeBackgroundColor(Token::Accent.theme_token());
        assert_eq!(bg.0, ThemeToken::new_static("mf.accent"));
    }
}
