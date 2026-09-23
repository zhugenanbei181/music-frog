//! Shared touch-gesture semantic contract for the shell (DUAL-15-07).
//!
//! Mobile touch gestures need one vocabulary every surface can publish, but
//! the *recognition* itself must not be duplicated: the widget layer's
//! `infiltrator_bevy_widgets::gesture::GestureRecognizer` is the single
//! source of truth for tap/double-tap/long-press/pan/swipe, and its
//! `PullToRefreshState`/`SwipeToActionItem`/`PinchZoomController` own the
//! pull/swipe/pinch state machines. This module is deliberately the
//! toolkit-neutral *language* those results are expressed in:
//!
//! * one raw-phase vocabulary ([`GestureTouchPhase`]) a host normalizes its
//!   toolkit events onto, plus the deterministic lifecycle event it maps to
//!   ([`GestureTouchPhase::touch_event`]);
//! * one typed semantic event set ([`GestureSemanticEvent`]) for
//!   pull-to-refresh progress/release, swipe-to-action, pinch/pan, safe-area
//!   insets and the touch capability;
//! * one bounded snapshot ([`GestureSnapshot`]) the shell publishes, with a
//!   revision counter and no unbounded collections.
//!
//! Nothing here depends on bevy, iced, or a windowing system, so the same
//! snapshot is meaningful for a headless test and for a future mobile host.

/// The top of the normalized per-mille scale used for progress/extent.
pub const GESTURE_PERMILLE_MAX: u16 = 1000;

/// Toolkit-neutral 2D point in logical pixels. Non-finite inputs collapse to
/// `0` so a broken host sample can never poison the snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GesturePoint {
    pub x: f32,
    pub y: f32,
}

impl GesturePoint {
    /// The origin.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// A sanitized point.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x: finite(x),
            y: finite(y),
        }
    }

    /// Whether both components are finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// The component-wise difference `self - from`.
    pub fn delta(self, from: Self) -> Self {
        Self {
            x: finite(self.x - from.x),
            y: finite(self.y - from.y),
        }
    }

    /// The Euclidean length.
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// Normalize `numerator / denominator` onto `0..=GESTURE_PERMILLE_MAX`.
fn permille(numerator: f32, denominator: f32) -> u16 {
    if !denominator.is_finite() || denominator <= 0.0 {
        return 0;
    }
    let fraction = (finite(numerator) / denominator).clamp(0.0, 1.0);
    (fraction * f32::from(GESTURE_PERMILLE_MAX)).round() as u16
}

/// A raw touch phase normalized from any toolkit's touch vocabulary.
///
/// This is the input side of the deterministic mapping: a host turns its own
/// phase type into one of these and the contract turns it into a
/// [`GestureSemanticEvent::Touch`] lifecycle event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureTouchPhase {
    Started,
    Moved,
    Ended,
    Canceled,
}

impl GestureTouchPhase {
    /// Whether the touch is still down.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Started | Self::Moved)
    }

    /// Whether the touch can no longer move.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Ended | Self::Canceled)
    }

    /// The shared lifecycle event for this raw phase at `position`.
    pub fn touch_event(self, position: GesturePoint) -> GestureSemanticEvent {
        GestureSemanticEvent::Touch {
            phase: self,
            position: GesturePoint::new(position.x, position.y),
        }
    }
}

/// Where a pull-to-refresh gesture is in its lifecycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PullPhase {
    /// No pull in progress.
    #[default]
    Idle,
    /// Pulling but not far enough to arm a refresh.
    Pulling,
    /// Past the threshold; releasing now would refresh.
    Armed,
    /// A refresh is running and the indicator stays pinned.
    Refreshing,
}

/// Which way a swipe-to-action row is displaced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwipeDirection {
    /// Centered.
    #[default]
    None,
    /// Displaced toward the leading (left) edge.
    Leading,
    /// Displaced toward the trailing (right) edge.
    Trailing,
}

