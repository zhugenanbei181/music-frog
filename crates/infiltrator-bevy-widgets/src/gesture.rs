//! Touch gesture recognition and safe-area insets for mobile/desktop.
//!
//! **Pure State Core**: [`GestureRecognizer`], [`GestureState`], [`SwipeAction`], [`PullToRefreshState`],
//! and [`SafeAreaInsets`] handle touch gestures and edge-to-edge layout math with zero bevy dependencies.

use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::resource::Resource;
use bevy::math::Vec2;

/// Safe-area insets in logical pixels (status bar, navigation bar, display cutouts/notches).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct SafeAreaInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl SafeAreaInsets {
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

/// Gesture touch pointer event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchPhase {
    Start(Vec2),
    Move(Vec2),
    End(Vec2),
    Cancel,
}

/// Recognized gesture outcome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureOutcome {
    Tap(Vec2),
    DoubleTap(Vec2),
    LongPress(Vec2),
    Swipe { delta: Vec2, velocity: Vec2 },
    Pan { delta: Vec2, current: Vec2 },
    Pinch { scale: f32, center: Vec2 },
}

/// Pure state machine for gesture recognition.
#[derive(Clone, Debug, Default)]
pub struct GestureRecognizer {
    start_pos: Option<Vec2>,
    current_pos: Option<Vec2>,
    start_time_ms: u64,
    last_tap_time_ms: Option<u64>,
    last_tap_pos: Option<Vec2>,
    is_panning: bool,
    threshold_pan_px: f32,
    long_press_duration_ms: u64,
    double_tap_duration_ms: u64,
}

impl GestureRecognizer {
    pub fn new() -> Self {
        Self {
            start_pos: None,
            current_pos: None,
            start_time_ms: 0,
            last_tap_time_ms: None,
            last_tap_pos: None,
            is_panning: false,
            threshold_pan_px: 10.0,
            long_press_duration_ms: 500,
            double_tap_duration_ms: 300,
        }
    }

    pub fn handle_touch(&mut self, phase: TouchPhase, time_ms: u64) -> Option<GestureOutcome> {
        match phase {
            TouchPhase::Start(pos) => {
                self.start_pos = Some(pos);
                self.current_pos = Some(pos);
                self.start_time_ms = time_ms;
                self.is_panning = false;
                None
            }
            TouchPhase::Move(pos) => {
                self.current_pos = Some(pos);
                if let Some(start) = self.start_pos {
                    let delta = pos - start;
                    if !self.is_panning && delta.length() > self.threshold_pan_px {
                        self.is_panning = true;
                    }
                    if self.is_panning {
                        return Some(GestureOutcome::Pan {
                            delta,
                            current: pos,
                        });
                    }
                }
                None
            }
            TouchPhase::End(pos) => {
                let start = self.start_pos.take()?;
                self.current_pos = None;
                let duration = time_ms.saturating_sub(self.start_time_ms);
                let delta = pos - start;

                if self.is_panning {
                    self.is_panning = false;
                    let dt = (duration as f32 / 1000.0).max(0.016);
                    let velocity = delta / dt;
                    if velocity.length() > 300.0 {
                        return Some(GestureOutcome::Swipe { delta, velocity });
                    }
                } else if duration >= self.long_press_duration_ms {
                    return Some(GestureOutcome::LongPress(pos));
                } else {
                    if let (Some(last_time), Some(last_pos)) =
                        (self.last_tap_time_ms, self.last_tap_pos)
                        && time_ms.saturating_sub(last_time) <= self.double_tap_duration_ms
                        && (pos - last_pos).length() < 24.0
                    {
                        self.last_tap_time_ms = None;
                        self.last_tap_pos = None;
                        return Some(GestureOutcome::DoubleTap(pos));
                    }
                    self.last_tap_time_ms = Some(time_ms);
                    self.last_tap_pos = Some(pos);
                    return Some(GestureOutcome::Tap(pos));
                }
                None
            }
            TouchPhase::Cancel => {
                self.start_pos = None;
                self.current_pos = None;
                self.is_panning = false;
                None
            }
        }
    }
}

/// Pull-to-refresh pull state machine.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PullToRefreshState {
    pub pull_offset: f32,
    pub threshold: f32,
    pub is_refreshing: bool,
}

impl PullToRefreshState {
    pub fn new(threshold: f32) -> Self {
        Self {
            pull_offset: 0.0,
            threshold,
            is_refreshing: false,
        }
    }

    pub fn pull(&mut self, dy: f32) {
        if self.is_refreshing {
            return;
        }
        // Quadratic resistance dampening
        let factor = (1.0 - (self.pull_offset / (self.threshold * 2.5)).min(0.8)).max(0.2);
        self.pull_offset = (self.pull_offset + dy * factor).max(0.0);
    }

