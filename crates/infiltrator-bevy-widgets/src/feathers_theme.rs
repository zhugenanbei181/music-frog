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
use bevy::color::{Alpha, Color, Luminance};
use bevy::ecs::change_detection::DetectChanges;
use bevy::feathers::dark_theme::create_dark_theme;
use bevy::feathers::theme::ThemeToken;
use bevy::feathers::theme::{SemanticToken, SurfaceLevel, ThemeProps, UiTheme};
use bevy::feathers::tokens::semantic;
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
    /// The wash under a **selected navigation row**. Separate from
    /// [`Self::SelectionFill`] because the two state different facts: that one
    /// is a chosen member (the accent), this one is where the user is (a quiet
    /// ink wash beside a narrow accent indicator).
    NavSelectionFill,
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
            Token::NavSelectionFill => "mf.nav_selection_fill",
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
            Token::NavSelectionFill => palette.nav_selection_fill(),
            Token::ChartFillUp => palette.chart_fill_up(),
            Token::ChartFillDown => palette.chart_fill_down(),
        }
    }
}

/// All palette-backed tokens, in palette field order (drives the builder and
/// the headless round-trip test).
pub const ALL_TOKENS: [Token; 23] = [
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
    Token::NavSelectionFill,
    Token::ChartFillUp,
    Token::ChartFillDown,
];

/// Build the feathers [`UiTheme`] from the resolved palette. Every token gets
/// a one-stage assignment plus a base color; floating surfaces (menus, modals,
/// toasts, popovers) lift surface-class tokens so overlays visibly sit above
/// the base, the feathers [`SurfaceLevel`] mechanism applied to our ladder.
pub fn build_ui_theme(palette: &UiPalette) -> UiTheme {
    // The official themed controls (`bevy_feathers::controls`) look up their
    // own `feathers.*` design tokens. Their token→semantic assignment is a
    // fact owned upstream, so start from the official theme rather than
    // restating 122 pairs here. Only the resolved colors are ours.
    let ThemeProps {
        mut token_assignments,
        semantic_overrides: mut official_overrides,
        ..
    } = create_dark_theme();
    let mut semantic_base = HashMap::default();
    let mut semantic_overrides: HashMap<SurfaceLevel, HashMap<SemanticToken, Color>> =
        HashMap::default();

    for token in ALL_TOKENS {
        token_assignments.insert(token.theme_token(), token.semantic_token());
        semantic_base.insert(token.semantic_token(), token.base_color(palette));
    }

    // Product palette is the single color authority: every semantic role the
    // official controls read resolves from the same `UiPalette` fields as the
    // `mf.*` tokens. Hover/pressed/disabled are derived luminance/alpha
    // variants of those same roles, never a second hard-coded palette.
    let accent_disabled = palette.disabled_ink.with_alpha(0.7);
    let solid_disabled = palette.surface_elevated.with_alpha(0.7);
    let text_disabled = palette.ink.with_alpha(0.4);
    let fill_none = Color::NONE;
    semantic_base.extend([
        (semantic::SURFACE_WINDOW, palette.window_clear),
        (semantic::SURFACE_PANE_BODY, palette.surface),
        (semantic::SURFACE_PANE_HEADER, palette.window_clear),
        (semantic::SURFACE_SUBPANE_BODY, palette.surface),
        (semantic::SURFACE_SUBPANE_HEADER, palette.surface_elevated),
        (semantic::SURFACE_GROUP, palette.surface_elevated),
        (semantic::SURFACE_DIALOG, palette.surface),
        (semantic::SURFACE_MENU, palette.surface_elevated),
        (semantic::BORDER_DEFAULT, palette.border),
        (semantic::TEXT_DEFAULT, palette.ink),
        (semantic::TEXT_DIM, palette.ink_dim),
        (semantic::TEXT_ON_ACCENT, palette.on_accent),
        (semantic::TEXT_DISABLED, text_disabled),
        (semantic::FILL_ACCENT_DEFAULT, palette.accent),
        (semantic::FILL_ACCENT_HOVER, palette.accent.lighter(0.05)),
        (semantic::FILL_ACCENT_PRESSED, palette.accent.lighter(0.1)),
        (semantic::FILL_ACCENT_DISABLED, accent_disabled),
        (semantic::FILL_SOLID_DEFAULT, palette.surface_elevated),
        (semantic::FILL_SOLID_HOVER, palette.hover_bg),
        (semantic::FILL_SOLID_PRESSED, palette.pressed_bg),
        (semantic::FILL_SOLID_DISABLED, solid_disabled),
        (semantic::FILL_ITEM_DEFAULT, fill_none),
        (semantic::FILL_ITEM_HOVER, palette.hover_bg),
        (semantic::FILL_ITEM_PRESSED, palette.pressed_bg),
        (semantic::FILL_FIELD_DEFAULT, palette.surface_elevated),
        (semantic::FILL_FIELD_HOVER, palette.hover_bg),
        (semantic::FILL_FIELD_PRESSED, palette.pressed_bg),
        (semantic::FILL_NONE, fill_none),
        (semantic::AXIS_X, palette.danger),
        (semantic::AXIS_Y, palette.success),
        (semantic::AXIS_Z, palette.accent),
        (semantic::AXIS_W, palette.warning),
        (semantic::FOCUS_RING, palette.focus_ring),
    ]);

    // Keep the official contextual lifts, but re-resolve their colors from
    // our palette so the four SurfaceLevel rungs stay single-sourced: the
    // recessed field and solid control fills climb to `icon_tile`, while
    // item fills stay transparent-ish washes from `hover_bg`/`pressed_bg`.
    for (level, overrides) in official_overrides.drain() {
        let mut resolved = HashMap::default();
        for (semantic_token, _) in overrides {
            let color = themed_lift(&semantic_token, &semantic_base, level, palette);
            resolved.insert(semantic_token, color);
        }
        semantic_overrides.insert(level, resolved);
    }

    // Contextual level: surface-class tokens lift one rung per level using the
    // palette's own elevation vocabulary (surface → surface_elevated →
    // icon_tile rung); ink-class tokens stay shared across levels.
    // Product `mf.*` tokens share the same contextual ladder. Insert into the
    // existing per-level maps so neither vocabulary displaces the other.
    semantic_overrides
        .entry(SurfaceLevel::Higher)
        .or_default()
        .insert(Token::Surface.semantic_token(), palette.surface_elevated);
    semantic_overrides
        .entry(SurfaceLevel::Highest)
        .or_default()
        .insert(Token::Surface.semantic_token(), palette.icon_tile);
    semantic_overrides
        .entry(SurfaceLevel::Floating)
        .or_default()
        .insert(Token::Surface.semantic_token(), palette.surface_elevated);
    semantic_overrides
        .entry(SurfaceLevel::Floating)
        .or_default()
        .insert(Token::SurfaceElevated.semantic_token(), palette.icon_tile);

    UiTheme(ThemeProps {
        token_assignments,
        semantic_base,
        semantic_overrides,
    })
}

