//! The four skin `Tokens` constants (EFFECTS-030 #119-#121): every surface
//! color, wash and shadow tier of one appearance, composed from the shared
//! contract palettes.

use super::{DualShadow, Tokens, soft_wash};
use iced::{Color, Radians, Shadow, Vector};
use infiltrator_contract::design_tokens::{RgbaToken, SkinCorePalette, SkinInteractionPalette};
use infiltrator_contract::surface_hierarchy::MaterialSurfaceTokens;
use infiltrator_contract::theme::ThemeSkin;
use infiltrator_contract::theme_palette::ThemePalette;
use std::f32::consts::FRAC_PI_2;

/// Resolve a shared token into an Iced color.
pub(crate) const fn token_color(token: RgbaToken) -> Color {
    Color {
        r: token.r,
        g: token.g,
        b: token.b,
        a: token.a,
    }
}

const LIGHT_PALETTE: ThemePalette = ThemePalette::of_skin(ThemeSkin::Light);
const DARK_PALETTE: ThemePalette = ThemePalette::of_skin(ThemeSkin::Dark);
const FOREST_PALETTE: ThemePalette = ThemePalette::of_skin(ThemeSkin::Forest);
const AMOLED_PALETTE: ThemePalette = ThemePalette::of_skin(ThemeSkin::Amoled);

const LIGHT_CORE: SkinCorePalette = LIGHT_PALETTE.core;
const DARK_CORE: SkinCorePalette = DARK_PALETTE.core;
const FOREST_CORE: SkinCorePalette = FOREST_PALETTE.core;
const AMOLED_CORE: SkinCorePalette = AMOLED_PALETTE.core;

const LIGHT_INTERACTION: SkinInteractionPalette = LIGHT_PALETTE.interaction;
const DARK_INTERACTION: SkinInteractionPalette = DARK_PALETTE.interaction;
const FOREST_INTERACTION: SkinInteractionPalette = FOREST_PALETTE.interaction;
const AMOLED_INTERACTION: SkinInteractionPalette = AMOLED_PALETTE.interaction;

const LIGHT_SURFACES: MaterialSurfaceTokens = LIGHT_PALETTE.surfaces;
const DARK_SURFACES: MaterialSurfaceTokens = DARK_PALETTE.surfaces;
const FOREST_SURFACES: MaterialSurfaceTokens = FOREST_PALETTE.surfaces;
const AMOLED_SURFACES: MaterialSurfaceTokens = AMOLED_PALETTE.surfaces;

/// A soft vertical wash angle: top-to-bottom fade (#120).
const VERTICAL: Radians = Radians(FRAC_PI_2);

/// Blend `c` toward white by `amount` — the const highlight overlay behind
/// `highlight_border` (#119).
const fn toward_white(c: Color, amount: f32) -> Color {
    Color {
        r: c.r + (1.0 - c.r) * amount,
        g: c.g + (1.0 - c.g) * amount,
        b: c.b + (1.0 - c.b) * amount,
        a: c.a,
    }
}