    pub fn release(&mut self) -> bool {
        if self.pull_offset >= self.threshold {
            self.is_refreshing = true;
            self.pull_offset = self.threshold;
            true
        } else {
            self.pull_offset = 0.0;
            false
        }
    }

    pub fn finish_refresh(&mut self) {
        self.is_refreshing = false;
        self.pull_offset = 0.0;
    }

    pub fn fraction(&self) -> f32 {
        (self.pull_offset / self.threshold).clamp(0.0, 1.0)
    }
}

/// Marker component for items supporting swipe-to-action (left delete / right pin).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct SwipeToActionItem {
    pub offset_x: f32,
    pub max_action_width: f32,
}

impl SwipeToActionItem {
    pub fn new(max_action_width: f32) -> Self {
        Self {
            offset_x: 0.0,
            max_action_width,
        }
    }

    pub fn apply_drag(&mut self, dx: f32) {
        self.offset_x = (self.offset_x + dx).clamp(-self.max_action_width, self.max_action_width);
    }

    pub fn settle(&mut self) {
        if self.offset_x.abs() > self.max_action_width * 0.5 {
            self.offset_x = self.offset_x.signum() * self.max_action_width;
        } else {
            self.offset_x = 0.0;
        }
    }
}

/// Gesture event dispatched to Bevy observers.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct GestureEvent(pub GestureOutcome);

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, JustifyContent, Node, Overflow, percent, px};
use bevy::ui::widget::Text;

/// Component tracking pull-to-refresh UI container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct PullToRefreshIndicator;

/// Construct a pull-to-refresh header indicator scene.
pub fn pull_to_refresh_scene(state: &PullToRefreshState, _palette: &UiPalette) -> Box<dyn Scene> {
    let height = state.pull_offset.min(state.threshold * 1.5);
    let label = if state.is_refreshing {
        "正在更新..."
    } else if state.pull_offset >= state.threshold {
        "释放以刷新"
    } else {
        "下拉刷新"
    };

    Box::new(bsn! {
        Node {
            width: percent(100),
            height: px(height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
        }
        PullToRefreshIndicator
        Children [
            (
                Text({ label.to_owned() })
                TextRole(Role::Caption)
            ),
        ]
    })
}

/// Interactive pinch-to-zoom and two-finger pan controller for waveforms and canvas views.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PinchZoomController {
    pub zoom_level: f32,
    pub min_zoom: f32,
    pub max_zoom: f32,
    pub focal_point: Vec2,
    pub pan_offset: Vec2,
}

impl Default for PinchZoomController {
    fn default() -> Self {
        Self::new()
    }
}

impl PinchZoomController {
    pub fn new() -> Self {
        Self {
            zoom_level: 1.0,
            min_zoom: 0.5,
            max_zoom: 4.0,
            focal_point: Vec2::ZERO,
            pan_offset: Vec2::ZERO,
        }
    }

    /// Apply a continuous pinch gesture scale delta around a focal center point.
    pub fn apply_pinch(&mut self, scale_delta: f32, center: Vec2) {
        let prev_zoom = self.zoom_level;
        self.zoom_level = (self.zoom_level * scale_delta).clamp(self.min_zoom, self.max_zoom);
        self.focal_point = center;

        // Keep focal point stationary during zoom: offset adjustments
        let ratio = self.zoom_level / prev_zoom;
        self.pan_offset = center - (center - self.pan_offset) * ratio;
    }

    /// Apply a 2D translational pan offset in screen coordinates.
    pub fn apply_pan(&mut self, delta: Vec2) {
        self.pan_offset += delta;
    }

    /// Reset zoom and pan back to origin default.
    pub fn reset(&mut self) {
        self.zoom_level = 1.0;
        self.focal_point = Vec2::ZERO;
        self.pan_offset = Vec2::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinch_zoom_controller_clamping_and_reset() {
        let mut pz = PinchZoomController::new();
        assert_eq!(pz.zoom_level, 1.0);

        // Zoom in by 2x
        pz.apply_pinch(2.0, Vec2::new(100.0, 100.0));
        assert_eq!(pz.zoom_level, 2.0);

        // Zoom in beyond max clamp (4.0)
        pz.apply_pinch(3.0, Vec2::new(100.0, 100.0));
        assert_eq!(pz.zoom_level, 4.0);

        // Zoom out beyond min clamp (0.5)
        pz.apply_pinch(0.1, Vec2::new(100.0, 100.0));
        assert_eq!(pz.zoom_level, 0.5);

        pz.pan_offset = Vec2::ZERO;
        // Pan
        pz.apply_pan(Vec2::new(20.0, -15.0));
        assert_eq!(pz.pan_offset.x, 20.0);

        pz.reset();
        assert_eq!(pz.zoom_level, 1.0);
        assert_eq!(pz.pan_offset, Vec2::ZERO);
    }
}
