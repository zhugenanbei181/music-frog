//! Zero-allocation fixed-capacity circular ring buffer, rolling telemetry statistics,
//! and background power throttling state machine.

use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Res, ResMut};

/// A fixed-capacity circular ring buffer with zero runtime allocations.
#[derive(Clone, Debug)]
pub struct FixedRingBuffer<T, const CAP: usize> {
    storage: [Option<T>; CAP],
    head: usize,
    len: usize,
}

impl<T, const CAP: usize> Default for FixedRingBuffer<T, CAP> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const CAP: usize> FixedRingBuffer<T, CAP> {
    /// Create a new, empty fixed-capacity ring buffer.
    pub const fn new() -> Self {
        Self {
            storage: [const { None }; CAP],
            head: 0,
            len: 0,
        }
    }

    /// Push an element to the newest position. Evicts the oldest when at capacity.
    pub fn push(&mut self, item: T) {
        if CAP == 0 {
            return;
        }
        self.storage[self.head] = Some(item);
        self.head = (self.head + 1) % CAP;
        if self.len < CAP {
            self.len += 1;
        }
    }

    /// Number of elements currently stored.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the buffer is full to its maximum capacity.
    pub const fn is_full(&self) -> bool {
        self.len == CAP
    }

    /// The maximum capacity of this buffer.
    pub const fn capacity(&self) -> usize {
        CAP
    }

    /// Clear all stored elements.
    pub fn clear(&mut self) {
        for slot in &mut self.storage {
            *slot = None;
        }
        self.head = 0;
        self.len = 0;
    }

    /// Retrieve an item by chronological index (0 is oldest, `len - 1` is newest).
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        let oldest_idx = if self.len < CAP { 0 } else { self.head };
        let actual_idx = (oldest_idx + index) % CAP;
        self.storage[actual_idx].as_ref()
    }

    /// Return reference to the oldest element.
    pub fn oldest(&self) -> Option<&T> {
        self.get(0)
    }

    /// Return reference to the newest element.
    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Iterate through items in chronological order (oldest → newest).
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let len = self.len;
        let oldest_idx = if self.len < CAP { 0 } else { self.head };
        (0..len).map(move |i| {
            let actual = (oldest_idx + i) % CAP;
            self.storage[actual].as_ref().unwrap()
        })
    }

    /// Convert stored elements to a standard Vec in chronological order.
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.iter().cloned().collect()
    }
}

/// Rolling statistical summary of a telemetry series.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TelemetryStatistics {
    pub count: usize,
    pub sum: f64,
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    pub p95: f32,
    pub p99: f32,
    pub ewma: f32,
}

impl TelemetryStatistics {
    /// Compute statistics over finite samples.
    pub fn compute(samples: &[f32], ewma_alpha: f32) -> Self {
        if samples.is_empty() {
            return Self::default();
        }

        let mut finite_samples: Vec<f32> =
            samples.iter().copied().filter(|v| v.is_finite()).collect();
        if finite_samples.is_empty() {
            return Self::default();
        }

        let count = finite_samples.len();
        let mut sum = 0.0f64;
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut ewma = finite_samples[0];
        let alpha = ewma_alpha.clamp(0.01, 0.99);

        for &val in &finite_samples {
            sum += val as f64;
            min = min.min(val);
            max = max.max(val);
            ewma = alpha * val + (1.0 - alpha) * ewma;
        }

        let mean = (sum / count as f64) as f32;

        // Sort to compute exact percentiles
        finite_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p95_idx = ((count as f32 * 0.95).round() as usize).min(count - 1);
        let p99_idx = ((count as f32 * 0.99).round() as usize).min(count - 1);

        Self {
            count,
            sum,
            min,
            max,
            mean,
            p95: finite_samples[p95_idx],
            p99: finite_samples[p99_idx],
            ewma,
        }
    }
}

/// Cadence throttling mode for telemetry charts to conserve CPU and GPU power.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CadenceMode {
    /// Active foreground view: full 60 FPS update rate.
    #[default]
    ForegroundActive,
    /// Idle foreground view: throttled to 10 FPS.
    ForegroundIdle,
    /// Background or unfocused window: throttled to 1 FPS.
    BackgroundThrottled,
    /// Minimized or completely hidden: suspended (0 FPS).
    Suspended,
}

