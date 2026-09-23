//! The Iced surface's honest touch-gesture boundary (DUAL-15-07).
//!
//! The mobile touch-gesture engine is a Bevy-shell capability: Bevy consumes
//! `bevy::input::touch::TouchInput` and drives the widget-layer recognizer into
//! the shared `infiltrator_contract::shell_gesture` snapshot. Iced is the
//! desktop winit surface and has no touch host wired, so it reports the typed
//! unsupported capability instead of fabricating a recognizer. This module
//! makes that boundary a typed, testable fact rather than an absence.

use infiltrator_contract::shell_gesture::TouchGestureSupport;

/// What this surface can do with touch: nothing is wired.
pub const fn touch_support() -> TouchGestureSupport {
    TouchGestureSupport::Unsupported {
        reason: "iced-desktop-surface-has-no-touch-gesture-host",
    }
}

#[cfg(test)]
#[path = "../tests/gui/gesture_boundary_tests.rs"]
mod gesture_boundary_tests;
