//! BANDROID-006: typed native window-insets seam.
//!
//! The Android host reads `WindowInsetsCompat`/`WindowInsets`, converts
//! physical pixels to logical coordinates with the real window density, and
//! feeds one [`WindowInsetsInput`] into the shell. This module owns only the
//! typed translation into the existing safe-area / responsive consumer:
//!
//! * permanent system insets (status bar, navigation bar, display cutout) are
//!   projected into the shared [`SafeAreaInsets`] / [`GestureHostReport`] the
//!   shell already consumes;
//! * the transient IME (keyboard) inset is kept separate in
//!   [`KeyboardAvoidance`] so the keyboard pushes content up without
//!   duplicating the permanent bottom margin (`max(nav_bar, ime)`).
//!
//! Bevy 0.20 exposes no `WindowInsets` API of its own, so a native host owns
//! the real read and this module is the typed boundary it writes through.

use crate::gesture::GestureHostReport;
use bevy::app::{App, Plugin};
use bevy::ecs::event::Event;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;
use infiltrator_bevy_widgets::responsive::SafeAreaInsets;
use infiltrator_contract::shell_gesture;

/// One inset group in physical pixels. Negative host values are clamped to
/// zero on read so a malformed platform report cannot pull layout off-screen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgeInsets {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

impl EdgeInsets {
    /// All edges zero.
    pub const ZERO: Self = Self {
        top: 0.0,
        bottom: 0.0,
        left: 0.0,
        right: 0.0,
    };

    /// Construct explicit edge values.
    pub const fn new(top: f32, bottom: f32, left: f32, right: f32) -> Self {
        Self {
            top,
            bottom,
            left,
            right,
        }
    }

    /// Edge-wise maximum; used to merge overlapping permanent sources without
    /// ever adding them together.
    pub fn max_with(self, other: Self) -> Self {
        Self {
            top: self.top.max(other.top),
            bottom: self.bottom.max(other.bottom),
            left: self.left.max(other.left),
            right: self.right.max(other.right),
        }
    }

    /// Whether every edge is zero.
    pub const fn is_zero(self) -> bool {
        self.top == 0.0 && self.bottom == 0.0 && self.left == 0.0 && self.right == 0.0
    }
}

/// Typed native window insets: status bar, navigation bar, display cutout and
/// IME, all in physical pixels, with the real window density for conversion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowInsets {
    pub density: f32,
    pub status_bar: EdgeInsets,
    pub nav_bar: EdgeInsets,
    pub cutout: EdgeInsets,
    pub ime: EdgeInsets,
}

impl Default for WindowInsets {
    fn default() -> Self {
        Self {
            density: 1.0,
            status_bar: EdgeInsets::ZERO,
            nav_bar: EdgeInsets::ZERO,
            cutout: EdgeInsets::ZERO,
            ime: EdgeInsets::ZERO,
        }
    }
}

impl WindowInsets {
    /// Build from physical-pixel edge groups. A non-positive density falls back
    /// to 1.0 so coordinate conversion never divides by zero.
    pub fn physical(
        density: f32,
        status_bar: EdgeInsets,
        nav_bar: EdgeInsets,
        cutout: EdgeInsets,
        ime: EdgeInsets,
    ) -> Self {
        Self {
            density: if density > 0.0 { density } else { 1.0 },
            status_bar: clamp(status_bar),
            nav_bar: clamp(nav_bar),
            cutout: clamp(cutout),
            ime: clamp(ime),
        }
    }

    /// Build from density-independent pixels, converting each edge to physical
    /// pixels with the real window density.
    pub fn from_dp(
        density: f32,
        status_bar: EdgeInsets,
        nav_bar: EdgeInsets,
        cutout: EdgeInsets,
        ime: EdgeInsets,
    ) -> Self {
        let density = if density > 0.0 { density } else { 1.0 };
        Self {
            density,
            status_bar: clamp(scale(status_bar, density)),
            nav_bar: clamp(scale(nav_bar, density)),
            cutout: clamp(scale(cutout, density)),
            ime: clamp(scale(ime, density)),
        }
    }

    /// Convert one physical pixel value to density-independent pixels.
    pub fn to_dp(&self, physical_px: f32) -> f32 {
        physical_px / self.density
    }

