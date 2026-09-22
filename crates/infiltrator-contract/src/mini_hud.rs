//! Shared Mini HUD read model and placement geometry.
//!
//! The floating 260x90 speed HUD shows live facts only: the duplex rates come
//! from the traffic projection, the exit node from the active-exit snapshot,
//! the two quick-switch states from the shared [`SystemToggleSnapshot`], and
//! the mode from the core. Both surfaces build the same
//! [`MiniHudReadModel`] and render the same status line — no surface bakes
//! "RULE" or "系统代理: 开启" into a scene.
//!
//! Placement is a persisted setting. The pure snap/clamp math lives here so a
//! surface only needs a display rectangle plus a host port to apply it; the
//! host port reports a typed unsupported when the platform cannot move the
//! HUD window (see [`MiniHudHostOutcome`]).

use crate::system_toggle::{SystemToggle, SystemToggleSnapshot, SystemToggleState};
use serde::{Deserialize, Serialize};

/// Persisted Mini HUD placement (logical pixels, screen coordinates).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MiniHudPlacement {
    pub x: i32,
    pub y: i32,
    pub pinned: bool,
}

impl Default for MiniHudPlacement {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            pinned: false,
        }
    }
}

impl MiniHudPlacement {
    /// The 260x90 floating window's logical size.
    pub const WINDOW_WIDTH: u32 = 260;
    pub const WINDOW_HEIGHT: u32 = 90;

    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            pinned: false,
        }
    }

    pub fn with_pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    /// The placement's window rectangle.
    pub fn window(self) -> MiniHudWindow {
        MiniHudWindow {
            x: self.x,
            y: self.y,
            width: Self::WINDOW_WIDTH,
            height: Self::WINDOW_HEIGHT,
        }
    }

    /// Constrain the placement (and its top-left corner) into a display so a
    /// HUD whose stored coordinates are off-screen is recoverable.
    pub fn clamped_to(self, display: MiniHudDisplay) -> Self {
        let window = MiniHudGeometry::constrain(self.window(), display);
        Self {
            x: window.x,
            y: window.y,
            ..self
        }
    }

    /// Snap the placement to the display edges within `threshold` pixels.
    pub fn snapped_to_edges(self, display: MiniHudDisplay, threshold: u32) -> MiniHudSnapPlacement {
        let snap = MiniHudGeometry::snap_to_edges(self.window(), display, threshold);
        MiniHudSnapPlacement {
            placement: Self {
                x: snap.window.x,
                y: snap.window.y,
                ..self
            },
            snapped_left: snap.snapped_left,
            snapped_right: snap.snapped_right,
            snapped_top: snap.snapped_top,
            snapped_bottom: snap.snapped_bottom,
        }
    }
}

/// A display rectangle in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniHudDisplay {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl MiniHudDisplay {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn right(self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }

    pub const fn bottom(self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }

    pub const fn contains_point(self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    /// The display whose rectangle holds the point, if any.
    pub fn display_for_point(displays: &[Self], px: i32, py: i32) -> Option<usize> {
        displays
            .iter()
            .position(|display| display.contains_point(px, py))
    }
}

/// A floating window rectangle in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniHudWindow {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl MiniHudWindow {
    pub const fn right(self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }

    pub const fn bottom(self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }
}

/// A snap outcome: the (possibly moved) window plus which edges caught.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniHudSnap {
    pub window: MiniHudWindow,
    pub snapped_left: bool,
    pub snapped_right: bool,
    pub snapped_top: bool,
    pub snapped_bottom: bool,
}

/// The persisted placement plus its snap flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiniHudSnapPlacement {
    pub placement: MiniHudPlacement,
    pub snapped_left: bool,
    pub snapped_right: bool,
    pub snapped_top: bool,
    pub snapped_bottom: bool,
}

impl MiniHudSnapPlacement {
    pub const fn has_snapped(self) -> bool {
        self.snapped_left || self.snapped_right || self.snapped_top || self.snapped_bottom
    }
}

/// Pure window geometry for the Mini HUD.
pub struct MiniHudGeometry;

impl MiniHudGeometry {
    /// Keep the window fully inside the display when it is larger than the
    /// display it was constrained to; otherwise clamp the top-left corner.
    pub fn constrain(window: MiniHudWindow, display: MiniHudDisplay) -> MiniHudWindow {
        let max_x = display.right().saturating_sub(window.width as i32);
        let x = if window.x < display.x {
            display.x
        } else if window.x > max_x {
            max_x.max(display.x)
        } else {
            window.x
        };
        let max_y = display.bottom().saturating_sub(window.height as i32);
        let y = if window.y < display.y {
            display.y
        } else if window.y > max_y {
            max_y.max(display.y)
        } else {
            window.y
        };
        MiniHudWindow { x, y, ..window }
    }

    /// Snap a window to the display edges within `threshold` logical pixels.
    pub fn snap_to_edges(
        window: MiniHudWindow,
        display: MiniHudDisplay,
        threshold: u32,
    ) -> MiniHudSnap {
        let t = threshold as i32;
        let mut x = window.x;
        let mut y = window.y;
        let (mut left, mut right, mut top, mut bottom) = (false, false, false, false);

        if (window.x - display.x).abs() <= t {
            x = display.x;
            left = true;
        } else if (window.right() - display.right()).abs() <= t {
            x = display.right() - window.width as i32;
            right = true;
        }

        if (window.y - display.y).abs() <= t {
            y = display.y;
            top = true;
        } else if (window.bottom() - display.bottom()).abs() <= t {
            y = display.bottom() - window.height as i32;
            bottom = true;
        }

        MiniHudSnap {
            window: MiniHudWindow { x, y, ..window },
            snapped_left: left,
            snapped_right: right,
            snapped_top: top,
            snapped_bottom: bottom,
        }
    }
}

