//! Render-cadence projection for the Bevy shell (DUAL-15-08).
//!
//! The policy lives in `infiltrator_contract::cadence`; Bevy's real host knob
//! is `bevy::winit::WinitSettings`, so this module maps the shared cadence onto
//! it and keeps it in sync with the live window facts (`WindowFocused`,
//! `WindowOccluded`, `Window::visible`):
//!
//! - focused + visible + engaged → `Continuous` (the ~60 FPS active cadence),
//! - focused + visible but idle (no input, no animation) → 10 FPS reactive
//!   low power (the contract's `Idling` cadence, BEVY-040-04),
//! - backgrounded → 2 FPS reactive low power,
//! - occluded/invisible → the longest low-power wait (an event-driven loop: no
//!   scheduled frames, but OS/user events still wake the app).
//!
//! Engagement is tracked by [`CadenceActivity`]: user input and running
//! animations keep the window hot, and a short frame debounce decays it to the
//! idling rate once the surface settles.
//!
//! The Iced shell consumes the same policy for its animation frame tick, so
//! both surfaces detune to the same rates.

use crate::host_capabilities::HostPreferences;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::{MouseButtonInput, MouseMotion, MouseWheel};
use bevy::input::touch::TouchInput;
use bevy::window::{CursorMoved, PrimaryWindow, Window, WindowFocused, WindowOccluded};
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

    /// The cadence including activity: a visible, focused window that is not
    /// engaged (no recent input and no running animation) detunes from
    /// [`RenderCadence::Active`] to the low-power [`RenderCadence::Idling`].
    /// Background and suspended states are unaffected by activity.
    pub fn cadence_with(self, engaged: bool) -> RenderCadence {
        match self.cadence() {
            RenderCadence::Active if !engaged => RenderCadence::Idling,
            cadence => cadence,
        }
    }
}

/// How many quiet frames after the last input or animation frame before a
/// visible, focused window detunes from `Active` to `Idling`.
pub const CADENCE_IDLE_FRAMES: u32 = 30;

/// Tracks the recent interaction and running animations that must keep the
/// render loop at the `Active` cadence. The engagement is a short frame
/// debounce: it decays every frame, and any input event or animation frame
/// re-arms it. Input and animation engagement are tracked separately so the
/// host's reduce-motion / energy preference can drop animation-driven heat
/// without making the shell unresponsive to the user.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CadenceActivity {
    input_frames: u32,
    animation_frames: u32,
}

impl Default for CadenceActivity {
    fn default() -> Self {
        // A cold start paints at full rate; idle decay begins once the shell
        // has settled.
        Self {
            input_frames: CADENCE_IDLE_FRAMES,
            animation_frames: 0,
        }
    }
}

impl CadenceActivity {
    /// Mark the shell engaged by user input for the next
    /// [`CADENCE_IDLE_FRAMES`] frames. Input tracking calls this per event.
    pub fn wake(&mut self) {
        self.input_frames = CADENCE_IDLE_FRAMES;
    }

    /// Mark the shell engaged by a running visible animation. An animation
    /// system calls this on every frame it advances a visible animation.
    pub fn wake_animation(&mut self) {
        self.animation_frames = CADENCE_IDLE_FRAMES;
    }

    /// Whether input or an animation was seen recently enough to stay hot,
    /// before the host preference is applied.
    pub const fn is_engaged(&self) -> bool {
        self.input_frames > 0 || self.animation_frames > 0
    }

    /// Whether the shell should stay hot given the host preference. Input
    /// always keeps it hot; animation-driven engagement is dropped when the
    /// host asks to reduce motion or conserve power.
    pub const fn is_engaged_with(&self, preferences: HostPreferences) -> bool {
        self.input_frames > 0 || (self.animation_frames > 0 && !preferences.animations_suppressed())
    }

    /// Advance one frame, decaying the engagement toward idle.
    pub fn decay(&mut self) {
        self.input_frames = self.input_frames.saturating_sub(1);
        self.animation_frames = self.animation_frames.saturating_sub(1);
    }
}