/// What a host can honestly do with touch gestures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchGestureSupport {
    /// The host delivers real touch events and hosts the recognizer.
    Hosted {
        /// Whether more than one simultaneous pointer is delivered.
        multi_touch: bool,
    },
    /// The host has no touch surface; it must not fake a recognizer.
    Unsupported { reason: &'static str },
}

impl TouchGestureSupport {
    /// The typed default for a host that declared nothing.
    pub const UNSUPPORTED: Self = Self::Unsupported {
        reason: "host did not declare a touch gesture surface",
    };

    /// Whether this host hosts the recognizer.
    pub const fn is_hosted(self) -> bool {
        matches!(self, Self::Hosted { .. })
    }

    /// Whether the host delivers more than one simultaneous pointer.
    pub const fn multi_touch(self) -> bool {
        match self {
            Self::Hosted { multi_touch } => multi_touch,
            Self::Unsupported { .. } => false,
        }
    }

    /// The typed boundary of a host without touch.
    pub const fn unsupported_reason(self) -> Option<&'static str> {
        match self {
            Self::Hosted { .. } => None,
            Self::Unsupported { reason } => Some(reason),
        }
    }
}

/// Safe-area insets in logical pixels (status bar, navigation bar, display
/// cutouts/notches). Negative and non-finite values collapse to `0` because a
/// host may not shrink the usable canvas below its own bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SafeAreaInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl SafeAreaInsets {
    /// No inset.
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// A sanitized inset.
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top: finite(top).max(0.0),
            right: finite(right).max(0.0),
            bottom: finite(bottom).max(0.0),
            left: finite(left).max(0.0),
        }
    }

    /// Left + right.
    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    /// Top + bottom.
    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }

    /// Whether every edge is zero.
    pub fn is_zero(self) -> bool {
        self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0 && self.left == 0.0
    }
}

/// The live single-pointer lifecycle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TouchSnapshot {
    /// The last raw phase, or `None` when the pointer settled on a recognized
    /// gesture (tap/long-press).
    pub phase: Option<GestureTouchPhase>,
    pub position: GesturePoint,
}

/// The live pull-to-refresh reading.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PullToRefreshSnapshot {
    pub phase: PullPhase,
    pub progress_permille: u16,
}

/// The live swipe-to-action reading.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SwipeToActionSnapshot {
    pub direction: SwipeDirection,
    pub extent_permille: u16,
}

/// The live pinch/zoom reading.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PinchZoomSnapshot {
    /// The last incremental scale, per-mille.
    pub scale_permille: u16,
    /// The accumulated zoom level, per-mille.
    pub zoom_permille: u16,
    pub focal: GesturePoint,
}

/// The live pan reading.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PanSnapshot {
    pub delta: GesturePoint,
    pub position: GesturePoint,
}

/// One typed semantic gesture event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureSemanticEvent {
    /// A raw touch phase, normalized.
    Touch {
        phase: GestureTouchPhase,
        position: GesturePoint,
    },
    Tap {
        position: GesturePoint,
    },
    DoubleTap {
        position: GesturePoint,
    },
    LongPress {
        position: GesturePoint,
    },
    Pan {
        delta: GesturePoint,
        position: GesturePoint,
    },
    Swipe {
        delta: GesturePoint,
        velocity: GesturePoint,
    },
    Pinch {
        scale_permille: u16,
        zoom_permille: u16,
        focal: GesturePoint,
    },
    PullToRefresh {
        phase: PullPhase,
        progress_permille: u16,
    },
    SwipeToAction {
        direction: SwipeDirection,
        extent_permille: u16,
    },
    /// Host-declared safe-area insets (published once they are known).
    SafeAreaInsets(SafeAreaInsets),
    /// Host-declared touch capability.
    TouchCapability(TouchGestureSupport),
}

