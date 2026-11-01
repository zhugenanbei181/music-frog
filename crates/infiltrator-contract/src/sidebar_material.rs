//! Desktop sidebar system material effects and honest fallback contract (FLPARITY-039).
//!
//! Defines host-owned system materials (Windows Mica/MicaAlt/Acrylic, macOS
//! Vibrancy, Linux Wayland blur) for the desktop sidebar, along with strictly
//! typed adaptation rules:
//! - Reduced transparency accessibility preference forces OpaqueFallback.
//! - Unfocused (inactive) window state drops translucent blur to prevent visual
//!   noise and power drain.
//! - Unsupported OS versions or platforms degrade honestly with explicit reasons.

use crate::window_chrome::DesktopPlatformKind;
use serde::{Deserialize, Serialize};

/// System material effect kind requested or applied to the desktop sidebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidebarMaterialKind {
    /// Neutral solid background without host translucent blur.
    #[default]
    OpaqueFallback,
    /// Windows 11 DWM Mica (Build >= 22000).
    WindowsMica,
    /// Windows 11 DWM Mica Alt / Tabbed (Build >= 22621).
    WindowsMicaAlt,
    /// Windows 10/11 DWM Acrylic blur (Build >= 17134).
    WindowsAcrylic,
    /// macOS NSVisualEffectView Vibrancy (Sidebar material).
    MacVibrancy,
    /// Linux Wayland compositor background blur (e.g. org_kde_kwin_blur).
    WaylandBlur,
}

impl SidebarMaterialKind {
    /// Whether this material requires a transparent/alpha window surface to show through.
    pub const fn needs_alpha_surface(self) -> bool {
        matches!(
            self,
            Self::WindowsAcrylic | Self::MacVibrancy | Self::WaylandBlur
        )
    }

    /// The standard human-readable identifier.
    pub const fn id(self) -> &'static str {
        match self {
            Self::OpaqueFallback => "opaque-fallback",
            Self::WindowsMica => "windows-mica",
            Self::WindowsMicaAlt => "windows-mica-alt",
            Self::WindowsAcrylic => "windows-acrylic",
            Self::MacVibrancy => "mac-vibrancy",
            Self::WaylandBlur => "wayland-blur",
        }
    }
}

/// Platform capability support for a sidebar system material.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidebarMaterialSupport {
    Supported {
        kind: SidebarMaterialKind,
        needs_alpha_surface: bool,
    },
    Unsupported {
        reason: String,
    },
}

impl SidebarMaterialSupport {
    pub const fn is_supported(&self) -> bool {
        matches!(self, Self::Supported { .. })
    }

    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported {
            reason: reason.into(),
        }
    }

    pub fn unsupported_reason(&self) -> Option<&str> {
        match self {
            Self::Supported { .. } => None,
            Self::Unsupported { reason } => Some(reason.as_str()),
        }
    }
}

/// Resolved runtime state of the sidebar material.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarMaterialState {
    pub target_material: SidebarMaterialKind,
    pub effective_material: SidebarMaterialKind,
    pub reduce_transparency_active: bool,
    pub window_focused: bool,
    pub unsupported_reason: Option<String>,
}

impl SidebarMaterialState {
    /// Whether the effective material has an alpha/translucent window surface.
    pub const fn is_translucent(&self) -> bool {
        self.effective_material.needs_alpha_surface()
    }

    /// Returns an opaque fallback state with a given reason.
    pub fn opaque(target_material: SidebarMaterialKind, reason: Option<impl Into<String>>) -> Self {
        Self {
            target_material,
            effective_material: SidebarMaterialKind::OpaqueFallback,
            reduce_transparency_active: false,
            window_focused: true,
            unsupported_reason: reason.map(Into::into),
        }
    }
}

/// Specification for resolving the sidebar material across platform conditions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarMaterialSpec {
    pub target_material: SidebarMaterialKind,
    pub reduce_transparency: bool,
    pub window_focused: bool,
    pub platform: DesktopPlatformKind,
    pub os_build: Option<u32>,
}

impl SidebarMaterialSpec {
    pub fn new(target_material: SidebarMaterialKind, platform: DesktopPlatformKind) -> Self {
        Self {
            target_material,
            reduce_transparency: false,
            window_focused: true,
            platform,
            os_build: None,
        }
    }

