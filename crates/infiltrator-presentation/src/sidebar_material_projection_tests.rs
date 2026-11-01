use super::*;

#[test]
fn project_sidebar_material_respects_reduce_transparency() {
    let state = project_sidebar_material(
        SidebarMaterialKind::WindowsAcrylic,
        true,
        true,
        DesktopPlatformKind::Windows,
        Some(22621),
    );
    assert_eq!(
        state.effective_material,
        SidebarMaterialKind::OpaqueFallback
    );
    assert!(state.reduce_transparency_active);
    let wash = project_sidebar_surface_wash(&state, (0.1, 0.1, 0.1), true);
    assert_eq!(wash.3, 1.0);
}

#[test]
fn project_sidebar_material_unfocused_drops_blur() {
    let state = project_sidebar_material(
        SidebarMaterialKind::MacVibrancy,
        false,
        false,
        DesktopPlatformKind::MacOs,
        None,
    );
    assert_eq!(
        state.effective_material,
        SidebarMaterialKind::OpaqueFallback
    );
    assert!(!state.window_focused);
    let wash = project_sidebar_surface_wash(&state, (0.9, 0.9, 0.9), false);
    assert_eq!(wash.3, 1.0);
}

#[test]
fn project_sidebar_surface_wash_acrylic_and_mica() {
    let acrylic_state = project_sidebar_material(
        SidebarMaterialKind::WindowsAcrylic,
        false,
        true,
        DesktopPlatformKind::Windows,
        Some(22621),
    );
    assert_eq!(
        acrylic_state.effective_material,
        SidebarMaterialKind::WindowsAcrylic
    );
    let acrylic_wash = project_sidebar_surface_wash(&acrylic_state, (0.05, 0.05, 0.05), true);
    assert!(acrylic_wash.3 < 1.0);
    assert!(acrylic_wash.3 > 0.5);

    let mica_state = project_sidebar_material(
        SidebarMaterialKind::WindowsMica,
        false,
        true,
        DesktopPlatformKind::Windows,
        Some(22000),
    );
    assert_eq!(
        mica_state.effective_material,
        SidebarMaterialKind::WindowsMica
    );
    let mica_wash = project_sidebar_surface_wash(&mica_state, (0.05, 0.05, 0.05), true);
    assert!(mica_wash.3 < 0.3);
}