impl GestureSemanticEvent {
    /// Deterministic pull-to-refresh mapping from the widget layer's
    /// `PullToRefreshState` values. `threshold <= 0` can never arm.
    pub fn pull_to_refresh(offset: f32, threshold: f32, is_refreshing: bool) -> Self {
        let offset = finite(offset).max(0.0);
        let phase = if is_refreshing {
            PullPhase::Refreshing
        } else if offset <= 0.0 {
            PullPhase::Idle
        } else if threshold > 0.0 && offset >= threshold {
            PullPhase::Armed
        } else {
            PullPhase::Pulling
        };
        let progress_permille = if is_refreshing {
            GESTURE_PERMILLE_MAX
        } else {
            permille(offset, threshold)
        };
        Self::PullToRefresh {
            phase,
            progress_permille,
        }
    }

    /// Deterministic swipe-to-action mapping from the widget layer's
    /// `SwipeToActionItem::offset_x` and its `max_action_width`.
    pub fn swipe_to_action(offset_x: f32, max_action_width: f32) -> Self {
        let offset = finite(offset_x);
        let direction = if offset > 0.0 {
            SwipeDirection::Trailing
        } else if offset < 0.0 {
            SwipeDirection::Leading
        } else {
            SwipeDirection::None
        };
        Self::SwipeToAction {
            direction,
            extent_permille: permille(offset.abs(), max_action_width),
        }
    }

    /// Deterministic pinch mapping from an incremental `scale_delta` and the
    /// accumulated zoom level of the widget layer's `PinchZoomController`.
    pub fn pinch(scale_delta: f32, zoom_level: f32, focal: GesturePoint) -> Self {
        Self::Pinch {
            scale_permille: scaled_permille(scale_delta),
            zoom_permille: scaled_permille(zoom_level),
            focal: GesturePoint::new(focal.x, focal.y),
        }
    }

    /// The pan mapping from the widget recognizer's delta and current point.
    pub fn pan(delta: GesturePoint, position: GesturePoint) -> Self {
        Self::Pan {
            delta: GesturePoint::new(delta.x, delta.y),
            position: GesturePoint::new(position.x, position.y),
        }
    }
}

fn scaled_permille(value: f32) -> u16 {
    let scaled = (finite(value).max(0.0) * f32::from(GESTURE_PERMILLE_MAX)).round();
    scaled.min(f32::from(u16::MAX)) as u16
}

/// The bounded gesture snapshot a shell publishes.
///
/// It contains fixed-size readings only (no vectors or maps), so a long-lived
/// app cannot grow it, and every [`GestureSnapshot::apply`] bumps a saturating
/// revision so consumers can detect a change.
#[derive(Clone, Debug, PartialEq)]
pub struct GestureSnapshot {
    pub revision: u64,
    pub capability: TouchGestureSupport,
    pub insets: SafeAreaInsets,
    pub touch: TouchSnapshot,
    pub pan: PanSnapshot,
    pub pull: PullToRefreshSnapshot,
    pub swipe: SwipeToActionSnapshot,
    pub pinch: PinchZoomSnapshot,
    /// The most recent semantic event (`None` before any input).
    pub last: Option<GestureSemanticEvent>,
}

impl Default for GestureSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            capability: TouchGestureSupport::UNSUPPORTED,
            insets: SafeAreaInsets::ZERO,
            touch: TouchSnapshot::default(),
            pan: PanSnapshot::default(),
            pull: PullToRefreshSnapshot::default(),
            swipe: SwipeToActionSnapshot::default(),
            pinch: PinchZoomSnapshot::default(),
            last: None,
        }
    }
}

impl GestureSnapshot {
    /// An empty snapshot for a host that declared `capability`.
    pub fn new(capability: TouchGestureSupport) -> Self {
        Self {
            capability,
            ..Self::default()
        }
    }

