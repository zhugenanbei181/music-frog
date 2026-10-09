//! The Overview traffic card's rate-history buffer: the (upload, download)
//! sample ring behind the trend chart, plus the demo fixture's synthetic
//! series.
//!
//! **Where the buffer lives**: a plain bevy `Resource`
//! ([`TrafficHistory`]) next to the projection seam, not page-local
//! components — the producer is the live pump's frame drain
//! ([`crate::controller::drain_overview_pump`], which appends every
//! *delivered* snapshot at the drain site) while the consumer is the page's
//! refresh observer (which restamps the chart plate from it). A resource
//! is the one shape both ends already share, and it survives page
//! remounts, so a re-routed page re-draws the full window immediately.
//!
//! **Honesty split**: a live-origin projection charts exactly what the
//! pump measured — the ring, oldest → newest, never padded or smoothed;
//! before the first sample the chart is the honest empty/grid state. The
//! demo fixture's rates are constants, so its trend is constants too —
//! the reference card's waves come from [`demo_traffic_series`], a pure,
//! fixed-seed sine superposition (the whole demo card is fixture data;
//! the banner already says 演示数据). [`chart_series`] is the one
//! origin → chart-input decision, so the two arms cannot drift.
//!
//! **Time travel (BEVY-036)**: every push also lands in a bounded
//! [`TelemetryStore`], and [`TrafficReplay`] freezes it into a
//! [`ReplayCursor`] for the Overview scrubber. [`replay_state`] is the
//! typed no-history outcome (`Unsupported` for the demo fixture, `Empty`
//! for a live source before its first sample) — the chart draws an empty
//! grid and never fabricates a series. [`replay_chart_inputs`] is the one
//! replay → chart-input decision.

use crate::projection::{OverviewOrigin, OverviewProjection};
use bevy::ecs::resource::Resource;
use infiltrator_bevy_widgets::tsdb::{
    DEFAULT_STORE_CAPACITY, ReplayCursor, TelemetrySample, TelemetryStore, downsample,
};
use infiltrator_contract::traffic_scale::TrafficScaleSnapshot;
use infiltrator_domain::traffic_scale::compute_from_rates;
use infiltrator_domain::traffic_waveform::display_series;
use std::collections::VecDeque;
use std::mem::swap;

/// Ring capacity: ~60 pump ticks ≈ 42s at the 700ms cadence — one screen
/// of recent shape, matching the reference card's rolling window.
pub const TRAFFIC_HISTORY_CAPACITY: usize = 60;

/// The (upload, download) rate history, oldest → newest. Rates ride as
/// `f32` because that is what the chart's polyline projection consumes.
///
/// The same pushes also feed a bounded [`TelemetryStore`] (`store`) — the
/// delta-compressed time-series behind the time-travel scrubber. The two
/// rings serve different budgets: `samples` is the 60-point chart window,
/// `store` retains up to [`DEFAULT_STORE_CAPACITY`] samples for replay.
/// Both are fixed-size, so growth is bounded no matter how long the pump
/// runs.
#[derive(Resource, Clone, Debug)]
pub struct TrafficHistory {
    samples: VecDeque<(f32, f32)>,
    store: TelemetryStore,
    next_tick: u64,
}

impl Default for TrafficHistory {
    fn default() -> Self {
        Self {
            samples: VecDeque::new(),
            store: TelemetryStore::new(DEFAULT_STORE_CAPACITY),
            next_tick: 0,
        }
    }
}

impl TrafficHistory {
    /// Append one sample; at capacity the oldest is evicted first (a
    /// fixed-size ring — the buffer never grows past
    /// [`TRAFFIC_HISTORY_CAPACITY`]). The same sample lands in the bounded
    /// time-series store under a monotonic tick timestamp.
    pub fn push(&mut self, upload_bps: f64, download_bps: f64) {
        let upload = sanitize_rate(upload_bps);
        let download = sanitize_rate(download_bps);
        if self.samples.len() == TRAFFIC_HISTORY_CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back((upload, download));
        let timestamp_sec = self.next_tick;
        self.next_tick = self.next_tick.saturating_add(1);
        self.store.push(TelemetrySample {
            timestamp_sec,
            upload_bytes: upload as u64,
            download_bytes: download as u64,
            active_connections: 0,
            latency_ms: 0.0,
        });
    }

