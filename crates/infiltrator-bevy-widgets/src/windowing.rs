//! Multi-window management, Picture-in-Picture (PiP) floating overlays, and dockable panels.

use bevy::ecs::component::Component;
use bevy::ecs::resource::Resource;
use bevy::math::Vec2;
#[cfg(target_os = "linux")]
use std::env::var_os;

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
    /// Share of the slot's primary axis this panel claims, in
    /// `[MIN_DOCK_RATIO, MAX_DOCK_RATIO]`.
    pub split_ratio: f32,
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
            split_ratio: 0.5,
            is_floating: false,
            floating_position: Vec2::new(100.0, 100.0),
            floating_size: Vec2::new(300.0, 200.0),
        }
    }

    /// Set the panel's docked split share, clamped into the legal band.
    pub fn set_split_ratio(&mut self, ratio: f32) {
        self.split_ratio = clamp_dock_ratio(ratio);
    }
}

/// Smallest share a dock split may assign to its leading region.
pub const MIN_DOCK_RATIO: f32 = 0.15;
/// Largest share a dock split may assign to its leading region.
pub const MAX_DOCK_RATIO: f32 = 0.85;

/// Clamp a dock split ratio into `[MIN_DOCK_RATIO, MAX_DOCK_RATIO]`; a
/// non-finite input resolves to the balanced `0.5`.
pub fn clamp_dock_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.clamp(MIN_DOCK_RATIO, MAX_DOCK_RATIO)
    } else {
        0.5
    }
}

/// Orientation of a dock split: `Horizontal` divides left/right,
/// `Vertical` divides top/bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

/// One resizable dock split with a clamped leading-region ratio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockSplit {
    pub axis: SplitAxis,
    pub ratio: f32,
}

impl DockSplit {
    pub fn new(axis: SplitAxis, ratio: f32) -> Self {
        Self {
            axis,
            ratio: clamp_dock_ratio(ratio),
        }
    }

    /// Re-clamp the split share after a drag.
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = clamp_dock_ratio(ratio);
    }

    /// Leading (left/top) extent in pixels for a total span.
    pub fn leading_extent(&self, total: f32) -> f32 {
        total.max(0.0) * self.ratio
    }

    /// Trailing (right/bottom) extent in pixels for a total span.
    pub fn trailing_extent(&self, total: f32) -> f32 {
        total.max(0.0) * (1.0 - self.ratio)
    }
}

/// Pixel extents of the four dock bands for one surface size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DockBandExtents {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

/// Dock layout resource: one left/right column split and one top/bottom row
/// split over the primary surface.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct DockLayout {
    pub columns: DockSplit,
    pub rows: DockSplit,
}

impl Default for DockLayout {
    fn default() -> Self {
        Self {
            columns: DockSplit::new(SplitAxis::Horizontal, 0.25),
            rows: DockSplit::new(SplitAxis::Vertical, 0.2),
        }
    }
}