    /// Resolve the effective material honestly against accessibility, focus, and platform rules.
    pub fn resolve(&self) -> SidebarMaterialState {
        if self.reduce_transparency {
            return SidebarMaterialState {
                target_material: self.target_material,
                effective_material: SidebarMaterialKind::OpaqueFallback,
                reduce_transparency_active: true,
                window_focused: self.window_focused,
                unsupported_reason: Some("reduce-transparency-enabled".to_string()),
            };
        }

        if !self.window_focused {
            return SidebarMaterialState {
                target_material: self.target_material,
                effective_material: SidebarMaterialKind::OpaqueFallback,
                reduce_transparency_active: false,
                window_focused: false,
                unsupported_reason: Some("window-unfocused".to_string()),
            };
        }

        match self.target_material {
            SidebarMaterialKind::OpaqueFallback => SidebarMaterialState {
                target_material: self.target_material,
                effective_material: SidebarMaterialKind::OpaqueFallback,
                reduce_transparency_active: false,
                window_focused: true,
                unsupported_reason: None,
            },
            SidebarMaterialKind::WindowsMica => {
                if self.platform == DesktopPlatformKind::Windows {
                    if self.os_build.unwrap_or(0) >= 22000 {
                        SidebarMaterialState {
                            target_material: self.target_material,
                            effective_material: SidebarMaterialKind::WindowsMica,
                            reduce_transparency_active: false,
                            window_focused: true,
                            unsupported_reason: None,
                        }
                    } else {
                        SidebarMaterialState {
                            target_material: self.target_material,
                            effective_material: SidebarMaterialKind::OpaqueFallback,
                            reduce_transparency_active: false,
                            window_focused: true,
                            unsupported_reason: Some("windows-build-below-22000".to_string()),
                        }
                    }
                } else {
                    SidebarMaterialState {
                        target_material: self.target_material,
                        effective_material: SidebarMaterialKind::OpaqueFallback,
                        reduce_transparency_active: false,
                        window_focused: true,
                        unsupported_reason: Some("non-windows-platform".to_string()),
                    }
                }
            }
            SidebarMaterialKind::WindowsMicaAlt => {
                if self.platform == DesktopPlatformKind::Windows {
                    let build = self.os_build.unwrap_or(0);
                    if build >= 22621 {
                        SidebarMaterialState {
                            target_material: self.target_material,
                            effective_material: SidebarMaterialKind::WindowsMicaAlt,
                            reduce_transparency_active: false,
                            window_focused: true,
                            unsupported_reason: None,
                        }
                    } else if build >= 22000 {
                        SidebarMaterialState {
                            target_material: self.target_material,
                            effective_material: SidebarMaterialKind::WindowsMica,
                            reduce_transparency_active: false,
                            window_focused: true,
                            unsupported_reason: Some("fallback-to-mica".to_string()),
                        }
                    } else {
                        SidebarMaterialState {
                            target_material: self.target_material,
                            effective_material: SidebarMaterialKind::OpaqueFallback,
                            reduce_transparency_active: false,
                            window_focused: true,
                            unsupported_reason: Some("windows-build-below-22000".to_string()),
                        }
                    }
                } else {
                    SidebarMaterialState {
                        target_material: self.target_material,
                        effective_material: SidebarMaterialKind::OpaqueFallback,
                        reduce_transparency_active: false,
                        window_focused: true,
                        unsupported_reason: Some("non-windows-platform".to_string()),
                    }
                }
            }
            SidebarMaterialKind::WindowsAcrylic => {
                if self.platform == DesktopPlatformKind::Windows {
                    if self.os_build.unwrap_or(0) >= 17134 {
                        SidebarMaterialState {
                            target_material: self.target_material,
                            effective_material: SidebarMaterialKind::WindowsAcrylic,
                            reduce_transparency_active: false,
                            window_focused: true,
                            unsupported_reason: None,
                        }
                    } else {
                        SidebarMaterialState {
                            target_material: self.target_material,
                            effective_material: SidebarMaterialKind::OpaqueFallback,
                            reduce_transparency_active: false,
                            window_focused: true,
                            unsupported_reason: Some("windows-build-below-17134".to_string()),
                        }
                    }
                } else {
                    SidebarMaterialState {
                        target_material: self.target_material,
                        effective_material: SidebarMaterialKind::OpaqueFallback,
                        reduce_transparency_active: false,
                        window_focused: true,
                        unsupported_reason: Some("non-windows-platform".to_string()),
                    }
                }
            }
            SidebarMaterialKind::MacVibrancy => {
                if self.platform == DesktopPlatformKind::MacOs {
                    SidebarMaterialState {
                        target_material: self.target_material,
                        effective_material: SidebarMaterialKind::MacVibrancy,
                        reduce_transparency_active: false,
                        window_focused: true,
                        unsupported_reason: None,
                    }
                } else {
                    SidebarMaterialState {
                        target_material: self.target_material,
                        effective_material: SidebarMaterialKind::OpaqueFallback,
                        reduce_transparency_active: false,
                        window_focused: true,
                        unsupported_reason: Some("non-macos-platform".to_string()),
                    }
                }
            }
            SidebarMaterialKind::WaylandBlur => {
                if self.platform == DesktopPlatformKind::Linux {
                    SidebarMaterialState {
                        target_material: self.target_material,
                        effective_material: SidebarMaterialKind::WaylandBlur,
                        reduce_transparency_active: false,
                        window_focused: true,
                        unsupported_reason: None,
                    }
                } else {
                    SidebarMaterialState {
                        target_material: self.target_material,
                        effective_material: SidebarMaterialKind::OpaqueFallback,
                        reduce_transparency_active: false,
                        window_focused: true,
                        unsupported_reason: Some("non-linux-platform".to_string()),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduce_transparency_forces_opaque_fallback() {
        let mut spec = SidebarMaterialSpec::new(
            SidebarMaterialKind::WindowsAcrylic,
            DesktopPlatformKind::Windows,
        );
        spec.os_build = Some(22621);
        spec.reduce_transparency = true;
        let resolved = spec.resolve();
        assert_eq!(
            resolved.effective_material,
            SidebarMaterialKind::OpaqueFallback
        );
        assert!(!resolved.is_translucent());
        assert!(resolved.reduce_transparency_active);
        assert_eq!(
            resolved.unsupported_reason.as_deref(),
            Some("reduce-transparency-enabled")
        );
    }

    #[test]
    fn window_unfocused_drops_translucent_blur() {
        let mut spec =
            SidebarMaterialSpec::new(SidebarMaterialKind::MacVibrancy, DesktopPlatformKind::MacOs);
        spec.window_focused = false;
        let resolved = spec.resolve();
        assert_eq!(
            resolved.effective_material,
            SidebarMaterialKind::OpaqueFallback
        );
        assert!(!resolved.is_translucent());
        assert!(!resolved.window_focused);
        assert_eq!(
            resolved.unsupported_reason.as_deref(),
            Some("window-unfocused")
        );
    }

    #[test]
    fn windows_mica_and_mica_alt_version_fences() {
        let mut spec = SidebarMaterialSpec::new(
            SidebarMaterialKind::WindowsMicaAlt,
            DesktopPlatformKind::Windows,
        );
        spec.os_build = Some(26100);
        assert_eq!(
            spec.resolve().effective_material,
            SidebarMaterialKind::WindowsMicaAlt
        );

        spec.os_build = Some(22000);
        let fallback_mica = spec.resolve();
        assert_eq!(
            fallback_mica.effective_material,
            SidebarMaterialKind::WindowsMica
        );
        assert_eq!(
            fallback_mica.unsupported_reason.as_deref(),
            Some("fallback-to-mica")
        );

        spec.os_build = Some(19045);
        assert_eq!(
            spec.resolve().effective_material,
            SidebarMaterialKind::OpaqueFallback
        );
    }

    #[test]
    fn platform_mismatch_falls_back_honestly() {
        let spec = SidebarMaterialSpec::new(
            SidebarMaterialKind::MacVibrancy,
            DesktopPlatformKind::Windows,
        );
        let resolved = spec.resolve();
        assert_eq!(
            resolved.effective_material,
            SidebarMaterialKind::OpaqueFallback
        );
        assert_eq!(
            resolved.unsupported_reason.as_deref(),
            Some("non-macos-platform")
        );
    }
}