    /// Permanent system safe area (status bar, navigation bar, cutout),
    /// independent of the keyboard. Overlapping top sources are merged with a
    /// maximum, never summed.
    pub fn permanent_safe_area(&self) -> SafeAreaInsets {
        let top = self.status_bar.top.max(self.cutout.top);
        let bottom = self.nav_bar.bottom.max(self.cutout.bottom);
        let left = self
            .status_bar
            .left
            .max(self.nav_bar.left)
            .max(self.cutout.left);
        let right = self
            .status_bar
            .right
            .max(self.nav_bar.right)
            .max(self.cutout.right);
        SafeAreaInsets::new(top, bottom, left, right)
    }

    /// Transient keyboard avoidance height at the bottom edge.
    pub fn keyboard_bottom(&self) -> f32 {
        self.ime.bottom.max(0.0)
    }

    /// The layout avoidance the responsive consumer applies: the permanent safe
    /// area with the bottom edge widened to the keyboard when the IME is up.
    /// The permanent bottom margin is never added on top of the IME inset, so
    /// there is no duplicate margin.
    pub fn layout_avoidance(&self) -> SafeAreaInsets {
        let permanent = self.permanent_safe_area();
        let bottom = permanent.bottom_px.max(self.keyboard_bottom());
        SafeAreaInsets::new(
            permanent.top_px,
            bottom,
            permanent.left_px,
            permanent.right_px,
        )
    }
}

fn clamp(edges: EdgeInsets) -> EdgeInsets {
    EdgeInsets {
        top: edges.top.max(0.0),
        bottom: edges.bottom.max(0.0),
        left: edges.left.max(0.0),
        right: edges.right.max(0.0),
    }
}

fn scale(edges: EdgeInsets, factor: f32) -> EdgeInsets {
    EdgeInsets {
        top: edges.top * factor,
        bottom: edges.bottom * factor,
        left: edges.left * factor,
        right: edges.right * factor,
    }
}

/// The latest native insets reported by the host.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct HostWindowInsets(pub WindowInsets);

/// Transient keyboard avoidance the responsive consumer adds on top of the
/// permanent safe area without duplicating it.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct KeyboardAvoidance {
    pub bottom_px: f32,
}

/// Typed insets input from the native host.
#[derive(Event, Clone, Copy, Debug, PartialEq)]
pub struct WindowInsetsInput(pub WindowInsets);

/// Project one insets report onto the existing safe-area consumers.
fn apply_window_insets(
    input: On<WindowInsetsInput>,
    mut host: ResMut<HostWindowInsets>,
    mut safe_area: Option<ResMut<SafeAreaInsets>>,
    mut gesture: Option<ResMut<GestureHostReport>>,
    mut keyboard: ResMut<KeyboardAvoidance>,
) {
    let insets = input.0;
    if host.0 == insets {
        return;
    }
    host.0 = insets;

    let permanent = insets.permanent_safe_area();
    if let Some(safe_area) = safe_area.as_deref_mut()
        && *safe_area != permanent
    {
        *safe_area = permanent;
    }
    if let Some(gesture) = gesture.as_deref_mut() {
        let shared = shell_gesture::SafeAreaInsets::new(
            permanent.top_px,
            permanent.right_px,
            permanent.bottom_px,
            permanent.left_px,
        );
        if gesture.insets != shared {
            gesture.insets = shared;
        }
    }
    let bottom = insets.keyboard_bottom();
    if keyboard.bottom_px != bottom {
        keyboard.bottom_px = bottom;
    }
}

/// Installs the typed native insets consumer.
pub struct HostInsetsPlugin;

