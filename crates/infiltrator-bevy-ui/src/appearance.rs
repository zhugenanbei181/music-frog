//! Appearance preference plumbing for the Bevy shell (DUAL-15-09).
//!
//! The shared contract owns the four skins and the `system` preference; this
//! module projects that vocabulary onto the widget layer's mirror, follows
//! the live OS appearance reported by winit, and repaints on switch.

use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::ui_widgets::Activate;
use bevy::window::{Window, WindowTheme};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::ThemeSkin;
use infiltrator_contract::theme::ThemePreference;

use crate::app::ThemeToggle;

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
pub fn skin_from_contract(skin: infiltrator_contract::theme::ThemeSkin) -> ThemeSkin {
    match skin {
        infiltrator_contract::theme::ThemeSkin::Dark => ThemeSkin::Dark,
        infiltrator_contract::theme::ThemeSkin::Light => ThemeSkin::Light,
        infiltrator_contract::theme::ThemeSkin::Forest => ThemeSkin::Forest,
        infiltrator_contract::theme::ThemeSkin::Amoled => ThemeSkin::Amoled,
    }
}

/// Project the widget layer's skin mirror back onto the shared vocabulary
/// (used by the capture/env knobs, which speak widget-layer skins).
pub fn contract_skin_from_widget(skin: ThemeSkin) -> infiltrator_contract::theme::ThemeSkin {
    match skin {
        ThemeSkin::Dark => infiltrator_contract::theme::ThemeSkin::Dark,
        ThemeSkin::Light => infiltrator_contract::theme::ThemeSkin::Light,
        ThemeSkin::Forest => infiltrator_contract::theme::ThemeSkin::Forest,
        ThemeSkin::Amoled => infiltrator_contract::theme::ThemeSkin::Amoled,
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