impl CadenceMode {
    /// Minimum seconds between frame repaints for this mode.
    pub fn min_interval_secs(self) -> f32 {
        match self {
            CadenceMode::ForegroundActive => 1.0 / 60.0,
            CadenceMode::ForegroundIdle => 1.0 / 10.0,
            CadenceMode::BackgroundThrottled => 1.0,
            CadenceMode::Suspended => f32::INFINITY,
        }
    }
}

/// Manager resource controlling rendering cadence and power saving.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct TelemetryCadenceManager {
    pub mode: CadenceMode,
    pub elapsed_since_last_tick: f32,
    pub is_window_focused: bool,
    pub is_window_visible: bool,
    pub frame_counter: u64,
}

impl Default for TelemetryCadenceManager {
    fn default() -> Self {
        Self {
            mode: CadenceMode::ForegroundActive,
            elapsed_since_last_tick: 0.0,
            is_window_focused: true,
            is_window_visible: true,
            frame_counter: 0,
        }
    }
}

impl TelemetryCadenceManager {
    /// Step the cadence timer with frame delta time `dt`. Returns `true` if a chart repaint should occur.
    pub fn on_frame(&mut self, dt: f32) -> bool {
        self.frame_counter = self.frame_counter.wrapping_add(1);
        self.elapsed_since_last_tick += dt;

        // Auto-update mode based on focus and visibility
        if !self.is_window_visible {
            self.mode = CadenceMode::Suspended;
        } else if !self.is_window_focused {
            self.mode = CadenceMode::BackgroundThrottled;
        } else {
            self.mode = CadenceMode::ForegroundActive;
        }

        let interval = self.mode.min_interval_secs();
        if self.elapsed_since_last_tick >= interval {
            self.elapsed_since_last_tick = 0.0;
            true
        } else {
            false
        }
    }

    /// Force an immediate wake-up / repaint.
    pub fn wake(&mut self) {
        self.elapsed_since_last_tick = f32::INFINITY;
    }
}

/// Bevy ECS system stepping the telemetry cadence manager every frame.
pub fn update_telemetry_cadence(
    time: Res<bevy::time::Time>,
    manager: Option<ResMut<TelemetryCadenceManager>>,
) {
    if let Some(mut manager) = manager {
        let dt = time.delta_secs();
        manager.on_frame(dt);
    }
}

/// Fixed-size sliding window aggregator computing rolling min, max, and moving average with O(1) space.
#[derive(Clone, Debug)]
pub struct RollingWindowAggregator<const N: usize> {
    buffer: [f32; N],
    index: usize,
    count: usize,
    sum: f64,
}

impl<const N: usize> Default for RollingWindowAggregator<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> RollingWindowAggregator<N> {
    pub const fn new() -> Self {
        Self {
            buffer: [0.0; N],
            index: 0,
            count: 0,
            sum: 0.0,
        }
    }

    /// Push a new telemetry value, evicting the oldest from the rolling sum.
    pub fn push(&mut self, val: f32) {
        if N == 0 {
            return;
        }
        if self.count >= N {
            self.sum -= self.buffer[self.index] as f64;
        } else {
            self.count += 1;
        }

        self.buffer[self.index] = val;
        self.sum += val as f64;
        self.index = (self.index + 1) % N;
    }

    /// Number of samples currently held in the window.
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Whether the window is empty.
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Simple moving average (SMA) across the window.
    pub fn moving_average(&self) -> f32 {
        if self.count == 0 {
            0.0
        } else {
            (self.sum / self.count as f64) as f32
        }
    }

    /// Minimum value across the active window.
    pub fn min(&self) -> f32 {
        self.buffer[..self.count]
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min)
    }

    /// Maximum value across the active window.
    pub fn max(&self) -> f32 {
        self.buffer[..self.count]
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_window_aggregator_statistics() {
        let mut agg = RollingWindowAggregator::<4>::new();
        assert!(agg.is_empty());

        agg.push(10.0);
        agg.push(20.0);
        assert_eq!(agg.len(), 2);
        assert_eq!(agg.moving_average(), 15.0);

        agg.push(30.0);
        agg.push(40.0);
        assert_eq!(agg.len(), 4);
        assert_eq!(agg.moving_average(), 25.0);
        assert_eq!(agg.min(), 10.0);
        assert_eq!(agg.max(), 40.0);

        // 5th push evicts 10.0 -> window has [20, 30, 40, 50]
        agg.push(50.0);
        assert_eq!(agg.len(), 4);
        assert_eq!(agg.moving_average(), 35.0);
        assert_eq!(agg.min(), 20.0);
        assert_eq!(agg.max(), 50.0);
    }
}
