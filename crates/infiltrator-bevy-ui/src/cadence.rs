//! Render-cadence projection for the Bevy shell (DUAL-15-08).
//!
//! The policy lives in `infiltrator_contract::cadence`; Bevy's real host knob
//! is `bevy::winit::WinitSettings`, so this module maps the shared cadence onto
//! it and keeps it in sync with the live window facts (`WindowFocused`,
//! `WindowOccluded`, `Window::visible`):
//!
//! - focused + visible → `Continuous` (the ~60 FPS active cadence),
//! - backgrounded → 2 FPS reactive low power,
//! - occluded/invisible → the longest low-power wait (an event-driven loop: no
//!   scheduled frames, but OS/user events still wake the app).
//!
//! The Iced shell consumes the same policy for its animation frame tick, so
//! both surfaces detune to the same rates.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::message::MessageReader;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::window::{PrimaryWindow, Window, WindowFocused, WindowOccluded};
use bevy::winit::{UpdateMode, WinitSettings};
use infiltrator_contract::cadence::RenderCadence;
use std::time::Duration;

/// The wait used while the host reports a hard suspend. Winit cannot stop the
/// loop, so the shortest honest expression is an event-driven loop with no
/// scheduled frames.
pub const SUSPENDED_WAIT: Duration = Duration::from_secs(60);

/// The live window power facts, initialised to the cold-start state (focused
/// and visible).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowCadenceState {
    pub focused: bool,
    pub visible: bool,
}

impl Default for WindowCadenceState {
    fn default() -> Self {
        Self {
            focused: true,
            visible: true,
        }
    }
}

impl WindowCadenceState {
    /// The shared cadence these facts select.
    pub fn cadence(self) -> RenderCadence {
        RenderCadence::from_visible_focused(self.visible, self.focused)
    }
}

/// The winit configuration for one window state.
///
/// `WinitSettings` selects by focus, so a state that must override the
/// focused branch (background while the OS still considers us focused, or a
/// hard suspend) sets both modes to the target rate.
pub fn winit_settings_for(cadence: RenderCadence) -> WinitSettings {
    let background = UpdateMode::reactive_low_power(Duration::from_millis(
        RenderCadence::BACKGROUND_FRAME_TIME_MS,
    ));
    match cadence {
        RenderCadence::Active => WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: background,
        },
        RenderCadence::Background => WinitSettings {
            focused_mode: background,
            unfocused_mode: background,
        },
        RenderCadence::Suspended => WinitSettings {
            focused_mode: UpdateMode::reactive_low_power(SUSPENDED_WAIT),
            unfocused_mode: UpdateMode::reactive_low_power(SUSPENDED_WAIT),
        },
    }
}

/// Keep the shared cadence and the winit update modes aligned with the live
/// window facts. Only a real state change rewrites the resource, so the
/// resource never fights another writer.
pub fn sync_window_cadence(
    mut focused_events: MessageReader<WindowFocused>,
    mut occluded_events: MessageReader<WindowOccluded>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<WindowCadenceState>,
    mut settings: ResMut<WinitSettings>,
) {
    let mut next = *state;
    for event in focused_events.read() {
        next.focused = event.focused;
    }
    for event in occluded_events.read() {
        next.visible = !event.occluded;
    }
    if let Ok(window) = windows.single() {
        next.visible = next.visible && window.visible;
    }
    let cadence = next.cadence();
    if next == *state && cadence == state.cadence() {
        return;
    }
    *state = next;
    *settings = winit_settings_for(cadence);
}

/// Installs the shared-cadence window policy.
pub struct CadencePlugin;

impl Plugin for CadencePlugin {
    fn build(&self, app: &mut App) {
        // The window power messages may already be registered by
        // `WindowPlugin`; registration is idempotent, and a headless test app
        // needs them so the system parameter is valid.
        app.add_message::<WindowFocused>()
            .add_message::<WindowOccluded>()
            .init_resource::<WindowCadenceState>()
            .init_resource::<WinitSettings>()
            .add_systems(Update, sync_window_cadence);
    }
}
