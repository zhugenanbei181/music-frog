//! DUAL-15-14 design-token mirror tests: the business-agnostic Bevy widget
//! layer must paint exactly the numbers the shared contract carries (the Iced
//! shell consumes the same contract values at compile time). A drifted mirror
//! is a real parity defect — the two surfaces would look different — so every
//! core palette channel and every structural token is asserted here.

use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin, metrics, radius, space};
use infiltrator_contract::design_tokens::{RgbaToken, SkinCorePalette, skin_core};
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