impl DockLayout {
    /// Pixel extents of the four edge bands for a given surface size.
    pub fn band_extents(&self, surface: Vec2) -> DockBandExtents {
        DockBandExtents {
            left: self.columns.leading_extent(surface.x),
            right: self.columns.trailing_extent(surface.x),
            top: self.rows.leading_extent(surface.y),
            bottom: self.rows.trailing_extent(surface.y),
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
    /// Legacy boolean snap: `true` pins to the top-right corner, `false` to
    /// the bottom-left, keeping the original two-corner contract.
    pub fn snap_to_corner(&mut self, screen_size: Vec2, top_right: bool) {
        let corner = if top_right {
            PipCorner::TopRight
        } else {
            PipCorner::BottomLeft
        };
        self.snap_to(screen_size, corner);
    }

    /// Snap the overlay to any of the four screen corners, keeping a
    /// [`PIP_SCREEN_MARGIN`] gutter and never crossing the top/left edge.
    pub fn snap_to(&mut self, screen_size: Vec2, corner: PipCorner) {
        self.position = corner_position(screen_size, self.size, corner);
    }

    /// Force always-on-top on.
    pub fn pin(&mut self) {
        self.is_pinned_top = true;
    }

    /// Release always-on-top.
    pub fn unpin(&mut self) {
        self.is_pinned_top = false;
    }

    /// Flip always-on-top.
    pub fn toggle_pin(&mut self) {
        self.is_pinned_top = !self.is_pinned_top;
    }

    /// Set whether pointer events pass through the overlay.
    pub fn set_click_through(&mut self, enabled: bool) {
        self.is_click_through = enabled;
    }

    /// Flip click-through.
    pub fn toggle_click_through(&mut self) {
        self.is_click_through = !self.is_click_through;
    }
}

/// Gutter kept between a snapped PiP overlay and the screen edge.
pub const PIP_SCREEN_MARGIN: f32 = 20.0;

/// Screen corner a PiP overlay snaps to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

fn corner_position(screen: Vec2, size: Vec2, corner: PipCorner) -> Vec2 {
    let right = (screen.x - size.x - PIP_SCREEN_MARGIN).max(PIP_SCREEN_MARGIN);
    let bottom = (screen.y - size.y - PIP_SCREEN_MARGIN).max(PIP_SCREEN_MARGIN);
    match corner {
        PipCorner::TopLeft => Vec2::new(PIP_SCREEN_MARGIN, PIP_SCREEN_MARGIN),
        PipCorner::TopRight => Vec2::new(right, PIP_SCREEN_MARGIN),
        PipCorner::BottomLeft => Vec2::new(PIP_SCREEN_MARGIN, bottom),
        PipCorner::BottomRight => Vec2::new(right, bottom),
    }
}

/// Maximum number of simultaneously registered secondary OS windows.
pub const MAX_SECONDARY_WINDOWS: usize = 8;

/// Stable identifier assigned to a secondary OS window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SecondaryWindowId(pub u64);

/// Descriptor of a secondary OS window (detached panel or PiP overlay).
#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryWindowDescriptor {
    pub id: SecondaryWindowId,
    pub title: String,
    pub position: Vec2,
    pub size: Vec2,
    pub always_on_top: bool,
    pub click_through: bool,
}

impl SecondaryWindowDescriptor {
    pub fn new(id: SecondaryWindowId, title: impl Into<String>, size: Vec2) -> Self {
        Self {
            id,
            title: title.into(),
            position: Vec2::ZERO,
            size,
            always_on_top: false,
            click_through: false,
        }
    }
}

/// Bounded registry of live secondary windows.
///
/// [`WindowRegistry::register`] refuses to grow past
/// [`MAX_SECONDARY_WINDOWS`], and [`WindowRegistry::remove`] retires the
/// descriptor so a closed window can never be addressed again.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct WindowRegistry {
    windows: Vec<SecondaryWindowDescriptor>,
    next_id: u64,
}

impl WindowRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.windows.len() >= MAX_SECONDARY_WINDOWS
    }

    pub fn contains(&self, id: SecondaryWindowId) -> bool {
        self.get(id).is_some()
    }

    pub fn get(&self, id: SecondaryWindowId) -> Option<&SecondaryWindowDescriptor> {
        self.windows.iter().find(|window| window.id == id)
    }

    pub fn get_mut(&mut self, id: SecondaryWindowId) -> Option<&mut SecondaryWindowDescriptor> {
        self.windows.iter_mut().find(|window| window.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &SecondaryWindowDescriptor> {
        self.windows.iter()
    }

    /// Register a new window; `None` once the registry is at capacity.
    pub fn register(&mut self, title: impl Into<String>, size: Vec2) -> Option<SecondaryWindowId> {
        if self.is_full() {
            return None;
        }
        let id = SecondaryWindowId(self.next_id);
        self.next_id += 1;
        self.windows
            .push(SecondaryWindowDescriptor::new(id, title, size));
        Some(id)
    }

    /// Retire a window's descriptor, returning it when it existed.
    pub fn remove(&mut self, id: SecondaryWindowId) -> Option<SecondaryWindowDescriptor> {
        let index = self.windows.iter().position(|window| window.id == id)?;
        Some(self.windows.remove(index))
    }
}

/// Typed lifecycle transition of a secondary window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowLifecycle {
    Spawned(SecondaryWindowId),
    Moved(SecondaryWindowId),
    Closed(SecondaryWindowId),
}

/// PiP overlay session: binds the overlay state machine to its registry
/// window and records the typed lifecycle. Closing always retires the
/// descriptor, so the session never leaves an orphaned window behind.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct PipOverlaySession {
    pub state: PipOverlayState,
    pub window: Option<SecondaryWindowId>,
    pub last_event: Option<WindowLifecycle>,
}

impl PipOverlaySession {
    pub fn new(state: PipOverlayState) -> Self {
        Self {
            state,
            window: None,
            last_event: None,
        }
    }

