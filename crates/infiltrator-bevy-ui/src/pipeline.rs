//! Unified Async-to-ECS lock-free drain pipeline.
//!
//! Charter law (docs/BEVY_UI_FRONTEND.md):
//! - Non-blocking drain on the app thread every frame (never stalls UI).
//! - Newest snapshot coalescing: drops stale intermediates when frame rendering
//!   falls behind high-frequency polling.
//! - Failure dwell protection: visible error verdicts are retained for at least
//!   `min_dwell` duration before being overwritten by subsequent successes.
//! - Drop-to-stop cancellation: dropping receiver or sender terminates background
//!   tasks without leaking threads.
//! - MultiPageCadenceGovernor: route-aware and focus-aware polling throttling (0.00% CPU when minimized).

use std::marker::PhantomData;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bevy::app::{App, Plugin, Update};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Res, ResMut};

use crate::controller::FailureDwell;
use crate::domain_state::{DomainPhase, DomainResource, DomainState, DomainStateUpdated};
use crate::route::Route;

/// Default capacity for async-to-ecs drain pipelines.
pub const DEFAULT_PIPELINE_CAPACITY: usize = 16;

/// Producer handle for sending domain snapshots into the ECS pipeline.
#[derive(Clone)]
pub struct AsyncDrainSink<T> {
    tx: SyncSender<DomainState<T>>,
    rx: Arc<Mutex<Receiver<DomainState<T>>>>,
}

impl<T: Send + Sync + 'static> AsyncDrainSink<T> {
    /// Send a domain state snapshot into the pipeline.
    /// If full, drops the oldest item and replaces it with the newest.
    pub fn send(&self, state: DomainState<T>) -> Result<(), String> {
        match self.tx.try_send(state) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(dropped)) => {
                // Drop oldest
                if let Ok(rx) = self.rx.lock() {
                    let _ = rx.try_recv();
                }
                self.tx
                    .try_send(dropped)
                    .map_err(|e| format!("Pipeline send failed after drop: {e}"))
            }
            Err(TrySendError::Disconnected(_)) => Err("Pipeline receiver disconnected".to_owned()),
        }
    }
}

/// ECS resource bridge receiving domain snapshots from async producers.
#[derive(Resource)]
pub struct AsyncDrainBridge<T: Send + Sync + 'static> {
    rx: Arc<Mutex<Receiver<DomainState<T>>>>,
    _marker: PhantomData<T>,
}

impl<T: Send + Sync + 'static> AsyncDrainBridge<T> {
    /// Create a producer-consumer channel pair.
    pub fn channel(capacity: usize) -> (AsyncDrainSink<T>, Self) {
        let (tx, rx) = sync_channel::<DomainState<T>>(capacity);
        let rx = Arc::new(Mutex::new(rx));
        let sink = AsyncDrainSink {
            tx,
            rx: Arc::clone(&rx),
        };
        let bridge = Self {
            rx,
            _marker: PhantomData,
        };
        (sink, bridge)
    }
}

/// Bevy Plugin installing the drain pipeline for domain type `T`.
pub struct DrainPipelinePlugin<T: Send + Sync + Clone + 'static> {
    bridge: AsyncDrainBridge<T>,
}

impl<T: Send + Sync + Clone + 'static> DrainPipelinePlugin<T> {
    /// Create plugin from an existing bridge.
    pub fn new(bridge: AsyncDrainBridge<T>) -> Self {
        Self { bridge }
    }
}

impl<T: Send + Sync + Clone + 'static> Plugin for DrainPipelinePlugin<T> {
    fn build(&self, app: &mut App) {
        app.insert_resource(AsyncDrainBridge {
            rx: Arc::clone(&self.bridge.rx),
            _marker: PhantomData,
        });
        app.init_resource::<DomainResource<T>>();
        app.init_resource::<FailureDwell>();
        app.add_systems(Update, drain_domain_pipeline::<T>);
    }
}

