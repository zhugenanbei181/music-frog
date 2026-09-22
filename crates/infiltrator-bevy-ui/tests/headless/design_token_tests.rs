//! DUAL-15-14 design-token mirror tests: the business-agnostic Bevy widget
//! layer must paint exactly the numbers the shared contract carries (the Iced
//! shell consumes the same contract values at compile time). A drifted mirror
//! is a real parity defect — the two surfaces would look different — so every
//! core palette channel and every structural token is asserted here.

use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin, metrics, radius, space};
use infiltrator_contract::design_tokens::{
    RgbaToken, SkinCorePalette, SkinInteractionPalette, skin_core, skin_interaction,
};
use infiltrator_contract::theme;

fn mirrored_skin(skin: theme::ThemeSkin) -> ThemeSkin {
    match skin {
        theme::ThemeSkin::Dark => ThemeSkin::Dark,
        theme::ThemeSkin::Light => ThemeSkin::Light,
        theme::ThemeSkin::Forest => ThemeSkin::Forest,
        theme::ThemeSkin::Amoled => ThemeSkin::Amoled,
    }
}

fn assert_token(
    token: infiltrator_bevy_widgets::theme::TokenColor,
    expected: RgbaToken,
    field: &str,
) {
    assert_eq!(
        (token.r, token.g, token.b, token.a),
        (expected.r, expected.g, expected.b, expected.a),
        "the widget mirror of `{field}` drifted from the shared contract"
    );
}

fn assert_core_palette(skin: theme::ThemeSkin) {
    let core: SkinCorePalette = skin_core(skin);
    let resolved = Theme::for_mode(mirrored_skin(skin));
    assert_eq!(resolved.mode.as_setting(), skin.as_setting());
    assert_token(resolved.window_bg, core.canvas, "canvas");
    assert_token(resolved.sidebar, core.sidebar, "sidebar");
    assert_token(resolved.surface, core.card, "card");
    assert_token(resolved.border, core.card_border, "card_border");
    assert_token(resolved.surface_elevated, core.control_bg, "control_bg");
    assert_token(resolved.ink, core.ink, "ink");
    assert_token(resolved.ink_dim, core.ink_dim, "ink_dim");
    assert_token(resolved.accent, core.accent, "accent");
    assert_token(resolved.on_accent, core.on_accent, "on_accent");
    assert_token(resolved.success, core.success, "success");
    assert_token(resolved.warning, core.warning, "warning");
    assert_token(resolved.danger, core.danger, "danger");
}

#[test]
fn the_widget_palette_mirrors_the_shared_design_tokens() {
    for skin in theme::ThemeSkin::ALL {
        assert_core_palette(skin);
    }
}

/// DUAL-15-14: the interaction/overlay tokens (scrim, hover, pressed, focus
/// ring, disabled ink) are part of the shared claim, so the widget mirror is
/// asserted channel-exactly and the palette must map every one of them from
/// the theme tokens (no re-derivation, no raw gray scrim).
fn assert_interaction_palette(skin: theme::ThemeSkin) {
    let interaction: SkinInteractionPalette = skin_interaction(skin);
    let resolved = Theme::for_mode(mirrored_skin(skin));
    assert_token(resolved.scrim, interaction.scrim, "scrim");
    assert_token(resolved.hover, interaction.hover, "hover");
    assert_token(resolved.pressed, interaction.pressed, "pressed");
    assert_token(resolved.focus_ring, interaction.focus_ring, "focus_ring");
    assert_token(
        resolved.disabled_ink,
        interaction.disabled_ink,
        "disabled_ink",
    );

    let palette = infiltrator_bevy_widgets::palette::UiPalette::new(&resolved);
    assert_eq!(
        palette.scrim,
        infiltrator_bevy_widgets::palette::theme_color(resolved.scrim),
        "the palette must read the scrim token instead of deriving one"
    );
    assert_eq!(
        palette.focus_ring,
        infiltrator_bevy_widgets::palette::theme_color(resolved.focus_ring)
    );
    assert_eq!(
        palette.disabled_ink,
        infiltrator_bevy_widgets::palette::theme_color(resolved.disabled_ink)
    );
    assert_eq!(
        palette.hover_bg,
        infiltrator_bevy_widgets::palette::theme_color(resolved.hover)
    );
    assert_eq!(
        palette.pressed_bg,
        infiltrator_bevy_widgets::palette::theme_color(resolved.pressed)
    );
}

#[test]
fn the_widget_interaction_tokens_mirror_the_shared_contract() {
    for skin in theme::ThemeSkin::ALL {
        assert_interaction_palette(skin);
    }
}

/// The overlay scrim is a dark translucent wash on every skin — the alignment
/// deliberately replaced the old "window token at half strength" derivation,
/// which lightened the backdrop on the light skin instead of dimming it.
#[test]
fn the_mirrored_scrim_dimms_the_backdrop_on_every_skin() {
    for skin in theme::ThemeSkin::ALL {
        let resolved = Theme::for_mode(mirrored_skin(skin));
        assert!(
            resolved.scrim.r < 0.2 && resolved.scrim.g < 0.7 && resolved.scrim.b < 0.7,
            "{skin:?} scrim must stay a dark wash, found {resolved:?}"
        );
        assert!(
            resolved.scrim.a > 0.0 && resolved.scrim.a < 1.0,
            "{skin:?} scrim must be translucent"
        );
        assert!(
            resolved.pressed.a > resolved.hover.a,
            "{skin:?} pressed wash must read stronger than hover"
        );
    }
}

#[test]
fn the_widget_ladders_mirror_the_shared_contract_numbers() {
    assert_eq!(space::S4, infiltrator_contract::design_tokens::space::XS);
    assert_eq!(space::S8, infiltrator_contract::design_tokens::space::SM);
    assert_eq!(space::S12, infiltrator_contract::design_tokens::space::MD);
    assert_eq!(space::S16, infiltrator_contract::design_tokens::space::LG);
    assert_eq!(space::S20, infiltrator_contract::design_tokens::space::XL);
    assert_eq!(space::S24, infiltrator_contract::design_tokens::space::XXL);
    assert_eq!(
        radius::CARD,
        infiltrator_contract::design_tokens::radius::CARD
    );
    assert_eq!(
        radius::CONTROL,
        infiltrator_contract::design_tokens::radius::CONTROL
    );
    assert_eq!(
        metrics::HAIRLINE,
        infiltrator_contract::design_tokens::metrics::HAIRLINE
    );
}

/// The contract values are the Iced reference, so the two dark-ish and
/// light-ish skins must stay visibly distinct after the alignment.
#[test]
fn the_aligned_palette_keeps_every_skin_distinct() {
    let dark = Theme::dark();
    let light = Theme::light();
    let forest = Theme::forest();
    let amoled = Theme::amoled();
    assert_ne!(dark.window_bg, light.window_bg);
    assert_ne!(dark.surface, light.surface);
    assert_ne!(forest.window_bg, light.window_bg);
    assert_ne!(amoled.window_bg, dark.window_bg);
    assert_ne!(dark.accent, light.accent);
}