    /// The retained time-series samples, oldest → newest. Empty until the
    /// first push — never padded.
    pub fn retained_samples(&self) -> Vec<TelemetrySample> {
        self.store.to_vec()
    }

    /// How many samples the time-series store retains.
    pub fn store_len(&self) -> usize {
        self.store.len()
    }

    /// The time-series store's retained-sample ceiling.
    pub fn store_capacity(&self) -> usize {
        self.store.capacity()
    }

    /// The upload series, oldest → newest (the chart polyline's input
    /// order — the newest sample lands on the right edge).
    pub fn upload_series(&self) -> Vec<f32> {
        self.samples.iter().map(|(up, _)| *up).collect()
    }

    /// The download series, oldest → newest.
    pub fn download_series(&self) -> Vec<f32> {
        self.samples.iter().map(|(_, down)| *down).collect()
    }

    /// How many samples are currently held.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether nothing has been recorded yet (the live chart's honest
    /// empty state).
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// Clamp one rate into chart-safe `f32`: the projection's non-finite or
/// negative rates render as the honest zero the rate *text* already shows
/// (`format_rate`), never as a fabricated spike or a polyline gap the
/// digits don't agree with. Pure function.
fn sanitize_rate(rate: f64) -> f32 {
    if rate.is_finite() && rate > 0.0 {
        rate as f32
    } else {
        0.0
    }
}

/// The demo fixture's trend: a deterministic sine superposition (fixed
/// frequencies and phases — no clock, no RNG, so headless tests and every
/// capture see the identical curve). Units are the fixture's own scale;
/// the chart normalizes min→bottom / max→top per series, so only the wave
/// shape is load-bearing. Pure function.
pub fn demo_traffic_series() -> (Vec<f32>, Vec<f32>) {
    let sample = |i: usize, a: f32, f_a: f32, p_a: f32, b: f32, f_b: f32, p_b: f32| {
        let t = i as f32;
        a * (t * f_a + p_a).sin() + b * (t * f_b + p_b).sin()
    };
    let up = (0..TRAFFIC_HISTORY_CAPACITY)
        .map(|i| sample(i, 1.0, 0.31, 0.0, 0.35, 0.11, 1.7))
        .collect();
    let down = (0..TRAFFIC_HISTORY_CAPACITY)
        .map(|i| sample(i, 0.8, 0.19, 0.6, 0.3, 0.07, 4.2))
        .collect();
    (up, down)
}

/// The chart inputs for one projection origin: the demo fixture draws its
/// synthetic trend, a live core draws the measured ring. Pure function —
/// the single decision point the scene mount and the refresh observer
/// both spell through.
pub fn chart_series(origin: OverviewOrigin, history: &TrafficHistory) -> (Vec<f32>, Vec<f32>) {
    match origin {
        OverviewOrigin::Demo => demo_traffic_series(),
        OverviewOrigin::LiveCore => (history.upload_series(), history.download_series()),
    }
}

/// Replay the application-owned live waveform even when it is empty or has one sample.
/// The boolean tells the Bevy chart whether the values are
/// already Bezier-densified by the shared domain algorithm.
pub fn chart_inputs(
    projection: &OverviewProjection,
    history: &TrafficHistory,
) -> (Vec<f32>, Vec<f32>, bool, TrafficScaleSnapshot) {
    if projection.origin == OverviewOrigin::LiveCore {
        let (upload, download) = display_series(&projection.traffic_waveform);
        (upload, download, false, projection.traffic_scale.clone())
    } else {
        let (upload, download) = chart_series(projection.origin, history);
        let upload_raw: Vec<f64> = upload.iter().map(|value| *value as f64).collect();
        let download_raw: Vec<f64> = download.iter().map(|value| *value as f64).collect();
        (
            upload,
            download,
            true,
            compute_from_rates(
                &upload_raw,
                &download_raw,
                projection.traffic_waveform.revision,
            ),
        )
    }
}

// ---- time-travel replay scrubber (BEVY-036) --------------------------------

/// How much of the frozen snapshot one seek jumps.
pub const SCRUB_SEEK_FRACTION: f32 = 0.1;
/// The replay window renders at most this many points (the chart budget).
pub const REPLAY_WINDOW_POINTS: usize = TRAFFIC_HISTORY_CAPACITY;
/// The replay window spans this many retained samples before downsampling,
/// so a long history is reduced to [`REPLAY_WINDOW_POINTS`] honest buckets.
pub const REPLAY_WINDOW_SPAN: usize = TRAFFIC_HISTORY_CAPACITY * 4;

/// One scrubber activation: a coarse seek or a single-sample step.
///
/// `Default` exists only so the button marker can ride a `bsn!` scene
/// component; every mounted button carries an explicit action.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrubberAction {
    /// Jump backward by [`SCRUB_SEEK_FRACTION`] of the frozen snapshot.
    #[default]
    SeekBackward,
    /// Land on the previous retained sample.
    StepBackward,
    /// Land on the next retained sample.
    StepForward,
    /// Jump forward by [`SCRUB_SEEK_FRACTION`] of the frozen snapshot.
    SeekForward,
    /// Leave time travel and resume the live window.
    ReturnToLive,
}