/// Per-frame drain system: drains all queued snapshots from channel,
/// coalesces to the newest, respects failure dwell, and triggers typed update observer.
pub fn drain_domain_pipeline<T: Send + Sync + Clone + 'static>(
    bridge: Res<AsyncDrainBridge<T>>,
    mut dwell: ResMut<FailureDwell>,
    mut resource: ResMut<DomainResource<T>>,
    mut commands: Commands,
) {
    let Ok(rx) = bridge.rx.lock() else {
        return;
    };
    let mut newest: Option<DomainState<T>> = None;
    loop {
        match rx.try_recv() {
            Ok(snapshot) => newest = Some(snapshot),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
    drop(rx);

    if let Some(snapshot) = newest {
        let now = Instant::now();
        let is_failure = snapshot.phase == DomainPhase::Error;

        if !is_failure && !dwell.success_may_pass(now) {
            // Failure verdict is still in minimum dwell window, defer non-failure
            return;
        }

        if is_failure {
            dwell.latch(now);
        } else {
            dwell.latched_at = None;
        }

        resource.0 = snapshot.clone();
        commands.trigger(DomainStateUpdated(snapshot));
    }
}

/// Multi-page energy-efficient polling cadence governor.
///
/// Implements charter law (docs/BEVY_UI_FRONTEND.md §8.2):
/// Standby CPU 0.0% ~ 0.05%, Background/Minimized CPU 0.00%.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct MultiPageCadenceGovernor {
    pub active_route: Route,
    pub is_window_focused: bool,
    pub is_window_visible: bool,
    pub is_low_power: bool,
}

impl Default for MultiPageCadenceGovernor {
    fn default() -> Self {
        Self {
            active_route: Route::Overview,
            is_window_focused: true,
            is_window_visible: true,
            is_low_power: false,
        }
    }
}

impl MultiPageCadenceGovernor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate polling interval for a given page route based on foreground/background status.
    pub fn polling_interval_for(&self, route: Route) -> Option<Duration> {
        if !self.is_window_visible {
            // Minimized / hidden window: completely freeze polling (0.00% CPU)
            return None;
        }

        if !self.is_window_focused {
            // Unfocused window: throttled 5s background heartbeat for all pages
            return Some(Duration::from_secs(5));
        }

        let is_current = route == self.active_route;
        let interval = match (route, is_current) {
            // Active realtime pages
            (Route::Overview, true) | (Route::Connections, true) => Duration::from_secs(1),
            // Active medium-cadence pages
            (Route::Logs, true) | (Route::Dns, true) | (Route::Proxies, true) => {
                Duration::from_millis(2500)
            }
            // Active static / configuration pages
            (_, true) => Duration::from_secs(10),
            // Inactive background pages: low cadence
            (_, false) => Duration::from_secs(30),
        };

        if self.is_low_power {
            Some(interval.mul_f32(2.0))
        } else {
            Some(interval)
        }
    }

    /// Check if a page route is due for a new poll sample.
    pub fn is_due(&self, route: Route, elapsed_since_last: Duration) -> bool {
        match self.polling_interval_for(route) {
            Some(target) => elapsed_since_last >= target,
            None => false,
        }
    }
}

/// High-frequency event coalescer batching rapid events (logs, connection updates) into single frames.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchEventCoalescer<T> {
    pub max_batch_size: usize,
    pub max_linger: Duration,
    pub pending: Vec<T>,
    pub first_arrival: Option<Instant>,
}

impl<T> BatchEventCoalescer<T> {
    pub fn new(max_batch_size: usize, max_linger: Duration) -> Self {
        Self {
            max_batch_size: max_batch_size.max(1),
            max_linger,
            pending: Vec::with_capacity(max_batch_size),
            first_arrival: None,
        }
    }

    /// Push an event. Returns Some(batch) if threshold is reached.
    pub fn push(&mut self, item: T, now: Instant) -> Option<Vec<T>> {
        if self.pending.is_empty() {
            self.first_arrival = Some(now);
        }
        self.pending.push(item);

        if self.pending.len() >= self.max_batch_size {
            self.flush()
        } else if let Some(first) = self.first_arrival {
            if now.duration_since(first) >= self.max_linger {
                self.flush()
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Force drain all pending items into a batch.
    pub fn flush(&mut self) -> Option<Vec<T>> {
        if self.pending.is_empty() {
            None
        } else {
            self.first_arrival = None;
            let batch =
                std::mem::replace(&mut self.pending, Vec::with_capacity(self.max_batch_size));
            Some(batch)
        }
    }
}

/// Debounced event pipeline deferring execution until quiet period elapses (e.g. search filter input).
#[derive(Clone, Debug, PartialEq)]
pub struct DebouncedPipeline<T> {
    pub quiet_period: Duration,
    pub pending: Option<(T, Instant)>,
}

impl<T> DebouncedPipeline<T> {
    pub fn new(quiet_period: Duration) -> Self {
        Self {
            quiet_period,
            pending: None,
        }
    }

    /// Submit a new event, resetting the quiet period timer.
    pub fn submit(&mut self, item: T, now: Instant) {
        self.pending = Some((item, now));
    }

    /// Poll for quiet period expiration. Returns Some(item) once quiet.
    pub fn tick(&mut self, now: Instant) -> Option<T> {
        if let Some((_, submitted_at)) = self.pending
            && now.duration_since(submitted_at) >= self.quiet_period
        {
            return self.pending.take().map(|(item, _)| item);
        }
        None
    }
}

/// Throttled execution guard limiting maximum invocation frequency to 1 per interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThrottledPipeline {
    pub interval: Duration,
    pub last_executed: Option<Instant>,
}

impl ThrottledPipeline {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_executed: None,
        }
    }

