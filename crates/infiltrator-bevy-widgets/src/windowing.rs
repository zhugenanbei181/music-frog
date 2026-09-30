//! Multi-window management, Picture-in-Picture (PiP) floating overlays, and dockable panels.

use bevy::ecs::component::Component;
use bevy::ecs::resource::Resource;
use bevy::math::Vec2;

/// Docking split slot orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockSlot {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

/// A dockable workspace panel node.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct DockPanel {
    pub id: String,
    pub title: String,
    pub current_slot: DockSlot,
    pub is_floating: bool,
    pub floating_position: Vec2,
    pub floating_size: Vec2,
}

impl DockPanel {
    pub fn new(id: impl Into<String>, title: impl Into<String>, slot: DockSlot) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            current_slot: slot,
            is_floating: false,
            floating_position: Vec2::new(100.0, 100.0),
            floating_size: Vec2::new(300.0, 200.0),
        }
    }
}

/// Picture-in-Picture (PiP) floating mini-window state machine.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PipOverlayState {
    pub is_active: bool,
    pub is_pinned_top: bool,
    pub is_click_through: bool,
    pub position: Vec2,
    pub size: Vec2,
    pub opacity: f32,
}

impl Default for PipOverlayState {
    fn default() -> Self {
        Self {
            is_active: false,
            is_pinned_top: true,
            is_click_through: false,
            position: Vec2::new(20.0, 20.0),
            size: Vec2::new(180.0, 64.0),
            opacity: 0.9,
        }
    }
}

impl PipOverlayState {
    pub fn snap_to_corner(&mut self, screen_size: Vec2, top_right: bool) {
        if top_right {
            self.position = Vec2::new(screen_size.x - self.size.x - 20.0, 20.0);
        } else {
            self.position = Vec2::new(20.0, screen_size.y - self.size.y - 20.0);
        }
    }
}

/// Native OS window material / backdrop type (BEVY-GAP-147, UI-04-06).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowBackdropMaterial {
    /// Standard opaque window surface without transparency effects.
    #[default]
    Opaque,
    /// Windows 11 Mica dynamic desktop-tinted material (DWM).
    Mica,
    /// Windows 11 Mica Alt (tabbed backdrop variant).
    MicaAlt,
    /// Windows 10/11 Acrylic blur effect.
    Acrylic,
    /// macOS NSVisualEffectView native frosted glass Vibrancy.
    Vibrancy,
}

/// Dynamic window backdrop appearance specification.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowBackdropSpec {
    pub material: WindowBackdropMaterial,
    /// Effective window background tint alpha (0.0 = completely clear, 1.0 = opaque).
    pub surface_alpha: f32,
    /// Corner curvature for the native window frame.
    pub corner_radius: f32,
    /// Whether dark mode tint is actively forced onto the backdrop.
    pub dark_tint: bool,
}

impl Default for WindowBackdropSpec {
    fn default() -> Self {
        Self {
            material: WindowBackdropMaterial::Opaque,
            surface_alpha: 1.0,
            corner_radius: 12.0,
            dark_tint: true,
        }
    }
}

impl WindowBackdropSpec {
    /// Create an optimized backdrop configuration for the target OS and build.
    pub fn for_platform(
        target: WindowBackdropMaterial,
        os: &str,
        win_build: Option<u32>,
        dark_mode: bool,
    ) -> Self {
        let (resolved, alpha) = match target {
            WindowBackdropMaterial::Opaque => (WindowBackdropMaterial::Opaque, 1.0),
            WindowBackdropMaterial::Mica => {
                if os.eq_ignore_ascii_case("windows") && win_build.is_some_and(|b| b >= 22000) {
                    (WindowBackdropMaterial::Mica, 0.78)
                } else {
                    (WindowBackdropMaterial::Opaque, 1.0)
                }
            }
            WindowBackdropMaterial::MicaAlt => {
                if os.eq_ignore_ascii_case("windows") && win_build.is_some_and(|b| b >= 22621) {
                    (WindowBackdropMaterial::MicaAlt, 0.72)
                } else if os.eq_ignore_ascii_case("windows")
                    && win_build.is_some_and(|b| b >= 22000)
                {
                    (WindowBackdropMaterial::Mica, 0.78)
                } else {
                    (WindowBackdropMaterial::Opaque, 1.0)
                }
            }
            WindowBackdropMaterial::Acrylic => {
                if os.eq_ignore_ascii_case("windows") && win_build.is_some_and(|b| b >= 17134) {
                    (WindowBackdropMaterial::Acrylic, 0.65)
                } else {
                    (WindowBackdropMaterial::Opaque, 1.0)
                }
            }
            WindowBackdropMaterial::Vibrancy => {
                if os.eq_ignore_ascii_case("macos") || os.eq_ignore_ascii_case("darwin") {
                    (WindowBackdropMaterial::Vibrancy, 0.75)
                } else {
                    (WindowBackdropMaterial::Opaque, 1.0)
                }
            }
        };

        Self {
            material: resolved,
            surface_alpha: alpha,
            corner_radius: if os.eq_ignore_ascii_case("macos") || os.eq_ignore_ascii_case("darwin")
            {
                10.0
            } else {
                8.0
            },
            dark_tint: dark_mode,
        }
    }

    /// Whether the resolved window backdrop requires semi-transparent rendering.
    pub fn is_translucent(&self) -> bool {
        self.material != WindowBackdropMaterial::Opaque && self.surface_alpha < 1.0
    }
}

/// Global resource holding active window backdrop parameters.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct WindowBackdropState(pub WindowBackdropSpec);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_backdrop_resolution_respects_platform_capabilities() {
        // Windows 11 Build 22000 supports Mica
        let win11_mica = WindowBackdropSpec::for_platform(
            WindowBackdropMaterial::Mica,
            "windows",
            Some(22000),
            true,
        );
        assert_eq!(win11_mica.material, WindowBackdropMaterial::Mica);
        assert!(win11_mica.is_translucent());
        assert!((win11_mica.surface_alpha - 0.78).abs() < 1e-4);

        // Windows 10 Build 19045 falls back to Opaque for Mica
        let win10_mica = WindowBackdropSpec::for_platform(
            WindowBackdropMaterial::Mica,
            "windows",
            Some(19045),
            true,
        );
        assert_eq!(win10_mica.material, WindowBackdropMaterial::Opaque);
        assert!(!win10_mica.is_translucent());

        // Windows 10 Build 19045 supports Acrylic
        let win10_acrylic = WindowBackdropSpec::for_platform(
            WindowBackdropMaterial::Acrylic,
            "windows",
            Some(19045),
            true,
        );
        assert_eq!(win10_acrylic.material, WindowBackdropMaterial::Acrylic);
        assert!(win10_acrylic.is_translucent());

        // macOS Vibrancy
        let macos_vibrancy =
            WindowBackdropSpec::for_platform(WindowBackdropMaterial::Vibrancy, "macos", None, true);
        assert_eq!(macos_vibrancy.material, WindowBackdropMaterial::Vibrancy);
        assert_eq!(macos_vibrancy.corner_radius, 10.0);
        assert!(macos_vibrancy.is_translucent());

        // Linux falls back to Opaque
        let linux_fallback =
            WindowBackdropSpec::for_platform(WindowBackdropMaterial::Mica, "linux", None, true);
        assert_eq!(linux_fallback.material, WindowBackdropMaterial::Opaque);
        assert!(!linux_fallback.is_translucent());
    }
}
