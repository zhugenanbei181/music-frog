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
//! Bevy 0.20 exposes no safe-area API. Android's NativeActivity supplies typed
//! physical-pixel system-bar, cutout, system-gesture and IME observations
//! through `WindowInsetsInput`; the shell converts them using display density
//! before publishing logical safe-area values through [`GestureHostReport`].
//! Zero is only the initial value before the host's first observation arrives.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::host_capabilities::{HostPreferenceIntent, MotionPreference};
use crate::pages::outbound_batch_probe::{OutboundBatchProbeReadback, submit_batch};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::MessageReader;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Local, Query, Res, ResMut, SystemParam};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::math::Vec2;
use bevy::time::Time;
use bevy::ui::prelude::{Display, Node, ScrollPosition, Val};
use bevy::ui_widgets::ScrollArea;
use infiltrator_bevy_widgets::gesture;
use infiltrator_bevy_widgets::gesture::{
    GestureOutcome, GestureRecognizer, PinchZoomController, PullToRefreshIndicator,
    PullToRefreshPhase, PullToRefreshSpring, PullToRefreshState, PullToRefreshText,
    SwipeActionDrawer, SwipeContentContainer, SwipeToActionItem, SwipeToActionSpring,
};
use infiltrator_bevy_widgets::localization::{
    LocaleCopySet, LocalizedText, WidgetLocalizationPlugin,
};
use infiltrator_bevy_widgets::scrollarea::ScrollMotionPreference;
use infiltrator_contract::shell_gesture::{
    GesturePoint, GestureSemanticEvent, GestureSnapshot, GestureTouchPhase, PullPhase,
    SafeAreaInsets, TouchGestureSupport,
};
use infiltrator_shared::i18n_interpolator::localize;
use lurker_core_contract::outbound::probe::OutboundProbeBatchScope;
use std::collections::BTreeMap;

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

    /// The widget pull-to-refresh state this shell drives.
    pub fn pull(&self) -> &PullToRefreshState {
        &self.pull
    }

    /// Mutable pull-to-refresh state.
    pub fn pull_mut(&mut self) -> &mut PullToRefreshState {
        &mut self.pull
    }
}

/// The bounded shared snapshot as a Bevy resource.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ShellGestureSnapshot(pub GestureSnapshot);

/// The host facts of this surface, inserted by [`ShellGesturePlugin`].
///
/// `insets` is the logical safe-area projection written by `HostInsetsPlugin`
/// from a host `WindowInsetsInput`; [`sync_gesture_capability`] publishes the
/// same values into the shared snapshot.
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

/// The four mutually exclusive diagnostic outcomes of an Android touch sequence (BANDROID-025.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchLayerDiagnosticCategory {
    /// MotionEvent was received by the Android host but no winit TouchInput event entered Bevy.
    TouchNotEnteredBevy,
    /// Touch arrived in Bevy but Picking interaction did not hit any UI entity.
    TouchMissedUi,
    /// Drag gesture or TouchInput::Moved occurred, but ScrollPosition was not driven (delta == 0).
    DragNotDrivenScroll,
    /// ScrollPosition changed, but no render presentation or visual offset update occurred.
    PositionNotRendered,
}

impl TouchLayerDiagnosticCategory {
    pub const fn label(self) -> &'static str {
        match self {
            Self::TouchNotEnteredBevy => "touch_not_entered_bevy",
            Self::TouchMissedUi => "touch_missed_ui",
            Self::DragNotDrivenScroll => "drag_not_driven_scroll",
            Self::PositionNotRendered => "position_not_rendered",
        }
    }

    pub const fn description_key(self) -> &'static str {
        match self {
            Self::TouchNotEnteredBevy => "touch_diag_not_entered_bevy",
            Self::TouchMissedUi => "touch_diag_missed_ui",
            Self::DragNotDrivenScroll => "touch_diag_drag_not_driven_scroll",
            Self::PositionNotRendered => "touch_diag_position_not_rendered",
        }
    }

    pub fn description(self) -> String {
        self.description_in("zh-CN")
    }

    pub fn description_in(self, locale: &str) -> String {
        localize(locale, self.description_key(), &[])
    }
}

/// Diagnostic sample tracking the touch pipeline through all 4 layers.
#[derive(Clone, Debug, PartialEq)]
pub struct TouchLayerDiagnosticEntry {
    /// Scenario identifier (shared with Compose, e.g. "SCN-01-OVERVIEW", "SCN-05-CONNECTIONS").
    pub scenario_id: String,
    /// Raw touch coordinates from Android MotionEvent (if known).
    pub raw_coords: Option<Vec2>,
    /// Screen density (dpi or scale factor).
    pub density: f32,
    /// Winit/Bevy TouchInput phase and coordinates.
    pub touch_input: Option<(TouchPhase, Vec2)>,
    /// Picking entity hit by this touch (if any).
    pub picking_hit: Option<Entity>,
    /// Recognized gesture mode (Pull, Swipe, Pinch, Recognizer, etc.).
    pub gesture_mode: String,
    /// Scroll position before and after this event (x, y).
    pub scroll_before: Option<Vec2>,
    pub scroll_after: Option<Vec2>,
    /// Was a render frame submitted for this position change?
    pub rendered: bool,
}