    /// Apply one event, updating the relevant reading and the revision.
    /// Returns the new revision.
    pub fn apply(&mut self, event: GestureSemanticEvent) -> u64 {
        match event {
            GestureSemanticEvent::Touch { phase, position } => {
                self.touch = TouchSnapshot {
                    phase: Some(phase),
                    position,
                };
            }
            GestureSemanticEvent::Tap { position }
            | GestureSemanticEvent::DoubleTap { position }
            | GestureSemanticEvent::LongPress { position } => {
                self.touch = TouchSnapshot {
                    phase: None,
                    position,
                };
            }
            GestureSemanticEvent::Pan { delta, position } => {
                self.pan = PanSnapshot { delta, position };
            }
            GestureSemanticEvent::Swipe { delta, .. } => {
                self.pan = PanSnapshot {
                    delta,
                    position: self.touch.position,
                };
            }
            GestureSemanticEvent::Pinch {
                scale_permille,
                zoom_permille,
                focal,
            } => {
                self.pinch = PinchZoomSnapshot {
                    scale_permille,
                    zoom_permille,
                    focal,
                };
            }
            GestureSemanticEvent::PullToRefresh {
                phase,
                progress_permille,
            } => {
                self.pull = PullToRefreshSnapshot {
                    phase,
                    progress_permille,
                };
            }
            GestureSemanticEvent::SwipeToAction {
                direction,
                extent_permille,
            } => {
                self.swipe = SwipeToActionSnapshot {
                    direction,
                    extent_permille,
                };
            }
            GestureSemanticEvent::SafeAreaInsets(insets) => {
                self.insets = insets;
            }
            GestureSemanticEvent::TouchCapability(support) => {
                self.capability = support;
            }
        }
        self.revision = self.revision.saturating_add(1);
        self.last = Some(event);
        self.revision
    }

    /// The most recent semantic event.
    pub fn last(&self) -> Option<GestureSemanticEvent> {
        self.last
    }