/// Soft warm-paper light appearance (anti-glare).
pub const LIGHT: Tokens = Tokens {
    canvas: token_color(LIGHT_SURFACES.canvas),
    sidebar: token_color(LIGHT_SURFACES.sidebar),
    card_bg: token_color(LIGHT_SURFACES.container_card),
    card_border: token_color(LIGHT_CORE.card_border),
    card_shadow: Shadow {
        color: Color {
            a: 0.05,
            ..Color::from_rgb(0.08, 0.12, 0.10)
        },
        offset: Vector::new(0.0, 1.0),
        blur_radius: 3.0,
    },
    floating_shadow: Shadow {
        color: Color {
            a: 0.14,
            ..Color::from_rgb(0.08, 0.12, 0.10)
        },
        offset: Vector::new(0.0, 4.0),
        blur_radius: 12.0,
    },
    card_dual_shadow: DualShadow {
        key: Shadow {
            color: Color {
                a: 0.04,
                ..Color::from_rgb(0.08, 0.12, 0.10)
            },
            offset: Vector::new(0.0, 1.0),
            blur_radius: 2.0,
        },
        ambient: Shadow {
            color: Color {
                a: 0.03,
                ..Color::from_rgb(0.08, 0.12, 0.10)
            },
            offset: Vector::new(0.0, 3.0),
            blur_radius: 8.0,
        },
    },
    floating_dual_shadow: DualShadow {
        key: Shadow {
            color: Color {
                a: 0.08,
                ..Color::from_rgb(0.08, 0.12, 0.10)
            },
            offset: Vector::new(0.0, 2.0),
            blur_radius: 4.0,
        },
        ambient: Shadow {
            color: Color {
                a: 0.10,
                ..Color::from_rgb(0.08, 0.12, 0.10)
            },
            offset: Vector::new(0.0, 8.0),
            blur_radius: 20.0,
        },
    },
    accent: token_color(LIGHT_CORE.accent),
    hero_wash: soft_wash(
        VERTICAL,
        token_color(LIGHT_CORE.sidebar),
        token_color(LIGHT_CORE.canvas),
    ),
    control_wash: soft_wash(
        VERTICAL,
        toward_white(token_color(LIGHT_CORE.control_bg), 0.04),
        token_color(LIGHT_CORE.control_bg),
    ),
    highlight_border: toward_white(token_color(LIGHT_CORE.card_border), 0.35),
    accent_soft: Color {
        a: 0.12,
        ..token_color(LIGHT_CORE.accent)
    },
    on_accent: token_color(LIGHT_CORE.on_accent),
    badge_accent: Color::from_rgb(0.04, 0.44, 0.88),
    text_primary: token_color(LIGHT_SURFACES.text_primary),
    text_secondary: token_color(LIGHT_SURFACES.text_secondary),
    text_tertiary: token_color(LIGHT_INTERACTION.disabled_ink),
    success: token_color(LIGHT_CORE.success),
    warning: token_color(LIGHT_CORE.warning),
    danger: token_color(LIGHT_CORE.danger),
    chip_bg: Color {
        a: 0.06,
        ..Color::BLACK
    },
    divider: Color {
        a: 0.08,
        ..Color::BLACK
    },
    sidebar_text: Color::from_rgb(0.12, 0.15, 0.14),
    sidebar_text_muted: Color {
        a: 0.82,
        ..Color::from_rgb(0.20, 0.24, 0.22)
    },
    control_bg: token_color(LIGHT_CORE.control_bg),
    hover: token_color(LIGHT_INTERACTION.hover),
    pressed: token_color(LIGHT_INTERACTION.pressed),
    focus_ring: token_color(LIGHT_INTERACTION.focus_ring),
    scrim: token_color(LIGHT_INTERACTION.scrim),
    overlay: Color {
        a: 0.90,
        ..Color::BLACK
    },
    overlay_text: Color::WHITE,
    overlay_text_muted: Color {
        a: 0.75,
        ..Color::WHITE
    },
    overlay_border: Color {
        a: 0.25,
        ..Color::WHITE
    },
    switch_track: Color {
        a: 0.32,
        ..Color::from_rgb(0.47, 0.47, 0.50)
    },
    switch_knob: Color::WHITE,
};

/// Soft dark appearance (deep balanced charcoal).
pub const DARK: Tokens = Tokens {
    canvas: token_color(DARK_SURFACES.canvas),
    sidebar: token_color(DARK_SURFACES.sidebar),
    card_bg: token_color(DARK_SURFACES.container_card),
    card_border: token_color(DARK_CORE.card_border),
    card_shadow: Shadow {
        color: Color {
            a: 0.32,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, 1.0),
        blur_radius: 4.0,
    },
    floating_shadow: Shadow {
        color: Color {
            a: 0.55,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, 4.0),
        blur_radius: 14.0,
    },
    card_dual_shadow: DualShadow {
        key: Shadow {
            color: Color {
                a: 0.25,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 1.0),
            blur_radius: 2.0,
        },
        ambient: Shadow {
            color: Color {
                a: 0.15,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 4.0),
            blur_radius: 10.0,
        },
    },
    floating_dual_shadow: DualShadow {
        key: Shadow {
            color: Color {
                a: 0.40,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 3.0),
            blur_radius: 6.0,
        },
        ambient: Shadow {
            color: Color {
                a: 0.30,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 10.0),
            blur_radius: 24.0,
        },
    },
    accent: token_color(DARK_CORE.accent),
    hero_wash: soft_wash(
        VERTICAL,
        token_color(DARK_CORE.sidebar),
        token_color(DARK_CORE.canvas),
    ),
    control_wash: soft_wash(
        VERTICAL,
        toward_white(token_color(DARK_CORE.control_bg), 0.04),
        token_color(DARK_CORE.control_bg),
    ),
    highlight_border: toward_white(token_color(DARK_CORE.card_border), 0.35),
    accent_soft: Color {
        a: 0.16,
        ..token_color(DARK_CORE.accent)
    },
    on_accent: token_color(DARK_CORE.on_accent),
    badge_accent: Color::from_rgb(0.46, 0.72, 1.0), // #76B8FF
    text_primary: token_color(DARK_SURFACES.text_primary),
    text_secondary: token_color(DARK_SURFACES.text_secondary),
    text_tertiary: token_color(DARK_INTERACTION.disabled_ink),
    success: token_color(DARK_CORE.success),
    warning: token_color(DARK_CORE.warning),
    danger: token_color(DARK_CORE.danger),
    chip_bg: Color {
        a: 0.10,
        ..Color::WHITE
    },
    divider: Color {
        a: 0.10,
        ..Color::WHITE
    },
    sidebar_text: Color::WHITE,
    sidebar_text_muted: Color {
        a: 0.75,
        ..Color::WHITE
    },
    control_bg: token_color(DARK_CORE.control_bg),
    hover: token_color(DARK_INTERACTION.hover),
    pressed: token_color(DARK_INTERACTION.pressed),
    focus_ring: token_color(DARK_INTERACTION.focus_ring),
    scrim: token_color(DARK_INTERACTION.scrim),
    overlay: Color {
        a: 0.92,
        ..Color::from_rgb(0.11, 0.11, 0.12)
    },
    overlay_text: Color::WHITE,
    overlay_text_muted: Color {
        a: 0.75,
        ..Color::WHITE
    },
    overlay_border: Color {
        a: 0.20,
        ..Color::WHITE
    },
    switch_track: Color {
        a: 0.32,
        ..Color::from_rgb(0.47, 0.47, 0.50)
    },
    switch_knob: Color::WHITE,
};

