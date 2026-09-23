//! Real Bevy-shell touch-gesture consumption (DUAL-15-07).
//!
//! Bevy delivers translated winit touch events as
//! `bevy::input::touch::TouchInput` messages (phase + position + finger id).
//! Nothing consumed them before this module, so the shell's mobile gesture
//! path is wired here:
//!
//! * **recognize**: raw phases drive the widget layer's
//!   `GestureRecognizer`/`PullToRefreshState`/`SwipeToActionItem`/
//!   `PinchZoomController` — the single source of truth for recognition, never
//!   re-implemented here;
//! * **map**: every result becomes a toolkit-neutral
//!   `infiltrator_contract::shell_gesture` semantic event, and the raw phase
//!   maps onto the shared lifecycle vocabulary;
//! * **publish**: the bounded `GestureSnapshot` is a Bevy resource any page can
//!   read, with a revision counter.
//!
//! The shell applies a deterministic routing policy so the same drag cannot be
//! read as three unrelated gestures at once: a downward drag that starts in the
//! top band is a pull-to-refresh, a dominant horizontal drag is a
//! swipe-to-action, a two-finger sequence is a pinch, and everything else goes
//! to the recognizer (tap/double-tap/long-press/pan). The boundary is explicit:
//! Bevy 0.19 exposes no safe-area API, so insets default to zero and a mobile
//! composition root must inject the real values through [`GestureHostReport`].

use std::collections::BTreeMap;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::message::MessageReader;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Res, ResMut};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::math::Vec2;
use bevy::time::Time;

use infiltrator_bevy_widgets::gesture;
use infiltrator_bevy_widgets::gesture::{
    GestureOutcome, GestureRecognizer, PinchZoomController, PullToRefreshState, SwipeToActionItem,
};
use infiltrator_contract::shell_gesture::{
    GesturePoint, GestureSemanticEvent, GestureSnapshot, GestureTouchPhase, SafeAreaInsets,
    TouchGestureSupport,
};

/// Maximum simultaneous touches tracked; extra fingers are ignored so the
/// active set can never grow without bound.
pub const MAX_TRACKED_TOUCHES: usize = 4;
/// A downward drag must start within this band from the top edge to be a pull.
pub const PULL_EDGE_PX: f32 = 96.0;
/// Pull distance that arms a refresh (the widget state machine's threshold).
pub const PULL_THRESHOLD_PX: f32 = 80.0;
/// Swipe-to-action travel handed to the widget item.
pub const SWIPE_ACTION_WIDTH_PX: f32 = 80.0;
/// Movement before the shell locks a gesture axis.
pub const GESTURE_DIRECTION_LOCK_PX: f32 = 10.0;

/// What this surface can do with touch: it hosts the real recognizer and
/// reports multiple pointers.
pub const fn touch_support() -> TouchGestureSupport {
    TouchGestureSupport::Hosted { multi_touch: true }
}

/// The gesture axis the current sequence locked onto.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum GestureMode {
    #[default]
    Undecided,
    Recognizer,
    Pull,
    Swipe,
    Pinch,
}

/// The live recognition state of the shell: the widget-layer state machines
/// plus the bounded active-touch set used to route the sequence.
#[derive(Resource)]
pub struct ShellGestureState {
    recognizer: GestureRecognizer,
    pull: PullToRefreshState,
    swipe: SwipeToActionItem,
    pinch: PinchZoomController,
    active: BTreeMap<u64, GesturePoint>,
    mode: GestureMode,
    start: Option<GesturePoint>,
    pinch_distance: Option<f32>,
}

impl Default for ShellGestureState {
    fn default() -> Self {
        Self {
            recognizer: GestureRecognizer::new(),
            pull: PullToRefreshState::new(PULL_THRESHOLD_PX),
            swipe: SwipeToActionItem::new(SWIPE_ACTION_WIDTH_PX),
            pinch: PinchZoomController::new(),
            active: BTreeMap::new(),
            mode: GestureMode::Undecided,
            start: None,
            pinch_distance: None,
        }
    }
}

impl ShellGestureState {
    /// The number of pointers currently tracked.
    pub fn tracked_touches(&self) -> usize {
        self.active.len()
    }

    /// The widget recognizer this shell drives (single source of truth).
    pub fn recognizer(&self) -> &GestureRecognizer {
        &self.recognizer
    }

    /// The widget pinch/zoom controller this shell drives.
    pub fn pinch(&self) -> &PinchZoomController {
        &self.pinch
    }
}

/// The bounded shared snapshot as a Bevy resource.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ShellGestureSnapshot(pub GestureSnapshot);

/// The host facts of this surface, inserted by [`ShellGesturePlugin`].
///
/// `insets` is public because Bevy 0.19 has no safe-area API: a mobile
/// composition root writes the real values here and
/// [`sync_gesture_capability`] publishes them into the shared snapshot.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct GestureHostReport {
    pub support: TouchGestureSupport,
    pub insets: SafeAreaInsets,
    pub tracked_touches: usize,
}

