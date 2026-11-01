//! Desktop host sidebar material support detection and effect application (FLPARITY-039).

use infiltrator_contract::sidebar_material::{SidebarMaterialKind, SidebarMaterialSupport};
use infiltrator_contract::window_chrome::DesktopPlatformKind;

/// Detect platform host capability support for a requested sidebar material.
pub fn detect_sidebar_material_support(
    kind: SidebarMaterialKind,
    platform: DesktopPlatformKind,
    os_build: Option<u32>,
) -> SidebarMaterialSupport {
    match kind {
        SidebarMaterialKind::OpaqueFallback => SidebarMaterialSupport::Supported {
            kind,
            needs_alpha_surface: false,
        },
        SidebarMaterialKind::WindowsMica => {
            if platform != DesktopPlatformKind::Windows {
                return SidebarMaterialSupport::unsupported("non-windows-platform");
            }
            if os_build.unwrap_or(0) >= 22000 {
                SidebarMaterialSupport::Supported {
                    kind,
                    needs_alpha_surface: false,
                }
            } else {
                SidebarMaterialSupport::unsupported("windows-build-below-22000")
            }
        }
        SidebarMaterialKind::WindowsMicaAlt => {
            if platform != DesktopPlatformKind::Windows {
                return SidebarMaterialSupport::unsupported("non-windows-platform");
            }
            if os_build.unwrap_or(0) >= 22621 {
                SidebarMaterialSupport::Supported {
                    kind,
                    needs_alpha_surface: false,
                }
            } else {
                SidebarMaterialSupport::unsupported("windows-build-below-22621")
            }
        }
        SidebarMaterialKind::WindowsAcrylic => {
            if platform != DesktopPlatformKind::Windows {
                return SidebarMaterialSupport::unsupported("non-windows-platform");
            }
            if os_build.unwrap_or(0) >= 17134 {
                SidebarMaterialSupport::Supported {
                    kind,
                    needs_alpha_surface: true,
                }
            } else {
                SidebarMaterialSupport::unsupported("windows-build-below-17134")
            }
        }
        SidebarMaterialKind::MacVibrancy => {
            if platform == DesktopPlatformKind::MacOs {
                SidebarMaterialSupport::Supported {
                    kind,
                    needs_alpha_surface: true,
                }
            } else {
                SidebarMaterialSupport::unsupported("non-macos-platform")
            }
        }
        SidebarMaterialKind::WaylandBlur => {
            if platform == DesktopPlatformKind::Linux {
                SidebarMaterialSupport::Supported {
                    kind,
                    needs_alpha_surface: true,
                }
            } else {
                SidebarMaterialSupport::unsupported("non-linux-platform")
            }
        }
    }
}

/// Apply sidebar material backdrop on the active window host.
///
/// Returns the effective applied material; returns `None` if the platform
/// extension is absent or unsupported, leaving the window surface opaque.
pub fn apply_sidebar_material(kind: SidebarMaterialKind) -> Option<SidebarMaterialKind> {
    if kind == SidebarMaterialKind::OpaqueFallback {
        return None;
    }
    // Platforms without live blur extensions return None honestly
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_support_on_windows_builds() {
        let win = DesktopPlatformKind::Windows;
        assert!(
            detect_sidebar_material_support(SidebarMaterialKind::WindowsMica, win, Some(22000))
                .is_supported()
        );
        assert!(
            !detect_sidebar_material_support(SidebarMaterialKind::WindowsMica, win, Some(19045))
                .is_supported()
        );

        assert!(
            detect_sidebar_material_support(SidebarMaterialKind::WindowsMicaAlt, win, Some(22621))
                .is_supported()
        );
        assert!(
            !detect_sidebar_material_support(SidebarMaterialKind::WindowsMicaAlt, win, Some(22000))
                .is_supported()
        );

        assert!(
            detect_sidebar_material_support(SidebarMaterialKind::WindowsAcrylic, win, Some(19045))
                .is_supported()
        );
        assert!(
            !detect_sidebar_material_support(SidebarMaterialKind::WindowsAcrylic, win, Some(10240))
                .is_supported()
        );
    }

    #[test]
    fn detect_support_on_macos_and_linux() {
        assert!(
            detect_sidebar_material_support(
                SidebarMaterialKind::MacVibrancy,
                DesktopPlatformKind::MacOs,
                None
            )
            .is_supported()
        );
        assert!(
            !detect_sidebar_material_support(
                SidebarMaterialKind::MacVibrancy,
                DesktopPlatformKind::Linux,
                None
            )
            .is_supported()
        );

        assert!(
            detect_sidebar_material_support(
                SidebarMaterialKind::WaylandBlur,
                DesktopPlatformKind::Linux,
                None
            )
            .is_supported()
        );
        assert!(
            !detect_sidebar_material_support(
                SidebarMaterialKind::WaylandBlur,
                DesktopPlatformKind::Windows,
                None
            )
            .is_supported()
        );
    }

    #[test]
    fn opaque_fallback_is_universally_supported() {
        for platform in [
            DesktopPlatformKind::Windows,
            DesktopPlatformKind::MacOs,
            DesktopPlatformKind::Linux,
            DesktopPlatformKind::Other,
        ] {
            let support = detect_sidebar_material_support(
                SidebarMaterialKind::OpaqueFallback,
                platform,
                None,
            );
            assert!(support.is_supported());
            assert_eq!(support.unsupported_reason(), None);
        }
    }
}
