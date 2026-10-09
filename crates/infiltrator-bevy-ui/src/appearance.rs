//! Appearance preference plumbing for the Bevy shell (DUAL-15-09).
//!
//! The shared contract owns the four skins and the `system` preference; this
//! module projects that vocabulary onto the widget layer's mirror, follows
//! the live OS appearance reported by winit, and repaints on switch.

use crate::app::ThemeToggle;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{BackgroundColor, BorderRadius, FlexDirection, Node, UiRect, percent, px};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use bevy::window::{Window, WindowTheme};
use infiltrator_bevy_widgets::palette::{UiPalette, theme_color};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin, space};
use infiltrator_bevy_widgets::theme_export::{
    DEFAULT_LADDER_STEPS, LadderSource, LadderStrategy, TonalLadder, generate_ladder,
    theme_tonal_ladders,
};
use infiltrator_contract::theme;
use infiltrator_contract::theme::ThemePreference;

/// The shell's appearance preference (shared contract: a pinned skin or
/// "follow the OS"). The painted token set is always resolved through
/// [`resolved_skin`], never read from this resource directly.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThemeMode(pub ThemePreference);

/// The OS appearance reported by the windowing layer (winit `Window::theme`).
/// `None` until the first window reports one; the shell paints its documented
/// dark cold start meanwhile.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemAppearance(pub Option<bool>);

/// Project the shared skin vocabulary onto the widget layer's mirror.
pub fn skin_from_contract(skin: theme::ThemeSkin) -> ThemeSkin {
    match skin {
        theme::ThemeSkin::Dark => ThemeSkin::Dark,
        theme::ThemeSkin::Light => ThemeSkin::Light,
        theme::ThemeSkin::Forest => ThemeSkin::Forest,
        theme::ThemeSkin::Amoled => ThemeSkin::Amoled,
    }
}

/// Project the widget layer's skin mirror back onto the shared vocabulary
/// (used by the capture/env knobs, which speak widget-layer skins).
pub fn contract_skin_from_widget(skin: ThemeSkin) -> theme::ThemeSkin {
    match skin {
        ThemeSkin::Dark => theme::ThemeSkin::Dark,
        ThemeSkin::Light => theme::ThemeSkin::Light,
        ThemeSkin::Forest => theme::ThemeSkin::Forest,
        ThemeSkin::Amoled => theme::ThemeSkin::Amoled,
    }
}

/// Resolve the preference against the live OS appearance.
pub fn resolved_skin(preference: ThemePreference, system_prefers_dark: Option<bool>) -> ThemeSkin {
    skin_from_contract(preference.resolve(system_prefers_dark.unwrap_or(true)))
}

/// Theme toggle observer: advances the shared appearance preference
/// (`system → dark → light → forest → amoled`) and repaints the resolved skin.
pub fn on_theme_pill_activated(
    activate: On<Activate>,
    toggles: Query<(), With<ThemeToggle>>,
    mut mode: ResMut<ThemeMode>,
    appearance: Option<Res<SystemAppearance>>,
    mut commands: Commands,
) {
    if !toggles.contains(activate.entity) {
        return;
    }
    let next = mode.0.next();
    mode.0 = next;
    let system = appearance.map(|appearance| appearance.0).unwrap_or(None);
    commands.trigger(ThemeSwitch(resolved_skin(next, system)));
}

/// Follow the OS appearance while the preference is `system`.
pub fn sync_system_appearance(
    mut commands: Commands,
    windows: Query<&Window>,
    mut appearance: ResMut<SystemAppearance>,
    mode: Res<ThemeMode>,
) {
    let Some(theme) = windows.iter().find_map(|window| window.window_theme) else {
        return;
    };
    let prefers_dark = matches!(theme, WindowTheme::Dark);
    if appearance.0 == Some(prefers_dark) {
        return;
    }
    appearance.0 = Some(prefers_dark);
    if mode.0.follows_system() {
        commands.trigger(ThemeSwitch(resolved_skin(mode.0, Some(prefers_dark))));
    }
}

// ---- BEVY-034: OKLCH tonal-ladder preview --------------------------------
//
// The preview is a derived view over the active skin's hand-picked semantic
// tokens: the perceptual OKLCH ladder (`theme_tonal_ladders`) plus the explicit
// sRGB fallback (`generate_ladder(.., LadderStrategy::SrgbFallback)`). Token
// names and values are never changed — only the surrounding ladder is shown.

/// Marker on the tonal-ladder preview card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TonalLadderPreviewRoot;

/// Marker on one perceptual (OKLCH) ladder stop swatch.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TonalLadderStopSwatch(pub usize);

/// Marker on one explicit sRGB-fallback ladder stop swatch.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TonalLadderFallbackSwatch(pub usize);