impl Default for GestureHostReport {
    fn default() -> Self {
        Self {
            support: touch_support(),
            insets: SafeAreaInsets::ZERO,
            tracked_touches: 0,
        }
    }
}

/// Installs the real touch consumer and the shared snapshot.
pub struct ShellGesturePlugin;

impl Plugin for ShellGesturePlugin {
    fn build(&self, app: &mut App) {
        // Headless compositions have no `InputPlugin`, so register the touch
        // message channel and the resources here (registration is idempotent
        // when `DefaultPlugins` already did it).
        app.add_message::<TouchInput>()
            .init_resource::<ShellGestureState>()
            .init_resource::<GestureHostReport>()
            .insert_resource(ShellGestureSnapshot(GestureSnapshot::new(touch_support())))
            .add_systems(
                Update,
                (consume_touch_input, sync_gesture_capability).chain(),
            );
    }
}

/// Map a raw Bevy touch phase onto the shared raw-phase vocabulary.
pub fn shared_phase(phase: TouchPhase) -> GestureTouchPhase {
    match phase {
        TouchPhase::Started => GestureTouchPhase::Started,
        TouchPhase::Moved => GestureTouchPhase::Moved,
        TouchPhase::Ended => GestureTouchPhase::Ended,
        TouchPhase::Canceled => GestureTouchPhase::Canceled,
    }
}

/// Map one widget-layer recognizer outcome onto the shared semantic event.
pub fn shared_outcome(outcome: GestureOutcome) -> GestureSemanticEvent {
    match outcome {
        GestureOutcome::Tap(position) => GestureSemanticEvent::Tap {
            position: point(position),
        },
        GestureOutcome::DoubleTap(position) => GestureSemanticEvent::DoubleTap {
            position: point(position),
        },
        GestureOutcome::LongPress(position) => GestureSemanticEvent::LongPress {
            position: point(position),
        },
        GestureOutcome::Swipe { delta, velocity } => GestureSemanticEvent::Swipe {
            delta: point(delta),
            velocity: point(velocity),
        },
        GestureOutcome::Pan { delta, current } => {
            GestureSemanticEvent::pan(point(delta), point(current))
        }
        GestureOutcome::Pinch { scale, center } => {
            GestureSemanticEvent::pinch(scale, 1.0, point(center))
        }
    }
}

fn point(value: Vec2) -> GesturePoint {
    GesturePoint::new(value.x, value.y)
}

fn vec2(value: GesturePoint) -> Vec2 {
    Vec2::new(value.x, value.y)
}

/// The deterministic axis policy for one drag.
fn decide_mode(start: GesturePoint, delta: GesturePoint) -> GestureMode {
    if delta.y.abs() > delta.x.abs() && delta.y > 0.0 && start.y <= PULL_EDGE_PX {
        GestureMode::Pull
    } else if delta.x.abs() > delta.y.abs() {
        GestureMode::Swipe
    } else {
        GestureMode::Recognizer
    }
}

/// The midpoint and separation of the two lowest-id active touches.
fn pinch_geometry(active: &BTreeMap<u64, GesturePoint>) -> Option<(GesturePoint, f32)> {
    let mut touches = active.values();
    let first = *touches.next()?;
    let second = *touches.next()?;
    let center = GesturePoint::new((first.x + second.x) * 0.5, (first.y + second.y) * 0.5);
    Some((center, first.delta(second).length()))
}