impl TouchLayerDiagnosticEntry {
    /// Classifies this touch sequence into one of the four required failure categories,
    /// or None if the touch was processed and rendered successfully.
    pub fn classify(&self) -> Option<TouchLayerDiagnosticCategory> {
        if self.touch_input.is_none() {
            return Some(TouchLayerDiagnosticCategory::TouchNotEnteredBevy);
        }
        if self.picking_hit.is_none() {
            return Some(TouchLayerDiagnosticCategory::TouchMissedUi);
        }
        if let (Some(before), Some(after)) = (self.scroll_before, self.scroll_after) {
            let is_drag = self.gesture_mode == "Pull"
                || self.gesture_mode == "Swipe"
                || self.gesture_mode == "Recognizer"
                || matches!(self.touch_input, Some((TouchPhase::Moved, _)));
            if is_drag && (after - before).length_squared() < 0.0001 {
                return Some(TouchLayerDiagnosticCategory::DragNotDrivenScroll);
            }
            if (after - before).length_squared() >= 0.0001 && !self.rendered {
                return Some(TouchLayerDiagnosticCategory::PositionNotRendered);
            }
        }
        None
    }

    /// Formats a structured diagnostic log line for logcat / automated log inspection.
    pub fn format_log(&self) -> String {
        let category = self.classify().map_or("SUCCESS", |c| c.label());
        format!(
            "[TOUCH_DIAG] scenario={} category={} touch={:?} picking={:?} gesture={} scroll_delta={:?} rendered={}",
            self.scenario_id,
            category,
            self.touch_input,
            self.picking_hit,
            self.gesture_mode,
            match (self.scroll_before, self.scroll_after) {
                (Some(b), Some(a)) => Some(a - b),
                _ => None,
            },
            self.rendered
        )
    }
}

/// In-memory diagnostic log resource for development/diagnostic mode.
#[derive(Resource, Default)]
pub struct TouchLayerDiagnosticLog {
    pub enabled: bool,
    pub current_scenario: String,
    pub entries: Vec<TouchLayerDiagnosticEntry>,
}

/// Installs the real touch consumer and the shared snapshot.
pub struct ShellGesturePlugin;

impl Plugin for ShellGesturePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<WidgetLocalizationPlugin>() {
            app.add_plugins(WidgetLocalizationPlugin);
        }
        // Headless compositions have no `InputPlugin`, so register the touch
        // message channel and the resources here (registration is idempotent
        // when `DefaultPlugins` already did it).
        app.add_message::<TouchInput>()
            .init_resource::<ShellGestureState>()
            .init_resource::<GestureHostReport>()
            .insert_resource(ShellGestureSnapshot(GestureSnapshot::new(touch_support())))
            .add_systems(
                Update,
                (
                    consume_touch_input,
                    sync_gesture_capability,
                    sync_pull_to_refresh_indicators.before(LocaleCopySet),
                    sync_swipe_to_action_items,
                )
                    .chain(),
            )
            .add_observer(sync_scroll_motion_preference);
    }
}