    /// Open the PiP window, mirroring the overlay's size and flags onto the
    /// descriptor. Returns the existing window when already open, `None`
    /// when the registry is at capacity.
    pub fn open(
        &mut self,
        registry: &mut WindowRegistry,
        title: impl Into<String>,
    ) -> Option<SecondaryWindowId> {
        if let Some(id) = self.window {
            return Some(id);
        }
        let id = registry.register(title, self.state.size)?;
        if let Some(descriptor) = registry.get_mut(id) {
            descriptor.position = self.state.position;
            descriptor.always_on_top = self.state.is_pinned_top;
            descriptor.click_through = self.state.is_click_through;
        }
        self.state.is_active = true;
        self.window = Some(id);
        self.last_event = Some(WindowLifecycle::Spawned(id));
        Some(id)
    }

    /// Move the overlay, mirroring the position onto its descriptor.
    pub fn move_to(&mut self, registry: &mut WindowRegistry, position: Vec2) -> bool {
        self.state.position = position;
        let Some(id) = self.window else {
            return false;
        };
        if let Some(descriptor) = registry.get_mut(id) {
            descriptor.position = position;
            self.last_event = Some(WindowLifecycle::Moved(id));
            true
        } else {
            // The descriptor was retired underneath us: drop the handle.
            self.window = None;
            false
        }
    }

    /// Snap the overlay to a corner and mirror the new position.
    pub fn snap(&mut self, registry: &mut WindowRegistry, screen: Vec2, corner: PipCorner) -> bool {
        self.state.snap_to(screen, corner);
        self.move_to(registry, self.state.position)
    }

    /// Flip always-on-top, mirroring it onto the descriptor.
    pub fn toggle_pin(&mut self, registry: &mut WindowRegistry) -> bool {
        self.state.toggle_pin();
        self.sync_flags(registry)
    }

    /// Flip click-through, mirroring it onto the descriptor.
    pub fn toggle_click_through(&mut self, registry: &mut WindowRegistry) -> bool {
        self.state.toggle_click_through();
        self.sync_flags(registry)
    }

    /// Close the PiP window, retiring its descriptor.
    pub fn close(&mut self, registry: &mut WindowRegistry) -> Option<WindowLifecycle> {
        let id = self.window.take()?;
        registry.remove(id);
        self.state.is_active = false;
        let event = WindowLifecycle::Closed(id);
        self.last_event = Some(event);
        Some(event)
    }

