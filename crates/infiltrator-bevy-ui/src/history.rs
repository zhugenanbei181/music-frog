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

use std::collections::VecDeque;

use bevy::ecs::resource::Resource;

use crate::projection::OverviewOrigin;

/// Ring capacity: ~60 pump ticks ≈ 42s at the 700ms cadence — one screen
/// of recent shape, matching the reference card's rolling window.
pub const TRAFFIC_HISTORY_CAPACITY: usize = 60;

/// The (upload, download) rate history, oldest → newest. Rates ride as
/// `f32` because that is what the chart's polyline projection consumes;
/// the ring is the only state, so `Clone`/`PartialEq` come for free.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct TrafficHistory {
    samples: VecDeque<(f32, f32)>,
}

impl TrafficHistory {
    /// Append one sample; at capacity the oldest is evicted first (a
    /// fixed-size ring — the buffer never grows past
    /// [`TRAFFIC_HISTORY_CAPACITY`]).
    pub fn push(&mut self, upload_bps: f64, download_bps: f64) {
        if self.samples.len() == TRAFFIC_HISTORY_CAPACITY {
            self.samples.pop_front();
        }
        self.samples
            .push_back((sanitize_rate(upload_bps), sanitize_rate(download_bps)));
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
        std::mem::swap(&mut self.front, &mut self.back);
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