/// Consume Bevy touch messages and publish shared semantic events.
fn consume_touch_input(
    mut events: MessageReader<TouchInput>,
    mut state: ResMut<ShellGestureState>,
    mut snapshot: ResMut<ShellGestureSnapshot>,
    mut report: ResMut<GestureHostReport>,
    time: Res<Time>,
) {
    let now_ms = time.elapsed().as_millis() as u64;
    for event in events.read() {
        let position = GesturePoint::new(event.position.x, event.position.y);
        let phase = shared_phase(event.phase);
        snapshot.0.apply(phase.touch_event(position));

        match event.phase {
            TouchPhase::Started => {
                if state.active.len() < MAX_TRACKED_TOUCHES {
                    state.active.insert(event.id, position);
                }
                state.start = Some(position);
                state.pinch_distance = None;
                state.mode = if state.active.len() >= 2 {
                    // A second finger makes the sequence a pinch; drop the
                    // single-pointer start so the recognizer never sees a
                    // stale touch.
                    state
                        .recognizer
                        .handle_touch(gesture::TouchPhase::Cancel, now_ms);
                    GestureMode::Pinch
                } else {
                    GestureMode::Undecided
                };
                if state.mode == GestureMode::Undecided
                    && let Some(outcome) = state
                        .recognizer
                        .handle_touch(gesture::TouchPhase::Start(vec2(position)), now_ms)
                {
                    snapshot.0.apply(shared_outcome(outcome));
                }
            }
            TouchPhase::Moved => {
                if let Some(slot) = state.active.get_mut(&event.id) {
                    *slot = position;
                }
                match state.mode {
                    GestureMode::Pinch => {
                        if let Some((center, distance)) = pinch_geometry(&state.active) {
                            if let Some(previous) = state.pinch_distance
                                && previous > 0.0
                            {
                                let scale_delta = distance / previous;
                                state.pinch.apply_pinch(scale_delta, vec2(center));
                                snapshot.0.apply(GestureSemanticEvent::pinch(
                                    scale_delta,
                                    state.pinch.zoom_level,
                                    center,
                                ));
                            }
                            state.pinch_distance = Some(distance);
                        }
                    }
                    GestureMode::Pull => {
                        if let Some(start) = state.start {
                            let delta = position.delta(start);
                            state.pull.pull(delta.y);
                            snapshot.0.apply(GestureSemanticEvent::pull_to_refresh(
                                state.pull.pull_offset,
                                state.pull.threshold,
                                state.pull.is_refreshing,
                            ));
                        }
                    }
                    GestureMode::Swipe => {
                        if let Some(start) = state.start {
                            let delta = position.delta(start);
                            state.swipe.apply_drag(delta.x);
                            snapshot.0.apply(GestureSemanticEvent::swipe_to_action(
                                state.swipe.offset_x,
                                state.swipe.max_action_width,
                            ));
                        }
                    }
                    GestureMode::Recognizer => {
                        if let Some(outcome) = state
                            .recognizer
                            .handle_touch(gesture::TouchPhase::Move(vec2(position)), now_ms)
                        {
                            snapshot.0.apply(shared_outcome(outcome));
                        }
                    }
                    GestureMode::Undecided => {
                        let Some(start) = state.start else {
                            continue;
                        };
                        let delta = position.delta(start);
                        if delta.length() <= GESTURE_DIRECTION_LOCK_PX {
                            continue;
                        }
                        state.mode = decide_mode(start, delta);
                        match state.mode {
                            GestureMode::Pull => {
                                state.pull.pull(delta.y);
                                snapshot.0.apply(GestureSemanticEvent::pull_to_refresh(
                                    state.pull.pull_offset,
                                    state.pull.threshold,
                                    state.pull.is_refreshing,
                                ));
                            }
                            GestureMode::Swipe => {
                                state.swipe.apply_drag(delta.x);
                                snapshot.0.apply(GestureSemanticEvent::swipe_to_action(
                                    state.swipe.offset_x,
                                    state.swipe.max_action_width,
                                ));
                            }
                            GestureMode::Recognizer | GestureMode::Undecided => {
                                if let Some(outcome) = state
                                    .recognizer
                                    .handle_touch(gesture::TouchPhase::Move(vec2(position)), now_ms)
                                {
                                    snapshot.0.apply(shared_outcome(outcome));
                                }
                            }
                            GestureMode::Pinch => {}
                        }
                    }
                }
            }
            TouchPhase::Ended => {
                state.active.remove(&event.id);
                match state.mode {
                    GestureMode::Pull => {
                        state.pull.release();
                        snapshot.0.apply(GestureSemanticEvent::pull_to_refresh(
                            state.pull.pull_offset,
                            state.pull.threshold,
                            state.pull.is_refreshing,
                        ));
                    }
                    GestureMode::Swipe => {
                        state.swipe.settle();
                        snapshot.0.apply(GestureSemanticEvent::swipe_to_action(
                            state.swipe.offset_x,
                            state.swipe.max_action_width,
                        ));
                    }
                    GestureMode::Recognizer | GestureMode::Undecided => {
                        if let Some(outcome) = state
                            .recognizer
                            .handle_touch(gesture::TouchPhase::End(vec2(position)), now_ms)
                        {
                            snapshot.0.apply(shared_outcome(outcome));
                        }
                    }
                    GestureMode::Pinch => {}
                }
                state.mode = GestureMode::Undecided;
                state.start = None;
                state.pinch_distance = None;
            }
            TouchPhase::Canceled => {
                state.active.remove(&event.id);
                state
                    .recognizer
                    .handle_touch(gesture::TouchPhase::Cancel, now_ms);
                state.mode = GestureMode::Undecided;
                state.start = None;
                state.pinch_distance = None;
            }
        }
    }
    report.tracked_touches = state.active.len();
}

/// Publish the host capability and safe-area insets into the shared snapshot
/// whenever they change.
fn sync_gesture_capability(
    report: Res<GestureHostReport>,
    mut snapshot: ResMut<ShellGestureSnapshot>,
) {
    if snapshot.0.capability != report.support {
        snapshot
            .0
            .apply(GestureSemanticEvent::TouchCapability(report.support));
    }
    if snapshot.0.insets != report.insets {
        snapshot
            .0
            .apply(GestureSemanticEvent::SafeAreaInsets(report.insets));
    }
}