    fn sync_flags(&mut self, registry: &mut WindowRegistry) -> bool {
        let Some(id) = self.window else {
            return false;
        };
        if let Some(descriptor) = registry.get_mut(id) {
            descriptor.always_on_top = self.state.is_pinned_top;
            descriptor.click_through = self.state.is_click_through;
            true
        } else {
            self.window = None;
            false
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
    /// Linux Wayland compositor translucent background with dynamic blur & CSD curvature.
    WaylandBlur,
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
            WindowBackdropMaterial::WaylandBlur => {
                if os.eq_ignore_ascii_case("linux") {
                    (WindowBackdropMaterial::WaylandBlur, 0.82)
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
            } else if os.eq_ignore_ascii_case("linux") {
                12.0
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

    /// Whether the resolved window backdrop is active on a Linux Wayland environment.
    pub fn is_wayland(&self) -> bool {
        self.material == WindowBackdropMaterial::WaylandBlur
    }

    /// Automatically resolve the best matching window backdrop material for the active host environment.
    pub fn resolve_for_host(dark_mode: bool) -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::for_platform(
                WindowBackdropMaterial::Mica,
                "windows",
                Some(22000),
                dark_mode,
            )
        }
        #[cfg(target_os = "macos")]
        {
            Self::for_platform(WindowBackdropMaterial::Vibrancy, "macos", None, dark_mode)
        }
        #[cfg(target_os = "linux")]
        {
            let is_wayland = var_os("WAYLAND_DISPLAY").is_some()
                || var_os("XDG_SESSION_TYPE")
                    .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("wayland"));
            if is_wayland {
                Self::for_platform(
                    WindowBackdropMaterial::WaylandBlur,
                    "linux",
                    None,
                    dark_mode,
                )
            } else {
                Self::for_platform(WindowBackdropMaterial::Opaque, "linux", None, dark_mode)
            }
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Self::for_platform(WindowBackdropMaterial::Opaque, "unknown", None, dark_mode)
        }
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

        // Linux WaylandBlur
        let wayland_blur = WindowBackdropSpec::for_platform(
            WindowBackdropMaterial::WaylandBlur,
            "linux",
            None,
            true,
        );
        assert_eq!(wayland_blur.material, WindowBackdropMaterial::WaylandBlur);
        assert_eq!(wayland_blur.corner_radius, 12.0);
        assert!(wayland_blur.is_translucent());
        assert!(wayland_blur.is_wayland());

        // Linux falls back to Opaque for Windows-specific Mica
        let linux_fallback =
            WindowBackdropSpec::for_platform(WindowBackdropMaterial::Mica, "linux", None, true);
        assert_eq!(linux_fallback.material, WindowBackdropMaterial::Opaque);
        assert!(!linux_fallback.is_translucent());
        assert!(!linux_fallback.is_wayland());

        // Host resolution doesn't panic
        let host_spec = WindowBackdropSpec::resolve_for_host(true);
        assert!(host_spec.corner_radius >= 8.0);
    }

    #[test]
    fn dock_split_ratio_math_is_clamped() {
        let mut split = DockSplit::new(SplitAxis::Horizontal, 0.3);
        assert!((split.leading_extent(1000.0) - 300.0).abs() < 1e-4);
        assert!((split.trailing_extent(1000.0) - 700.0).abs() < 1e-4);

        split.set_ratio(0.0);
        assert_eq!(split.ratio, MIN_DOCK_RATIO);
        split.set_ratio(2.0);
        assert_eq!(split.ratio, MAX_DOCK_RATIO);

        assert_eq!(clamp_dock_ratio(f32::NAN), 0.5);
        assert_eq!(clamp_dock_ratio(-1.0), MIN_DOCK_RATIO);
    }

    #[test]
    fn dock_layout_band_extents_follow_ratios() {
        let bands = DockLayout::default().band_extents(Vec2::new(1000.0, 800.0));
        assert!((bands.left - 250.0).abs() < 1e-4);
        assert!((bands.right - 750.0).abs() < 1e-4);
        assert!((bands.top - 160.0).abs() < 1e-4);
        assert!((bands.bottom - 640.0).abs() < 1e-4);
    }

    #[test]
    fn dock_panel_split_ratio_is_clamped() {
        let mut panel = DockPanel::new("p1", "Diagnostics", DockSlot::Right);
        assert_eq!(panel.split_ratio, 0.5);
        panel.set_split_ratio(0.99);
        assert_eq!(panel.split_ratio, MAX_DOCK_RATIO);
        panel.set_split_ratio(f32::INFINITY);
        assert_eq!(panel.split_ratio, 0.5);
    }

    #[test]
    fn window_registry_is_bounded_and_retires_descriptors() {
        let mut registry = WindowRegistry::new();
        let mut ids = Vec::new();
        for _ in 0..MAX_SECONDARY_WINDOWS {
            let id = registry
                .register("panel", Vec2::new(320.0, 240.0))
                .expect("registry has capacity");
            ids.push(id);
        }
        assert_eq!(registry.len(), MAX_SECONDARY_WINDOWS);
        assert!(registry.is_full());
        assert!(
            registry
                .register("overflow", Vec2::new(10.0, 10.0))
                .is_none()
        );

        let removed = registry.remove(ids[0]).expect("descriptor retired");
        assert_eq!(removed.id, ids[0]);
        assert!(!registry.contains(ids[0]));
        assert_eq!(registry.len(), MAX_SECONDARY_WINDOWS - 1);
        assert!(!registry.is_full());

        let replacement = registry
            .register("replacement", Vec2::new(10.0, 10.0))
            .expect("freed slot reused");
        assert!(!ids.contains(&replacement));
        assert!(registry.remove(SecondaryWindowId(9999)).is_none());
        assert_eq!(registry.iter().count(), MAX_SECONDARY_WINDOWS);
    }

    #[test]
    fn secondary_window_descriptor_carries_placement_flags() {
        let mut registry = WindowRegistry::new();
        let id = registry
            .register("PiP", Vec2::new(180.0, 64.0))
            .expect("registered");
        let descriptor = registry.get_mut(id).expect("descriptor present");
        descriptor.position = Vec2::new(40.0, 60.0);
        descriptor.always_on_top = true;
        descriptor.click_through = true;

        let stored = registry.get(id).expect("descriptor present");
        assert_eq!(stored.position, Vec2::new(40.0, 60.0));
        assert_eq!(stored.size, Vec2::new(180.0, 64.0));
        assert!(stored.always_on_top);
        assert!(stored.click_through);
    }

    #[test]
    fn pip_snap_places_overlay_in_each_corner() {
        let mut pip = PipOverlayState::default();
        let screen = Vec2::new(1920.0, 1080.0);

        pip.snap_to(screen, PipCorner::TopLeft);
        assert_eq!(
            pip.position,
            Vec2::new(PIP_SCREEN_MARGIN, PIP_SCREEN_MARGIN)
        );
        pip.snap_to(screen, PipCorner::TopRight);
        assert_eq!(
            pip.position,
            Vec2::new(1920.0 - 180.0 - PIP_SCREEN_MARGIN, PIP_SCREEN_MARGIN)
        );
        pip.snap_to(screen, PipCorner::BottomLeft);
        assert_eq!(
            pip.position,
            Vec2::new(PIP_SCREEN_MARGIN, 1080.0 - 64.0 - PIP_SCREEN_MARGIN)
        );
        pip.snap_to(screen, PipCorner::BottomRight);
        assert_eq!(
            pip.position,
            Vec2::new(
                1920.0 - 180.0 - PIP_SCREEN_MARGIN,
                1080.0 - 64.0 - PIP_SCREEN_MARGIN
            )
        );

        // Legacy boolean snap keeps the original top-right / bottom-left contract.
        pip.snap_to_corner(screen, true);
        assert_eq!(pip.position, Vec2::new(1920.0 - 180.0 - 20.0, 20.0));
        pip.snap_to_corner(screen, false);
        assert_eq!(pip.position, Vec2::new(20.0, 1080.0 - 64.0 - 20.0));
    }

    #[test]
    fn pip_pin_and_click_through_toggle_mirror_to_descriptor() {
        let mut registry = WindowRegistry::new();
        let mut session = PipOverlaySession::new(PipOverlayState::default());
        let id = session.open(&mut registry, "Mini HUD").expect("opened");
        assert_eq!(session.last_event, Some(WindowLifecycle::Spawned(id)));
        assert!(registry.get(id).expect("descriptor").always_on_top);

        assert!(session.toggle_pin(&mut registry));
        assert!(!session.state.is_pinned_top);
        assert!(!registry.get(id).expect("descriptor").always_on_top);
        assert!(session.toggle_pin(&mut registry));
        assert!(registry.get(id).expect("descriptor").always_on_top);

        assert!(!session.state.is_click_through);
        assert!(session.toggle_click_through(&mut registry));
        assert!(session.state.is_click_through);
        assert!(registry.get(id).expect("descriptor").click_through);
    }

    #[test]
    fn pip_snap_and_move_update_descriptor() {
        let mut registry = WindowRegistry::new();
        let mut session = PipOverlaySession::new(PipOverlayState::default());
        let id = session.open(&mut registry, "Mini HUD").expect("opened");

        assert!(session.snap(
            &mut registry,
            Vec2::new(1280.0, 720.0),
            PipCorner::BottomRight
        ));
        assert_eq!(session.last_event, Some(WindowLifecycle::Moved(id)));
        assert_eq!(
            registry.get(id).expect("descriptor").position,
            session.state.position
        );

        assert!(session.move_to(&mut registry, Vec2::new(5.0, 6.0)));
        assert_eq!(
            registry.get(id).expect("descriptor").position,
            Vec2::new(5.0, 6.0)
        );
    }

    #[test]
    fn pip_close_retires_descriptor_without_orphan() {
        let mut registry = WindowRegistry::new();
        let mut session = PipOverlaySession::new(PipOverlayState::default());
        let id = session.open(&mut registry, "Mini HUD").expect("opened");
        assert!(registry.contains(id));

        assert_eq!(
            session.close(&mut registry),
            Some(WindowLifecycle::Closed(id))
        );
        assert!(session.window.is_none());
        assert!(!registry.contains(id));
        assert!(!session.state.is_active);
        assert!(session.close(&mut registry).is_none());
    }

    #[test]
    fn pip_session_clears_handle_when_registry_retires_window() {
        let mut registry = WindowRegistry::new();
        let mut session = PipOverlaySession::new(PipOverlayState::default());
        let id = session.open(&mut registry, "Mini HUD").expect("opened");
        registry.remove(id).expect("descriptor retired");

        assert!(!session.move_to(&mut registry, Vec2::new(1.0, 2.0)));
        assert!(session.window.is_none());
        assert!(!session.toggle_pin(&mut registry));
    }

    #[test]
    fn pip_open_respects_registry_capacity() {
        let mut registry = WindowRegistry::new();
        for _ in 0..MAX_SECONDARY_WINDOWS {
            registry
                .register("filler", Vec2::new(10.0, 10.0))
                .expect("registry has capacity");
        }
        let mut session = PipOverlaySession::new(PipOverlayState::default());
        assert!(session.open(&mut registry, "Mini HUD").is_none());
        assert!(session.window.is_none());
        assert_eq!(session.last_event, None);
    }
}