/// Resolve an official contextual override to the palette's elevation rung.
/// Falling back to the base color is deliberate: no second color vocabulary.
fn themed_lift(
    semantic_token: &SemanticToken,
    semantic_base: &HashMap<SemanticToken, Color>,
    level: SurfaceLevel,
    palette: &UiPalette,
) -> Color {
    let lifted = matches!(level, SurfaceLevel::Highest | SurfaceLevel::Floating);
    if lifted
        && (*semantic_token == semantic::FILL_FIELD_DEFAULT
            || *semantic_token == semantic::FILL_SOLID_DEFAULT)
    {
        return palette.icon_tile;
    }
    semantic_base
        .get(semantic_token)
        .copied()
        .unwrap_or(palette.surface_elevated)
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
    use bevy::feathers::dark_theme::create_dark_theme;
    use bevy::feathers::theme::ThemeBackgroundColor;
    use bevy::feathers::tokens;

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

    #[test]
    fn official_control_tokens_resolve_from_the_product_palette() {
        let palette = UiPalette::new(&Theme::dark());
        let theme = build_ui_theme(&palette);
        let official = create_dark_theme();
        assert_eq!(
            theme.0.token_assignments.len(),
            official.token_assignments.len() + ALL_TOKENS.len(),
            "every official token assignment survives the product merge"
        );
        for (token, semantic_token) in &official.token_assignments {
            assert_eq!(
                theme.0.token_assignments.get(token),
                Some(semantic_token),
                "official token {token:?} keeps its upstream semantic role"
            );
        }
        assert_eq!(theme.color(&tokens::WINDOW_BG), palette.window_clear);
        assert_eq!(theme.color(&tokens::BUTTON_PRIMARY_BG), palette.accent);
        assert_eq!(theme.color(&tokens::TEXT_MAIN), palette.ink);
        assert_eq!(theme.color(&tokens::FOCUS_RING), palette.focus_ring);
        assert_eq!(
            theme.context_color(&tokens::BUTTON_BG, SurfaceLevel::Floating),
            palette.icon_tile,
            "floating contexts still lift the official solid fill"
        );
    }

    #[test]
    fn official_switch_and_scrollbar_tokens_share_the_product_semantics() {
        let palette = UiPalette::new(&Theme::dark());
        let theme = build_ui_theme(&palette);
        let official = create_dark_theme();
        let resolve = |token: &ThemeToken| {
            let semantic_token = official
                .token_assignments
                .get(token)
                .expect("official token is assigned");
            theme
                .0
                .semantic_base
                .get(semantic_token)
                .copied()
                .expect("product theme resolves the semantic role")
        };
        assert_eq!(resolve(&tokens::SWITCH_BG_CHECKED), palette.accent);
        assert_eq!(resolve(&tokens::SWITCH_BG), palette.surface_elevated);
        assert_eq!(resolve(&tokens::SCROLLBAR_THUMB), palette.accent);
        assert_eq!(resolve(&tokens::CHECKBOX_MARK), palette.on_accent);
        assert_eq!(resolve(&tokens::MENUITEM_BG), Color::NONE);
    }
}