/// Map the shell's typed host accessibility preference to scroll physics.
fn sync_scroll_motion_preference(
    intent: On<HostPreferenceIntent>,
    preference: Option<ResMut<ScrollMotionPreference>>,
) {
    let Some(mut preference) = preference else {
        return;
    };
    let next = match intent.0.effective_motion() {
        MotionPreference::Reduced => ScrollMotionPreference::Reduced,
        MotionPreference::Standard => ScrollMotionPreference::Full,
    };
    if *preference != next {
        *preference = next;
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

pub fn map_pull_phase(phase: PullToRefreshPhase) -> PullPhase {
    match phase {
        PullToRefreshPhase::Idle => PullPhase::Idle,
        PullToRefreshPhase::Pulling => PullPhase::Pulling,
        PullToRefreshPhase::Armed => PullPhase::Armed,
        PullToRefreshPhase::Refreshing => PullPhase::Refreshing,
    }
}

fn point(value: Vec2) -> GesturePoint {
    GesturePoint::new(value.x, value.y)
}

fn vec2(value: GesturePoint) -> Vec2 {
    Vec2::new(value.x, value.y)
}

/// The deterministic axis policy for one drag.
fn decide_mode(start: GesturePoint, delta: GesturePoint, is_scrolled: bool) -> GestureMode {
    if delta.y.abs() > delta.x.abs() && delta.y > 0.0 && start.y <= PULL_EDGE_PX && !is_scrolled {
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
    scroll_positions: Query<&ScrollPosition, With<ScrollArea>>,
    time: Res<Time>,
) {
    let is_scrolled = scroll_positions.iter().any(|pos| pos.y > 1.0);
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
                                map_pull_phase(state.pull.phase),
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
                        state.mode = decide_mode(start, delta, is_scrolled);
                        match state.mode {
                            GestureMode::Pull => {
                                state.pull.pull(delta.y);
                                snapshot.0.apply(GestureSemanticEvent::pull_to_refresh(
                                    state.pull.pull_offset,
                                    state.pull.threshold,
                                    map_pull_phase(state.pull.phase),
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
                            map_pull_phase(state.pull.phase),
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

/// Typed resources and queries used by the indicator and refresh dispatch.
#[derive(SystemParam)]
pub struct PullToRefreshIndicatorParams<'w, 's> {
    time: Option<Res<'w, Time>>,
    gesture_state: Res<'w, ShellGestureState>,
    active_route: Option<Res<'w, ActiveRoute>>,
    command_sink: Option<Res<'w, CommandSinkHandle>>,
    latest: Option<Res<'w, LatestSurfaceSnapshot>>,
    batch_readback: Option<ResMut<'w, OutboundBatchProbeReadback>>,
    last_refreshing: Local<'s, bool>,
    indicators: Query<
        'w,
        's,
        (
            &'static mut Node,
            &'static mut PullToRefreshSpring,
            &'static Children,
        ),
        With<PullToRefreshIndicator>,
    >,
    texts: Query<'w, 's, &'static mut LocalizedText, With<PullToRefreshText>>,
}

/// Synchronize pull-to-refresh indicators with the live gesture state using spring physics.
pub fn sync_pull_to_refresh_indicators(params: PullToRefreshIndicatorParams) {
    let PullToRefreshIndicatorParams {
        time,
        gesture_state,
        active_route,
        command_sink,
        latest,
        mut batch_readback,
        mut last_refreshing,
        mut indicators,
        mut texts,
    } = params;
    let dt = time
        .as_ref()
        .map(|t| t.delta_secs())
        .filter(|&dt| dt > 0.001)
        .unwrap_or(1.0 / 60.0);
    let pull = &gesture_state.pull;
    let is_refreshing = pull.phase.is_refreshing();

    // Trigger command when entering refreshing state
    if is_refreshing
        && !*last_refreshing
        && let Some(sink) = command_sink.as_ref()
    {
        let route = active_route
            .as_ref()
            .and_then(|r| r.0)
            .unwrap_or(Route::Overview);
        match route {
            Route::Proxies => {
                if let (Some(latest), Some(readback)) =
                    (latest.as_deref(), batch_readback.as_deref_mut())
                {
                    submit_batch(
                        &latest.0,
                        OutboundProbeBatchScope::All,
                        route,
                        sink,
                        readback,
                    );
                }
            }
            Route::Rules => {
                sink.submit(UiCommand::RefreshRuleProviders);
            }
            Route::Profiles => {
                sink.submit(UiCommand::UpdateAllSubscriptions);
            }
            _ => {}
        }
    }
    *last_refreshing = is_refreshing;

    // Spring target height calculation
    let target_height = if is_refreshing {
        pull.threshold
    } else if gesture_state.mode == GestureMode::Pull {
        pull.pull_offset.min(pull.threshold * 1.5)
    } else {
        0.0
    };

    let key = pull.label_key();

    for (mut node, mut spring_tracker, children) in &mut indicators {
        spring_tracker.spring.target = target_height;
        let current_height = spring_tracker.spring.step(dt);
        let display_height = if current_height <= 0.2 && target_height == 0.0 {
            0.0
        } else {
            current_height.max(0.0)
        };
        node.height = Val::Px(display_height);

        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(*child)
                && text.key != key
            {
                text.key = key;
            }
        }
    }
}

/// Synchronize swipe-to-action item displacement using spring dynamics.
pub fn sync_swipe_to_action_items(
    time: Option<Res<Time>>,
    gesture_state: Res<ShellGestureState>,
    mut swipe_items: Query<(&mut SwipeToActionItem, &mut SwipeToActionSpring, &Children)>,
    mut swipe_contents: Query<&mut Node, With<SwipeContentContainer>>,
    mut swipe_drawers: Query<&mut Node, (With<SwipeActionDrawer>, Without<SwipeContentContainer>)>,
) {
    let dt = time
        .as_ref()
        .map(|t| t.delta_secs())
        .filter(|&dt| dt > 0.001)
        .unwrap_or(1.0 / 60.0);
    let active_swipe_offset = if gesture_state.mode == GestureMode::Swipe {
        Some(gesture_state.swipe.offset_x)
    } else {
        None
    };

    for (mut item, mut spring_tracker, children) in &mut swipe_items {
        if let Some(offset) = active_swipe_offset {
            item.offset_x = offset.clamp(-item.max_action_width, item.max_action_width);
        }
        spring_tracker.spring.target = item.offset_x;
        let current_x = spring_tracker.spring.step(dt);
        let display_x = if current_x.abs() <= 0.2 && item.offset_x == 0.0 {
            0.0
        } else {
            current_x
        };

        for child in children.iter() {
            if let Ok(mut node) = swipe_contents.get_mut(*child) {
                node.left = Val::Px(display_x);
            }
            if let Ok(mut drawer_node) = swipe_drawers.get_mut(*child) {
                let drawer_display = if display_x.abs() > 0.5 {
                    Display::Flex
                } else {
                    Display::None
                };
                if drawer_node.display != drawer_display {
                    drawer_node.display = drawer_display;
                }
            }
        }
    }
}