    /// Whether this host hosts a real touch surface.
    pub fn has_touch_host(&self) -> bool {
        self.capability.is_hosted()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pull_release_maps_to_a_bounded_semantic_event() {
        assert_eq!(
            GestureSemanticEvent::pull_to_refresh(0.0, 80.0, false),
            GestureSemanticEvent::PullToRefresh {
                phase: PullPhase::Idle,
                progress_permille: 0,
            }
        );
        let pulling = GestureSemanticEvent::pull_to_refresh(40.0, 80.0, false);
        assert_eq!(
            pulling,
            GestureSemanticEvent::PullToRefresh {
                phase: PullPhase::Pulling,
                progress_permille: 500,
            }
        );
        assert_eq!(
            GestureSemanticEvent::pull_to_refresh(80.0, 80.0, false),
            GestureSemanticEvent::PullToRefresh {
                phase: PullPhase::Armed,
                progress_permille: 1000,
            }
        );
        assert_eq!(
            GestureSemanticEvent::pull_to_refresh(120.0, 80.0, true),
            GestureSemanticEvent::PullToRefresh {
                phase: PullPhase::Refreshing,
                progress_permille: 1000,
            }
        );
        // A non-finite or non-positive threshold never fabricates progress.
        assert_eq!(
            GestureSemanticEvent::pull_to_refresh(f32::NAN, 0.0, false),
            GestureSemanticEvent::PullToRefresh {
                phase: PullPhase::Idle,
                progress_permille: 0,
            }
        );
    }

    #[test]
    fn a_horizontal_drag_maps_to_a_typed_swipe_direction() {
        assert_eq!(
            GestureSemanticEvent::swipe_to_action(-40.0, 80.0),
            GestureSemanticEvent::SwipeToAction {
                direction: SwipeDirection::Leading,
                extent_permille: 500,
            }
        );
        assert_eq!(
            GestureSemanticEvent::swipe_to_action(80.0, 80.0),
            GestureSemanticEvent::SwipeToAction {
                direction: SwipeDirection::Trailing,
                extent_permille: 1000,
            }
        );
        assert_eq!(
            GestureSemanticEvent::swipe_to_action(0.0, 80.0),
            GestureSemanticEvent::SwipeToAction {
                direction: SwipeDirection::None,
                extent_permille: 0,
            }
        );
    }

    #[test]
    fn raw_touch_phases_map_to_the_shared_lifecycle_vocabulary() {
        let point = GesturePoint::new(12.0, 34.0);
        assert_eq!(
            GestureTouchPhase::Started.touch_event(point),
            GestureSemanticEvent::Touch {
                phase: GestureTouchPhase::Started,
                position: point,
            }
        );
        assert!(GestureTouchPhase::Started.is_active());
        assert!(GestureTouchPhase::Moved.is_active());
        assert!(GestureTouchPhase::Ended.is_terminal());
        assert!(GestureTouchPhase::Canceled.is_terminal());
        assert!(!GestureTouchPhase::Ended.is_active());
        assert_eq!(GesturePoint::new(f32::NAN, 1.0).x, 0.0);
        assert_eq!(GesturePoint::new(f32::INFINITY, 1.0).x, 0.0);
        assert!(GesturePoint::new(f32::INFINITY, 1.0).is_finite());
        let raw = GesturePoint {
            x: f32::INFINITY,
            y: 1.0,
        };
        assert!(!raw.is_finite());
    }

    #[test]
    fn safe_area_insets_reject_non_finite_and_negative_values() {
        let insets = SafeAreaInsets::new(44.0, -8.0, f32::NAN, 16.0);
        assert_eq!(insets.top, 44.0);
        assert_eq!(insets.right, 0.0);
        assert_eq!(insets.bottom, 0.0);
        assert_eq!(insets.left, 16.0);
        assert_eq!(insets.vertical(), 44.0);
        assert_eq!(insets.horizontal(), 16.0);
        assert!(SafeAreaInsets::ZERO.is_zero());
        assert!(!insets.is_zero());
    }

    #[test]
    fn the_snapshot_reducer_is_bounded_and_revisioned() {
        let mut snapshot = GestureSnapshot::new(TouchGestureSupport::Hosted { multi_touch: true });
        assert!(snapshot.has_touch_host());
        assert_eq!(snapshot.revision, 0);

        let revision = snapshot.apply(GestureSemanticEvent::pull_to_refresh(40.0, 80.0, false));
        assert_eq!(revision, 1);
        assert_eq!(snapshot.pull.progress_permille, 500);
        assert_eq!(
            snapshot.last(),
            Some(GestureSemanticEvent::PullToRefresh {
                phase: PullPhase::Pulling,
                progress_permille: 500,
            })
        );

        snapshot.apply(GestureSemanticEvent::Touch {
            phase: GestureTouchPhase::Moved,
            position: GesturePoint::new(5.0, 6.0),
        });
        assert_eq!(snapshot.touch.phase, Some(GestureTouchPhase::Moved));
        assert_eq!(snapshot.revision, 2);

        snapshot.apply(GestureSemanticEvent::Tap {
            position: GesturePoint::new(5.0, 6.0),
        });
        assert_eq!(snapshot.touch.phase, None, "a settled tap clears the phase");
        assert_eq!(snapshot.revision, 3);
    }

    #[test]
    fn a_host_without_touch_reports_a_typed_reason() {
        let support = TouchGestureSupport::UNSUPPORTED;
        assert!(!support.is_hosted());
        assert!(!support.multi_touch());
        assert_eq!(
            support.unsupported_reason(),
            Some("host did not declare a touch gesture surface")
        );

        let hosted = TouchGestureSupport::Hosted { multi_touch: true };
        assert!(hosted.is_hosted());
        assert!(hosted.multi_touch());
        assert_eq!(hosted.unsupported_reason(), None);

        let snapshot = GestureSnapshot::default();
        assert!(!snapshot.has_touch_host());
    }
}