/// The typed scrubber state the Overview chart renders. `Unsupported` and
/// `Empty` are the honest no-history outcomes — the chart draws an empty
/// grid and no sample is ever fabricated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayState {
    /// Not time-travelling: the chart draws the live window.
    Live,
    /// This source has no measured history to replay (the demo fixture's
    /// trend is synthetic).
    Unsupported,
    /// Time travel is available but nothing has been recorded yet.
    Empty,
    /// Time-travelling a frozen snapshot at `index` of `len` samples.
    Historical { index: usize, len: usize },
}

/// Which side of the live/replay seam the scrubber is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ReplayMode {
    #[default]
    Live,
    Scrubbing,
}

/// The Overview chart's time-travel state: a frozen snapshot of the
/// retained store plus a [`ReplayCursor`] over it. The cursor walks the
/// snapshot while the store keeps absorbing live samples, so scrubbing is
/// stable and never races the pump.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct TrafficReplay {
    mode: ReplayMode,
    snapshot: Vec<TelemetrySample>,
    cursor: ReplayCursor,
}

impl Default for TrafficReplay {
    fn default() -> Self {
        Self {
            mode: ReplayMode::Live,
            snapshot: Vec::new(),
            cursor: ReplayCursor::new(Vec::new()),
        }
    }
}

impl TrafficReplay {
    /// Whether the chart is currently time-travelling.
    pub fn is_scrubbing(&self) -> bool {
        self.mode == ReplayMode::Scrubbing
    }

    /// Freeze the retained store and land on its newest sample. Returns
    /// `false` when nothing was retained (the typed empty outcome).
    pub fn begin(&mut self, history: &TrafficHistory) -> bool {
        self.snapshot = history.retained_samples();
        self.cursor = ReplayCursor::new(self.snapshot.clone());
        if !self.snapshot.is_empty() {
            self.cursor.seek_to_index(self.snapshot.len() - 1);
        }
        self.mode = ReplayMode::Scrubbing;
        !self.snapshot.is_empty()
    }

    /// Leave time travel and resume the live window.
    pub fn return_to_live(&mut self) {
        self.mode = ReplayMode::Live;
        self.snapshot.clear();
        self.cursor = ReplayCursor::new(Vec::new());
    }

    /// Apply one scrubber action. A step or seek first freezes the store, so
    /// the first interaction lands on the newest retained sample.
    pub fn apply(&mut self, history: &TrafficHistory, action: ScrubberAction) {
        if action == ScrubberAction::ReturnToLive {
            self.return_to_live();
            return;
        }
        if !self.is_scrubbing() {
            self.begin(history);
        }
        match action {
            ScrubberAction::SeekBackward => {
                let fraction = (self.fraction() - SCRUB_SEEK_FRACTION).max(0.0);
                self.cursor.seek_to_fraction(fraction);
            }
            ScrubberAction::SeekForward => {
                let fraction = (self.fraction() + SCRUB_SEEK_FRACTION).min(1.0);
                self.cursor.seek_to_fraction(fraction);
            }
            ScrubberAction::StepBackward => {
                self.cursor.step_backward();
            }
            ScrubberAction::StepForward => {
                self.cursor.step_forward();
            }
            ScrubberAction::ReturnToLive => unreachable!("handled above"),
        }
    }