/// Marker on the source label naming the generators behind the two ladders.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TonalLadderSourceText;

/// The derived preview ladders for one skin.
#[derive(Clone, Debug, PartialEq)]
pub struct TonalLadderPreview {
    /// The perceptual OKLCH ladder generated from the accent seed.
    pub perceptual: TonalLadder,
    /// The explicit sRGB ramp used when OKLCH cannot be applied.
    pub fallback: TonalLadder,
}

/// Derive the preview ladders from the active skin's real accent token. The
/// seed keeps its hand-picked value; only the surrounding ladder is generated.
pub fn ladder_preview_for_skin(skin: ThemeSkin) -> TonalLadderPreview {
    let theme = Theme::for_mode(skin);
    let ladders = theme_tonal_ladders(&theme);
    TonalLadderPreview {
        perceptual: ladders.accent,
        fallback: generate_ladder(
            theme.accent,
            DEFAULT_LADDER_STEPS,
            LadderStrategy::SrgbFallback,
        ),
    }
}

fn ladder_source_label(source: LadderSource) -> &'static str {
    match source {
        LadderSource::Oklch => "oklch",
        LadderSource::SrgbFallback => "srgb-fallback",
    }
}

fn ladder_swatch_scene(index: usize, perceptual: bool, palette: &UiPalette) -> Box<dyn Scene> {
    if perceptual {
        Box::new(bsn! {
                Node { flex_grow: 1.0, height: percent(100) }
                TonalLadderStopSwatch(index)
                BackgroundColor({ palette.accent })
        })
    } else {
        Box::new(bsn! {
                Node { flex_grow: 1.0, height: percent(100) }
                TonalLadderFallbackSwatch(index)
                BackgroundColor({ palette.surface_elevated })
        })
    }
}

/// The tonal-ladder preview card for the Settings appearance section. The
/// swatch fills are restamped from the live palette by
/// [`refresh_tonal_ladder_preview`]; the tree is never rebuilt.
pub fn tonal_ladder_preview_scene(palette: &UiPalette) -> impl Scene + use<> {
    let perceptual: Vec<Box<dyn Scene>> = (0..DEFAULT_LADDER_STEPS)
        .map(|index| ladder_swatch_scene(index, true, palette))
        .collect();
    let fallback: Vec<Box<dyn Scene>> = (0..DEFAULT_LADDER_STEPS)
        .map(|index| ladder_swatch_scene(index, false, palette))
        .collect();
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(space::S4),
                padding: UiRect::all(px(space::S8)),
                border_radius: BorderRadius::all(px(palette.control_radius_px)),
            }
            TonalLadderPreviewRoot
            BackgroundColor({ palette.surface_elevated })
            Children [
                Text(String::new()) TonalLadderSourceText TextRole(Role::Caption)
                --
                Node {
                    width: percent(100),
                    height: px(24.0),
                    border_radius: BorderRadius::all(px(palette.control_radius_px)),
                }
                Children [ { perceptual } ]
                --
                Node {
                    width: percent(100),
                    height: px(12.0),
                    border_radius: BorderRadius::all(px(palette.control_radius_px)),
                }
                Children [ { fallback } ]
            ]
    }
}

/// Mutable swatch access: perceptual and fallback stops share the fill
/// component, so they are read through one disjoint query.
#[derive(QueryData)]
#[query_data(mutable)]
pub struct LadderSwatch {
    fill: &'static mut BackgroundColor,
    perceptual: Option<&'static TonalLadderStopSwatch>,
    fallback: Option<&'static TonalLadderFallbackSwatch>,
}

/// Restamp the preview from the active skin. Runs every frame and compares
/// before writing, so an unchanged frame costs nothing.
pub fn refresh_tonal_ladder_preview(
    mode: Res<ThemeMode>,
    appearance: Option<Res<SystemAppearance>>,
    mut swatches: Query<LadderSwatch>,
    mut labels: Query<&mut Text, With<TonalLadderSourceText>>,
) {
    let system = appearance.map(|appearance| appearance.0).unwrap_or(None);
    let preview = ladder_preview_for_skin(resolved_skin(mode.0, system));

    for mut swatch in &mut swatches {
        let stop = if let Some(perceptual) = swatch.perceptual {
            preview.perceptual.stops.get(perceptual.0).copied()
        } else if let Some(fallback) = swatch.fallback {
            preview.fallback.stops.get(fallback.0).copied()
        } else {
            None
        };
        let Some(stop) = stop else { continue };
        let color = theme_color(stop);
        if swatch.fill.0 != color {
            swatch.fill.0 = color;
        }
    }

    let label = format!(
        "{} · {}",
        ladder_source_label(preview.perceptual.source),
        ladder_source_label(preview.fallback.source),
    );
    for mut text in &mut labels {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}