/// The live facts the Mini HUD renders. Every field is a real projection
/// value; an unknown toggle stays `Unknown` and renders as `—` through the
/// shared [`SystemToggleState::compact_label`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MiniHudReadModel {
    pub visible: bool,
    pub mode_zh: String,
    pub exit_node: String,
    pub down_bytes_per_sec: u64,
    pub up_bytes_per_sec: u64,
    pub system_proxy: SystemToggleState,
    pub tun: SystemToggleState,
    pub placement: MiniHudPlacement,
}

impl MiniHudReadModel {
    /// The shared status line both surfaces render under the exit-node pill:
    /// the state letters come from the shared compact labels, so Iced and
    /// Bevy cannot drift apart.
    pub fn status_line(&self) -> String {
        format!(
            "系统代理: {} · TUN: {}",
            self.system_proxy.compact_label(),
            self.tun.compact_label()
        )
    }

    /// The desired value of the next quick-toggle press, or `None` while the
    /// toggle is not in a state that accepts a user action.
    pub fn next_value(&self, toggle: SystemToggle) -> Option<bool> {
        let state = match toggle {
            SystemToggle::SystemProxy => &self.system_proxy,
            SystemToggle::Tun => &self.tun,
        };
        if state.can_toggle() {
            Some(!state.is_enabled())
        } else {
            None
        }
    }

    /// Rebuild the two toggle states from the shared snapshot.
    pub fn with_toggles(mut self, snapshot: &SystemToggleSnapshot) -> Self {
        self.system_proxy = snapshot.system_proxy.clone();
        self.tun = snapshot.tun.clone();
        self
    }

    pub fn with_traffic(mut self, up_bytes_per_sec: u64, down_bytes_per_sec: u64) -> Self {
        self.up_bytes_per_sec = up_bytes_per_sec;
        self.down_bytes_per_sec = down_bytes_per_sec;
        self
    }

    pub fn with_exit_node(mut self, node: impl Into<String>) -> Self {
        self.exit_node = node.into();
        self
    }

    pub fn with_mode(mut self, mode_zh: impl Into<String>) -> Self {
        self.mode_zh = mode_zh.into();
        self
    }

    pub fn with_placement(mut self, placement: MiniHudPlacement) -> Self {
        self.placement = placement;
        self
    }
}

/// What a host reports after being asked to apply a HUD placement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MiniHudHostOutcome {
    /// The host moved (or pinned) the floating window.
    Applied,
    /// The host owns no floating HUD window yet; the persisted placement is
    /// stored but cannot be projected onto a real window.
    Unsupported { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fhd() -> MiniHudDisplay {
        MiniHudDisplay::new(0, 0, 1920, 1080)
    }

    #[test]
    fn snap_catches_each_edge_within_threshold() {
        assert_eq!(
            MiniHudPlacement::new(8, 12).snapped_to_edges(fhd(), 16),
            MiniHudSnapPlacement {
                placement: MiniHudPlacement::new(0, 0),
                snapped_left: true,
                snapped_right: false,
                snapped_top: true,
                snapped_bottom: false,
            }
        );
        let far_corner = MiniHudPlacement::new(1920 - 260 - 8, 1080 - 90 - 12);
        let snap = far_corner.snapped_to_edges(fhd(), 16);
        assert!(snap.snapped_right && snap.snapped_bottom);
        assert_eq!(snap.placement.x, 1920 - 260);
        assert_eq!(snap.placement.y, 1080 - 90);

        let free = MiniHudPlacement::new(500, 300).snapped_to_edges(fhd(), 16);
        assert!(!free.has_snapped() && free.placement == MiniHudPlacement::new(500, 300));
    }

    #[test]
    fn clamping_recovers_an_offscreen_placement() {
        let clamped = MiniHudPlacement::new(-400, 5000).clamped_to(fhd());
        assert_eq!(clamped.x, 0);
        assert_eq!(clamped.y, 1080 - 90);
        let inside = MiniHudPlacement::new(200, 200).clamped_to(fhd());
        assert_eq!(inside, MiniHudPlacement::new(200, 200));
    }

    #[test]
    fn the_read_model_status_line_is_derived_from_real_toggle_states() {
        let snapshot = SystemToggleSnapshot::from_legacy(true, Some(false), 3)
            .with_pending(SystemToggle::Tun, true);
        let model = MiniHudReadModel::default()
            .with_toggles(&snapshot)
            .with_traffic(1024, 2_048)
            .with_exit_node("HK-01")
            .with_mode("规则");
        assert_eq!(model.status_line(), "系统代理: 开 · TUN: …");
        assert_eq!(model.next_value(SystemToggle::SystemProxy), Some(false));
        assert_eq!(
            model.next_value(SystemToggle::Tun),
            None,
            "a pending toggle does not accept a second press"
        );
        assert_eq!(model.exit_node, "HK-01");
    }

    #[test]
    fn placement_round_trips_through_serde_with_defaults() {
        let placement = MiniHudPlacement::new(-12, 640).with_pinned(true);
        let json = serde_json::to_string(&placement).expect("serialize");
        assert_eq!(
            serde_json::from_str::<MiniHudPlacement>(&json).expect("deserialize"),
            placement
        );
        // An older settings file without the key falls back to the default.
        let legacy: MiniHudPlacement = serde_json::from_str("{}").expect("legacy default");
        assert_eq!(legacy, MiniHudPlacement::default());
    }
}