    /// Number of samples in the frozen snapshot (0 while live).
    pub fn snapshot_len(&self) -> usize {
        self.snapshot.len()
    }

    /// The sample under the cursor, if any.
    pub fn current(&self) -> Option<TelemetrySample> {
        self.cursor.current()
    }

    fn fraction(&self) -> f32 {
        let len = self.snapshot.len();
        if len <= 1 {
            1.0
        } else {
            self.cursor.index() as f32 / (len - 1) as f32
        }
    }

    /// The historical window ending at the cursor as chart series
    /// `(upload, download)`, downsampled to [`REPLAY_WINDOW_POINTS`].
    /// `None` while live or with no retained history — never a fabricated
    /// series.
    pub fn replay_series(&self) -> Option<(Vec<f32>, Vec<f32>)> {
        if !self.is_scrubbing() || self.snapshot.is_empty() {
            return None;
        }
        let end = self.cursor.index().min(self.snapshot.len() - 1);
        let start = end.saturating_sub(REPLAY_WINDOW_SPAN - 1);
        let window = downsample(&self.snapshot[start..=end], REPLAY_WINDOW_POINTS);
        Some((
            window
                .iter()
                .map(|sample| sample.upload_bytes as f32)
                .collect(),
            window
                .iter()
                .map(|sample| sample.download_bytes as f32)
                .collect(),
        ))
    }
}

/// The typed scrubber state for `origin` and `replay`. The demo fixture's
/// synthetic trend has no measured history (`Unsupported`); a live source
/// with an empty store is `Empty`; otherwise `Historical`.
pub fn replay_state(origin: OverviewOrigin, replay: &TrafficReplay) -> ReplayState {
    if !replay.is_scrubbing() {
        return ReplayState::Live;
    }
    if origin == OverviewOrigin::Demo {
        return ReplayState::Unsupported;
    }
    if replay.snapshot.is_empty() {
        return ReplayState::Empty;
    }
    ReplayState::Historical {
        index: replay.cursor.index(),
        len: replay.snapshot.len(),
    }
}

/// The chart inputs while scrubbing: the historical window under the cursor
/// with a scale derived from the same window. `None` when there is nothing
/// honest to draw (live or empty).
pub fn replay_chart_inputs(
    replay: &TrafficReplay,
) -> Option<(Vec<f32>, Vec<f32>, bool, TrafficScaleSnapshot)> {
    let (upload, download) = replay.replay_series()?;
    let upload_raw: Vec<f64> = upload.iter().map(|value| *value as f64).collect();
    let download_raw: Vec<f64> = download.iter().map(|value| *value as f64).collect();
    let scale = compute_from_rates(&upload_raw, &download_raw, 0);
    Some((upload, download, false, scale))
}

#[cfg(test)]
#[path = "../tests/headless/history_observation_tests.rs"]
mod observation_tests;

#[cfg(test)]
#[path = "../tests/headless/overview_replay_tests.rs"]
mod replay_tests;

/// Double-buffered ring snapshot decoupling async producers from UI render loops.
#[derive(Clone, Debug)]
pub struct DoubleBufferedRing<T: Clone> {
    front: Vec<T>,
    back: Vec<T>,
    capacity: usize,
}