/// The last cadence written to [`WinitSettings`], so [`sync_window_cadence`]
/// can tell an activity-driven change from a no-op without fighting another
/// writer that installed its own settings.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppliedCadence(pub RenderCadence);

impl Default for AppliedCadence {
    fn default() -> Self {
        Self(RenderCadence::Active)
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
        RenderCadence::Idling => WinitSettings {
            focused_mode: UpdateMode::reactive_low_power(Duration::from_millis(
                RenderCadence::IDLING_FRAME_TIME_MS,
            )),
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

/// Cohesive cadence resources for [`sync_window_cadence`]: the live window
/// facts plus the host preference that decides whether animation-driven
/// engagement counts. Bundled so the system keeps a small typed parameter list
/// (BEVY-ECS-009).
#[derive(SystemParam)]
pub struct CadenceSurface<'w> {
    activity: Res<'w, CadenceActivity>,
    preferences: Res<'w, HostPreferences>,
    applied: ResMut<'w, AppliedCadence>,
    state: ResMut<'w, WindowCadenceState>,
    settings: ResMut<'w, WinitSettings>,
}

/// Keep the shared cadence and the winit update modes aligned with the live
/// window facts and the current activity. Only a real change rewrites the
/// resource, so the resource never fights another writer.
pub fn sync_window_cadence(
    mut focused_events: MessageReader<WindowFocused>,
    mut occluded_events: MessageReader<WindowOccluded>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut surface: CadenceSurface,
) {
    let mut next = *surface.state;
    for event in focused_events.read() {
        next.focused = event.focused;
    }
    for event in occluded_events.read() {
        next.visible = !event.occluded;
    }
    if let Ok(window) = windows.single() {
        next.visible = next.visible && window.visible;
    }
    let engaged = surface.activity.is_engaged_with(*surface.preferences);
    let cadence = next.cadence_with(engaged);
    if next == *surface.state && cadence == surface.applied.0 {
        return;
    }
    *surface.state = next;
    surface.applied.0 = cadence;
    *surface.settings = winit_settings_for(cadence);
}

/// Decay the engagement by one frame and re-arm it on any user input. Runs
/// before [`sync_window_cadence`] so input in the same frame selects the
/// active cadence.
pub fn advance_cadence_activity(
    mut activity: ResMut<CadenceActivity>,
    mut keyboard: MessageReader<KeyboardInput>,
    mut mouse_button: MessageReader<MouseButtonInput>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut cursor: MessageReader<CursorMoved>,
    mut touch: MessageReader<TouchInput>,
) {
    activity.decay();
    let mut interacted = false;
    interacted |= drain_message(&mut keyboard);
    interacted |= drain_message(&mut mouse_button);
    interacted |= drain_message(&mut mouse_motion);
    interacted |= drain_message(&mut mouse_wheel);
    interacted |= drain_message(&mut cursor);
    interacted |= drain_message(&mut touch);
    if interacted {
        activity.wake();
    }
}

/// Consume a reader and report whether it held any unread message.
fn drain_message<M: Message>(reader: &mut MessageReader<M>) -> bool {
    let seen = !reader.is_empty();
    reader.clear();
    seen
}

/// Installs the shared-cadence window policy.
pub struct CadencePlugin;

impl Plugin for CadencePlugin {
    fn build(&self, app: &mut App) {
        // The window power and input messages may already be registered by
        // `WindowPlugin`/`InputPlugin`; registration is idempotent, and a
        // headless test app needs them so the system parameters are valid.
        app.add_message::<WindowFocused>()
            .add_message::<WindowOccluded>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseButtonInput>()
            .add_message::<MouseMotion>()
            .add_message::<MouseWheel>()
            .add_message::<CursorMoved>()
            .add_message::<TouchInput>()
            .init_resource::<WindowCadenceState>()
            .init_resource::<CadenceActivity>()
            .init_resource::<AppliedCadence>()
            .init_resource::<HostPreferences>()
            .init_resource::<WinitSettings>()
            .add_systems(
                Update,
                (advance_cadence_activity, sync_window_cadence).chain(),
            );
    }
}