impl Plugin for HostInsetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HostWindowInsets>()
            .init_resource::<KeyboardAvoidance>()
            .add_observer(apply_window_insets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top(px: f32) -> EdgeInsets {
        EdgeInsets::new(px, 0.0, 0.0, 0.0)
    }

    fn bottom(px: f32) -> EdgeInsets {
        EdgeInsets::new(0.0, px, 0.0, 0.0)
    }

    #[test]
    fn permanent_safe_area_excludes_the_keyboard() {
        let insets = WindowInsets::physical(
            3.0,
            top(72.0),
            bottom(144.0),
            EdgeInsets::ZERO,
            bottom(900.0),
        );
        let permanent = insets.permanent_safe_area();
        assert_eq!(permanent.top_px, 72.0);
        assert_eq!(
            permanent.bottom_px, 144.0,
            "the permanent area never contains the IME"
        );
        assert_eq!(insets.keyboard_bottom(), 900.0);
        assert_eq!(insets.to_dp(72.0), 24.0);
    }

    #[test]
    fn layout_avoidance_never_double_counts_the_bottom_margin() {
        let insets = WindowInsets::physical(
            1.0,
            top(24.0),
            bottom(48.0),
            EdgeInsets::ZERO,
            bottom(300.0),
        );
        let avoidance = insets.layout_avoidance();
        assert_eq!(avoidance.top_px, 24.0);
        assert_eq!(
            avoidance.bottom_px, 300.0,
            "the IME replaces the nav bar margin"
        );
        assert_ne!(
            avoidance.bottom_px, 348.0,
            "nav bar + IME must not be summed"
        );

        let hidden = WindowInsets::physical(
            1.0,
            top(24.0),
            bottom(48.0),
            EdgeInsets::ZERO,
            EdgeInsets::ZERO,
        );
        assert_eq!(hidden.layout_avoidance().bottom_px, 48.0);
    }

    #[test]
    fn cutout_and_status_bar_share_the_top_edge() {
        let insets = WindowInsets::physical(
            1.0,
            top(24.0),
            EdgeInsets::ZERO,
            EdgeInsets::new(30.0, 0.0, 0.0, 0.0),
            EdgeInsets::ZERO,
        );
        assert_eq!(insets.permanent_safe_area().top_px, 30.0);
        assert_ne!(insets.permanent_safe_area().top_px, 54.0);
    }

    #[test]
    fn dp_inputs_scale_by_the_window_density() {
        let insets = WindowInsets::from_dp(
            3.0,
            top(24.0),
            bottom(16.0),
            EdgeInsets::ZERO,
            EdgeInsets::ZERO,
        );
        assert_eq!(insets.status_bar.top, 72.0);
        assert_eq!(insets.nav_bar.bottom, 48.0);
        assert_eq!(insets.to_dp(insets.status_bar.top), 24.0);
    }

    #[test]
    fn plugin_projects_permanent_insets_and_separates_keyboard_avoidance() {
        let mut app = App::new();
        app.init_resource::<SafeAreaInsets>();
        app.init_resource::<GestureHostReport>();
        app.add_plugins(HostInsetsPlugin);

        let portrait = WindowInsets::physical(
            1.0,
            top(24.0),
            bottom(48.0),
            EdgeInsets::ZERO,
            EdgeInsets::ZERO,
        );
        app.world_mut().trigger(WindowInsetsInput(portrait));
        app.update();

        let safe = app.world().resource::<SafeAreaInsets>();
        assert_eq!(safe.top_px, 24.0);
        assert_eq!(safe.bottom_px, 48.0);
        let report = app.world().resource::<GestureHostReport>();
        assert_eq!(report.insets.top, 24.0);
        assert_eq!(report.insets.bottom, 48.0);
        assert_eq!(app.world().resource::<KeyboardAvoidance>().bottom_px, 0.0);

        // Keyboard opens: the permanent safe area is unchanged, the transient
        // avoidance grows to the IME height only.
        let typing = WindowInsets::physical(
            1.0,
            top(24.0),
            bottom(48.0),
            EdgeInsets::ZERO,
            bottom(300.0),
        );
        app.world_mut().trigger(WindowInsetsInput(typing));
        app.update();
        assert_eq!(app.world().resource::<SafeAreaInsets>().bottom_px, 48.0);
        assert_eq!(app.world().resource::<KeyboardAvoidance>().bottom_px, 300.0);

        // Rotation: the cutout moves to the horizontal edges live.
        let landscape = WindowInsets::physical(
            1.0,
            top(24.0),
            bottom(48.0),
            EdgeInsets::new(0.0, 0.0, 60.0, 60.0),
            EdgeInsets::ZERO,
        );
        app.world_mut().trigger(WindowInsetsInput(landscape));
        app.update();
        let safe = app.world().resource::<SafeAreaInsets>();
        assert_eq!(safe.left_px, 60.0);
        assert_eq!(safe.right_px, 60.0);
        assert_eq!(app.world().resource::<KeyboardAvoidance>().bottom_px, 0.0);
    }

    #[test]
    fn repeating_the_same_report_is_inert() {
        let mut app = App::new();
        app.init_resource::<SafeAreaInsets>();
        app.add_plugins(HostInsetsPlugin);
        let report = WindowInsets::physical(
            1.0,
            top(24.0),
            EdgeInsets::ZERO,
            EdgeInsets::ZERO,
            EdgeInsets::ZERO,
        );
        app.world_mut().trigger(WindowInsetsInput(report));
        app.update();
        app.world_mut().trigger(WindowInsetsInput(report));
        app.update();
        assert_eq!(app.world().resource::<HostWindowInsets>().0, report);
    }
}