/// Eye-care forest appearance (EyeForest, TaskForest-inspired).
pub const FOREST: Tokens = Tokens {
    canvas: token_color(FOREST_SURFACES.canvas),
    sidebar: token_color(FOREST_SURFACES.sidebar),
    card_bg: token_color(FOREST_SURFACES.container_card),
    card_border: token_color(FOREST_CORE.card_border),
    card_shadow: Shadow {
        color: Color {
            a: 0.06,
            ..Color::from_rgb(0.12, 0.21, 0.14)
        },
        offset: Vector::new(0.0, 1.0),
        blur_radius: 3.0,
    },
    floating_shadow: Shadow {
        color: Color {
            a: 0.16,
            ..Color::from_rgb(0.12, 0.21, 0.14)
        },
        offset: Vector::new(0.0, 4.0),
        blur_radius: 12.0,
    },
    card_dual_shadow: DualShadow {
        key: Shadow {
            color: Color {
                a: 0.05,
                ..Color::from_rgb(0.12, 0.21, 0.14)
            },
            offset: Vector::new(0.0, 1.0),
            blur_radius: 2.0,
        },
        ambient: Shadow {
            color: Color {
                a: 0.04,
                ..Color::from_rgb(0.12, 0.21, 0.14)
            },
            offset: Vector::new(0.0, 4.0),
            blur_radius: 8.0,
        },
    },
    floating_dual_shadow: DualShadow {
        key: Shadow {
            color: Color {
                a: 0.10,
                ..Color::from_rgb(0.12, 0.21, 0.14)
            },
            offset: Vector::new(0.0, 2.0),
            blur_radius: 4.0,
        },
        ambient: Shadow {
            color: Color {
                a: 0.12,
                ..Color::from_rgb(0.12, 0.21, 0.14)
            },
            offset: Vector::new(0.0, 8.0),
            blur_radius: 20.0,
        },
    },
    accent: token_color(FOREST_CORE.accent),
    hero_wash: soft_wash(
        VERTICAL,
        token_color(FOREST_CORE.sidebar),
        token_color(FOREST_CORE.canvas),
    ),
    control_wash: soft_wash(
        VERTICAL,
        toward_white(token_color(FOREST_CORE.control_bg), 0.04),
        token_color(FOREST_CORE.control_bg),
    ),
    highlight_border: toward_white(token_color(FOREST_CORE.card_border), 0.35),
    accent_soft: Color {
        a: 0.14,
        ..token_color(FOREST_CORE.accent)
    },
    on_accent: token_color(FOREST_CORE.on_accent),
    badge_accent: Color::from_rgb(0.188, 0.435, 0.306),
    text_primary: token_color(FOREST_SURFACES.text_primary),
    text_secondary: token_color(FOREST_SURFACES.text_secondary),
    text_tertiary: token_color(FOREST_INTERACTION.disabled_ink),
    success: token_color(FOREST_CORE.success),
    warning: token_color(FOREST_CORE.warning),
    danger: token_color(FOREST_CORE.danger),
    chip_bg: Color {
        a: 0.12,
        ..Color::from_rgb(0.341, 0.439, 0.353)
    },
    divider: Color {
        a: 0.16,
        ..Color::from_rgb(0.341, 0.439, 0.353)
    },
    sidebar_text: Color::from_rgb(0.122, 0.208, 0.145),
    sidebar_text_muted: Color {
        a: 0.85,
        ..Color::from_rgb(0.18, 0.28, 0.20)
    },
    control_bg: token_color(FOREST_CORE.control_bg),
    hover: token_color(FOREST_INTERACTION.hover),
    pressed: token_color(FOREST_INTERACTION.pressed),
    focus_ring: token_color(FOREST_INTERACTION.focus_ring),
    scrim: token_color(FOREST_INTERACTION.scrim),
    overlay: Color {
        a: 0.94,
        ..Color::from_rgb(0.122, 0.208, 0.145)
    },
    overlay_text: Color::from_rgb(0.973, 0.984, 0.961),
    overlay_text_muted: Color {
        a: 0.75,
        ..Color::from_rgb(0.973, 0.984, 0.961)
    },
    overlay_border: Color {
        a: 0.25,
        ..Color::from_rgb(0.973, 0.984, 0.961)
    },
    switch_track: Color {
        a: 0.35,
        ..Color::from_rgb(0.40, 0.48, 0.42)
    },
    switch_knob: Color::WHITE,
};