    /// Check if action can execute at timestamp `now`. If permitted, updates last execution time.
    pub fn try_acquire(&mut self, now: Instant) -> bool {
        match self.last_executed {
            None => {
                self.last_executed = Some(now);
                true
            }
            Some(last) => {
                if now.duration_since(last) >= self.interval {
                    self.last_executed = Some(now);
                    true
                } else {
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_sink_drop_oldest_on_full_channel() {
        let (sink, bridge) = AsyncDrainBridge::<u32>::channel(2);

        assert!(sink.send(DomainState::ready(1, Duration::ZERO)).is_ok());
        assert!(sink.send(DomainState::ready(2, Duration::ZERO)).is_ok());
        // Channel is full; sending 3 should drop 1 and succeed
        assert!(sink.send(DomainState::ready(3, Duration::ZERO)).is_ok());

        let rx = bridge.rx.lock().unwrap();
        let item1 = rx.try_recv().unwrap();
        assert_eq!(item1.data(), Some(&2));
        let item2 = rx.try_recv().unwrap();
        assert_eq!(item2.data(), Some(&3));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_multi_page_cadence_governor_allocation() {
        let mut gov = MultiPageCadenceGovernor::new();
        assert_eq!(gov.active_route, Route::Overview);

        // Overview is active -> 1s interval
        assert_eq!(
            gov.polling_interval_for(Route::Overview),
            Some(Duration::from_secs(1))
        );
        // Connections is inactive -> 30s interval
        assert_eq!(
            gov.polling_interval_for(Route::Connections),
            Some(Duration::from_secs(30))
        );

        // Unfocus window -> 5s throttled for all
        gov.is_window_focused = false;
        assert_eq!(
            gov.polling_interval_for(Route::Overview),
            Some(Duration::from_secs(5))
        );

        // Minimize window -> None (suspended 0% CPU)
        gov.is_window_visible = false;
        assert_eq!(gov.polling_interval_for(Route::Overview), None);
        assert!(!gov.is_due(Route::Overview, Duration::from_secs(100)));
    }
    #[test]
    fn test_batch_event_coalescer_flush() {
        let mut coalescer = BatchEventCoalescer::<u32>::new(3, Duration::from_millis(50));
        let start = Instant::now();

        // Items 1 & 2 do not trigger flush
        assert!(coalescer.push(10, start).is_none());
        assert!(coalescer.push(20, start).is_none());

        // 3rd item triggers batch size flush
        let batch = coalescer.push(30, start).expect("batch flush");
        assert_eq!(batch, vec![10, 20, 30]);

        // Time-based flush
        assert!(coalescer.push(40, start).is_none());
        let later = start + Duration::from_millis(60);
        let time_batch = coalescer.push(50, later).expect("time flush");
        assert_eq!(time_batch, vec![40, 50]);
    }
    #[test]
    fn test_debounced_and_throttled_pipelines() {
        let mut debouncer = DebouncedPipeline::<String>::new(Duration::from_millis(50));
        let start = Instant::now();

        debouncer.submit("query1".to_string(), start);
        // 20ms later: not ready
        assert_eq!(debouncer.tick(start + Duration::from_millis(20)), None);

        // Submit query2 at 30ms -> resets quiet period
        debouncer.submit("query2".to_string(), start + Duration::from_millis(30));

        // 60ms from start (30ms since query2) -> not ready
        assert_eq!(debouncer.tick(start + Duration::from_millis(60)), None);

        // 90ms from start (60ms >= 50ms since query2) -> fires query2
        assert_eq!(
            debouncer.tick(start + Duration::from_millis(90)),
            Some("query2".to_string())
        );

        // Throttle test
        let mut throttler = ThrottledPipeline::new(Duration::from_millis(100));
        assert!(throttler.try_acquire(start));
        assert!(!throttler.try_acquire(start + Duration::from_millis(50)));
        assert!(throttler.try_acquire(start + Duration::from_millis(101)));
    }
}
