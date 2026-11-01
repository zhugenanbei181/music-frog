//! Projection of the desktop sidebar system material state (FLPARITY-039).

use infiltrator_contract::sidebar_material::{
    SidebarMaterialKind, SidebarMaterialSpec, SidebarMaterialState,
};
use infiltrator_contract::window_chrome::DesktopPlatformKind;

#[cfg(test)]
#[path = "sidebar_material_projection_tests.rs"]
mod tests;

/// Rgba tuple `(r, g, b, a)` where components are in range 0.0..=1.0.
pub type RgbaTuple = (f32, f32, f32, f32);

/// Project the sidebar material state based on target, accessibility, focus, and platform.
pub fn project_sidebar_material(
    target: SidebarMaterialKind,
    reduce_transparency: bool,
    window_focused: bool,
    platform: DesktopPlatformKind,
    os_build: Option<u32>,
) -> SidebarMaterialState {
    let mut spec = SidebarMaterialSpec::new(target, platform);
    spec.reduce_transparency = reduce_transparency;
    spec.window_focused = window_focused;
    spec.os_build = os_build;
    spec.resolve()
}

/// Compute the sidebar container surface wash color with appropriate alpha transparency.
///
/// If the material is translucent or host-rendered (Mica/Acrylic/Vibrancy/WaylandBlur),
/// the alpha is adjusted so host blur shows through without washing out text contrast.
/// If opaque fallback is active, alpha is always 1.0 (fully opaque).
pub fn project_sidebar_surface_wash(
    state: &SidebarMaterialState,
    base_rgb: (f32, f32, f32),
    is_dark: bool,
) -> RgbaTuple {
    let (r, g, b) = base_rgb;
    if !state.window_focused || state.reduce_transparency_active {
        return (r, g, b, 1.0);
    }

    match state.effective_material {
        SidebarMaterialKind::OpaqueFallback => (r, g, b, 1.0),
        SidebarMaterialKind::WindowsMica | SidebarMaterialKind::WindowsMicaAlt => {
            // Mica draws on opaque window background in DWM; subtle tint for surface layering
            let alpha = if is_dark { 0.12 } else { 0.18 };
            (r, g, b, alpha)
        }
        SidebarMaterialKind::WindowsAcrylic => {
            // Acrylic needs translucent alpha surface for DWM blur
            let alpha = if is_dark { 0.65 } else { 0.72 };
            (r, g, b, alpha)
        }
        SidebarMaterialKind::MacVibrancy => {
            // macOS NSVisualEffectView vibrancy
            let alpha = if is_dark { 0.20 } else { 0.25 };
            (r, g, b, alpha)
        }
        SidebarMaterialKind::WaylandBlur => {
            // Wayland compositor background blur
            let alpha = if is_dark { 0.60 } else { 0.70 };
            (r, g, b, alpha)
        }
    }
}