impl<T: Clone> DoubleBufferedRing<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            front: Vec::with_capacity(capacity),
            back: Vec::with_capacity(capacity),
            capacity: capacity.max(1),
        }
    }

    /// Push an item into the write buffer (back). Drops oldest when full.
    pub fn push_back(&mut self, item: T) {
        if self.back.len() >= self.capacity {
            self.back.remove(0);
        }
        self.back.push(item);
    }

    /// Atomically swap back and front buffers at frame boundary. O(1).
    pub fn swap_buffers(&mut self) {
        swap(&mut self.front, &mut self.back);
        // Back buffer copies latest front state as starting point
        self.back.clone_from(&self.front);
    }

    /// Read front buffer snapshot (guaranteed stable for current frame).
    pub fn read_front(&self) -> &[T] {
        &self.front
    }

    pub fn len(&self) -> usize {
        self.front.len()
    }

    pub fn is_empty(&self) -> bool {
        self.front.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ring holds at most [`TRAFFIC_HISTORY_CAPACITY`] samples: past
    /// capacity the oldest is evicted (push order survives as
    /// oldest → newest across the wrap).
    #[test]
    fn push_wraps_at_capacity_keeping_order() {
        let mut history = TrafficHistory::default();
        assert!(history.is_empty());
        for tick in 0..(TRAFFIC_HISTORY_CAPACITY as u64 + 5) {
            history.push(tick as f64, (tick * 2) as f64);
        }
        assert_eq!(history.len(), TRAFFIC_HISTORY_CAPACITY, "hard capacity");
        let up = history.upload_series();
        let down = history.download_series();
        // 65 pushes into 60 slots: samples 5..65 survive, in order.
        assert_eq!(up.first(), Some(&5.0), "the five oldest wrapped away");
        assert_eq!(up.last(), Some(&64.0), "the newest sits at the right edge");
        assert_eq!(down.first(), Some(&10.0));
        assert_eq!(*up.last().unwrap() - up[0], 59.0, "contiguous after wrap");
    }

    /// Non-finite and negative rates clamp to the honest zero (the value
    /// the rate line prints), never NaN into the raster.
    #[test]
    fn push_sanitizes_non_finite_and_negative_rates() {
        let mut history = TrafficHistory::default();
        history.push(f64::NAN, f64::INFINITY);
        history.push(-3.0, 1.5);
        assert_eq!(
            history.upload_series(),
            vec![0.0, 0.0],
            "NaN/negative upload → zero"
        );
        assert_eq!(
            history.download_series(),
            vec![0.0, 1.5],
            "negative download → zero, positive survives"
        );
    }

    /// The demo trend is a pure fixture: identical across calls, full
    /// window length, and actually wavy (non-constant, all finite — the
    /// rasterizer would drop non-finite samples as gaps).
    #[test]
    fn demo_series_is_deterministic_and_wavy() {
        let (up_a, down_a) = demo_traffic_series();
        let (up_b, down_b) = demo_traffic_series();
        assert_eq!(up_a, up_b, "fixed seed: identical every call");
        assert_eq!(down_a, down_b);
        assert_eq!(up_a.len(), TRAFFIC_HISTORY_CAPACITY);
        for series in [&up_a, &down_a] {
            assert!(series.iter().all(|v| v.is_finite()), "no gaps");
            assert!(
                series.iter().cloned().reduce(f32::max).unwrap()
                    > series.iter().cloned().reduce(f32::min).unwrap(),
                "the wave actually moves"
            );
        }
    }

    /// The origin decision: demo draws the synthetic trend regardless of
    /// any measured ring; live draws exactly the recorded ring.
    #[test]
    fn chart_series_follows_the_origin() {
        let (demo_up, demo_down) = demo_traffic_series();
        let mut history = TrafficHistory::default();
        history.push(10.0, 20.0);
        history.push(30.0, 40.0);

        let (up, down) = chart_series(OverviewOrigin::Demo, &history);
        assert_eq!(up, demo_up, "demo ignores the ring");
        assert_eq!(down, demo_down);

        let (up, down) = chart_series(OverviewOrigin::LiveCore, &history);
        assert_eq!(up, vec![10.0, 30.0], "live draws the measured ring");
        assert_eq!(down, vec![20.0, 40.0]);

        // A live core with no samples yet stays honestly empty.
        let (up, down) = chart_series(OverviewOrigin::LiveCore, &TrafficHistory::default());
        assert!(up.is_empty() && down.is_empty());
    }
    #[test]
    fn test_double_buffered_ring_swap_and_isolation() {
        let mut ring = DoubleBufferedRing::<u32>::new(3);
        assert!(ring.is_empty());

        ring.push_back(10);
        ring.push_back(20);
        // Before swap: front is still empty
        assert!(ring.is_empty());

        // Swap at frame boundary
        ring.swap_buffers();
        assert_eq!(ring.read_front(), &[10, 20]);

        // Push to back while front is being read
        ring.push_back(30);
        assert_eq!(ring.read_front(), &[10, 20]); // Front is completely isolated

        // Next swap propagates 30
        ring.swap_buffers();
        assert_eq!(ring.read_front(), &[10, 20, 30]);
    }
}