/// Pure pitch-black appearance optimized for OLED displays and battery savings.
pub const AMOLED: Tokens = Tokens {
    canvas: token_color(AMOLED_SURFACES.canvas),
    sidebar: token_color(AMOLED_SURFACES.sidebar),
    card_bg: token_color(AMOLED_SURFACES.container_card),
    card_border: token_color(AMOLED_CORE.card_border),
    card_shadow: Shadow {
        color: Color {
            a: 0.45,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, 1.0),
        blur_radius: 4.0,
    },
    floating_shadow: Shadow {
        color: Color {
            a: 0.75,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, 4.0),
        blur_radius: 16.0,
    },
    card_dual_shadow: DualShadow {
        key: Shadow {
            color: Color {
                a: 0.35,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 1.0),
            blur_radius: 2.0,
        },
        ambient: Shadow {
            color: Color {
                a: 0.25,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 4.0),
            blur_radius: 8.0,
        },
    },
    floating_dual_shadow: DualShadow {
        key: Shadow {
            color: Color {
                a: 0.50,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 2.0),
            blur_radius: 4.0,
        },
        ambient: Shadow {
            color: Color {
                a: 0.60,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
    },
    accent: token_color(AMOLED_CORE.accent),
    hero_wash: soft_wash(
        VERTICAL,
        token_color(AMOLED_CORE.sidebar),
        token_color(AMOLED_CORE.canvas),
    ),
    control_wash: soft_wash(
        VERTICAL,
        toward_white(token_color(AMOLED_CORE.control_bg), 0.04),
        token_color(AMOLED_CORE.control_bg),
    ),
    highlight_border: toward_white(token_color(AMOLED_CORE.card_border), 0.35),
    accent_soft: Color {
        a: 0.18,
        ..token_color(AMOLED_CORE.accent)
    },
    on_accent: token_color(AMOLED_CORE.on_accent),
    badge_accent: Color::from_rgb(0.46, 0.72, 1.0), // #76B8FF
    text_primary: token_color(AMOLED_SURFACES.text_primary),
    text_secondary: token_color(AMOLED_SURFACES.text_secondary),
    text_tertiary: token_color(AMOLED_INTERACTION.disabled_ink),
    success: token_color(AMOLED_CORE.success),
    warning: token_color(AMOLED_CORE.warning),
    danger: token_color(AMOLED_CORE.danger),
    chip_bg: Color {
        a: 0.12,
        ..Color::WHITE
    },
    divider: Color {
        a: 0.12,
        ..Color::WHITE
    },
    sidebar_text: Color::from_rgb(0.973, 0.980, 0.988),
    sidebar_text_muted: Color {
        a: 0.50,
        ..Color::WHITE
    },
    control_bg: token_color(AMOLED_CORE.control_bg),
    hover: token_color(AMOLED_INTERACTION.hover),
    pressed: token_color(AMOLED_INTERACTION.pressed),
    focus_ring: token_color(AMOLED_INTERACTION.focus_ring),
    scrim: token_color(AMOLED_INTERACTION.scrim),
    overlay: Color {
        a: 0.95,
        ..Color::from_rgb(0.05, 0.06, 0.07)
    },
    overlay_text: Color::from_rgb(0.973, 0.980, 0.988),
    overlay_text_muted: Color {
        a: 0.75,
        ..Color::WHITE
    },
    overlay_border: Color {
        a: 0.22,
        ..Color::WHITE
    },
    switch_track: Color {
        a: 0.35,
        ..Color::from_rgb(0.47, 0.47, 0.50)
    },
    switch_knob: Color::WHITE,
};
