//! Application-owned complete surface snapshot pump.
//!
//! The pump is deliberately toolkit-neutral. A host supplies one
//! [`SurfaceReader`] and one [`ApplicationRuntime`]; Iced and Bevy only drain
//! owned `SurfaceSnapshot` values and project them into their native widgets.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface_snapshot::{SurfaceEvent, SurfaceSnapshot};
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::error::PortError;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, SyncSender, TrySendError, channel, sync_channel,
};
use std::sync::{Arc, Mutex};
use std::thread::Builder;
use std::time::Duration;

const SNAPSHOT_CAPACITY: usize = 4;

struct Shared {
    last: Arc<Mutex<SurfaceSnapshot>>,
    stop: Arc<AtomicBool>,
    wake: SurfaceWake,
}

/// Toolkit-neutral wake signal shared between the pump and a host event loop.
///
/// The pump owns one clone and calls [`SurfaceWake::wake`] every time it
/// publishes a new snapshot, including a terminal failure or lifecycle state.
/// A host owns another clone, installs its event-loop wake callback with
/// [`SurfaceWake::set_waker`] (for example a thin adapter over a winit
/// `EventLoopProxy`), and clears the pending bit with
/// [`SurfaceWake::acknowledge`] *before* draining the bounded snapshot channel.
///
/// The pending state is a single atomic bit: repeated requests while a wake is
/// already pending collapse into one callback invocation and there is never an
/// unbounded backlog. Because the host acknowledges before draining, an enqueue
/// that races with the drain sets the bit again and wakes the loop, so a
/// terminal snapshot is never lost.
#[derive(Clone)]
pub struct SurfaceWake {
    inner: Arc<SurfaceWakeInner>,
}

struct SurfaceWakeInner {
    pending: AtomicBool,
    delivered: AtomicUsize,
    waker: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl SurfaceWake {
    /// Create an unattached wake signal. Register the host callback with
    /// [`SurfaceWake::set_waker`] before relying on event-loop notification.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SurfaceWakeInner {
                pending: AtomicBool::new(false),
                delivered: AtomicUsize::new(0),
                waker: Mutex::new(None),
            }),
        }
    }

    /// Install the host callback invoked when a wake transitions to pending.
    ///
    /// The callback must not block; a host event loop only needs to enqueue its
    /// own wake token here. Re-registering replaces the previous callback.
    pub fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        let waker: Arc<dyn Fn() + Send + Sync> = Arc::new(waker);
        *self.inner.waker.lock().expect("surface wake lock") = Some(waker);
    }

    /// Request a wake. Coalesces while a wake is already pending.
    pub fn wake(&self) {
        if self.inner.pending.swap(true, Ordering::AcqRel) {
            return;
        }
        self.inner.delivered.fetch_add(1, Ordering::AcqRel);
        let waker = self.inner.waker.lock().expect("surface wake lock").clone();
        if let Some(waker) = waker {
            waker();
        }
    }

    /// Clear the pending bit. Call this before draining the snapshot channel so
    /// an enqueue racing with the drain re-arms the wake instead of being lost.
    pub fn acknowledge(&self) {
        self.inner.pending.store(false, Ordering::Release);
    }

    /// Whether a wake is currently pending (not yet acknowledged).
    pub fn is_pending(&self) -> bool {
        self.inner.pending.load(Ordering::Acquire)
    }

    /// Number of wakes actually delivered. Coalesced requests do not increment
    /// this, so it is always bounded by the number of transitions to pending.
    pub fn delivered(&self) -> usize {
        self.inner.delivered.load(Ordering::Acquire)
    }
}

impl Default for SurfaceWake {
    fn default() -> Self {
        Self::new()
    }
}

/// Application-owned source for the complete cross-surface read model.
#[derive(Clone)]
pub struct SurfacePump {
    shared: Arc<Shared>,
    snapshot_rx: Arc<Mutex<Receiver<SurfaceSnapshot>>>,
}

/// Frame/FFI-facing bridge. No executor or channel implementation crosses
/// this type boundary.
#[derive(Clone)]
pub struct SurfacePumpBridge {
    snapshot_rx: Arc<Mutex<Receiver<SurfaceSnapshot>>>,
}

impl SurfacePumpBridge {
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.snapshot_rx) as usize
    }

    pub fn drain(&self) -> Vec<SurfaceSnapshot> {
        let receiver = self.snapshot_rx.lock().expect("surface channel lock");
        let mut snapshots = Vec::new();
        while let Ok(snapshot) = receiver.try_recv() {
            snapshots.push(snapshot);
        }
        snapshots
    }

    pub fn drain_events(&self) -> Vec<SurfaceEvent> {
        self.drain()
            .into_iter()
            .map(SurfaceEvent::SnapshotUpdated)
            .collect()
    }
}

impl SurfacePump {
    /// Spawn a bounded, runtime-neutral surface reader.
    pub fn spawn(
        reader: Arc<dyn SurfaceReader>,
        sample_interval: Duration,
        runtime: Arc<dyn ApplicationRuntime>,
        initial: SurfaceSnapshot,
    ) -> Self {
        Self::spawn_with_wake(
            reader,
            sample_interval,
            runtime,
            initial,
            SurfaceWake::new(),
        )
    }

    /// Spawn like [`SurfacePump::spawn`] while owning a caller-provided
    /// [`SurfaceWake`], so a host can install its event-loop wake callback
    /// before the worker publishes its first snapshot.
    pub fn spawn_with_wake(
        reader: Arc<dyn SurfaceReader>,
        sample_interval: Duration,
        runtime: Arc<dyn ApplicationRuntime>,
        initial: SurfaceSnapshot,
        wake: SurfaceWake,
    ) -> Self {
        let (snapshot_tx, snapshot_rx) = sync_channel(SNAPSHOT_CAPACITY);
        let snapshot_rx = Arc::new(Mutex::new(snapshot_rx));
        let last = Arc::new(Mutex::new(initial));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Shared {
            last: Arc::clone(&last),
            stop: Arc::clone(&stop),
            wake: wake.clone(),
        });
        let pump = Self {
            shared: Arc::clone(&shared),
            snapshot_rx: Arc::clone(&snapshot_rx),
        };
        Builder::new()
            .name("infiltrator-surface".to_owned())
            .spawn(move || {
                pump_loop(
                    reader,
                    sample_interval,
                    runtime,
                    snapshot_tx,
                    snapshot_rx,
                    shared,
                )
            })
            .expect("surface pump thread must spawn");
        pump
    }

    pub fn current(&self) -> SurfaceSnapshot {
        self.shared
            .last
            .lock()
            .expect("surface mirror lock")
            .clone()
    }

    /// Toolkit-neutral wake handle the host holds to wake a sleeping event
    /// loop. The pump signals it whenever it publishes a new snapshot.
    pub fn wake_handle(&self) -> SurfaceWake {
        self.shared.wake.clone()
    }

    pub fn bridge(&self) -> SurfacePumpBridge {
        SurfacePumpBridge {
            snapshot_rx: Arc::clone(&self.snapshot_rx),
        }
    }
}

impl Drop for SurfacePump {
    fn drop(&mut self) {
        // Cloned bridges do not own the worker; the final pump handle does.
        if Arc::strong_count(&self.shared) == 1 {
            self.shared.stop.store(true, Ordering::Release);
        }
    }
}

fn pump_loop(
    reader: Arc<dyn SurfaceReader>,
    sample_interval: Duration,
    runtime: Arc<dyn ApplicationRuntime>,
    snapshot_tx: SyncSender<SurfaceSnapshot>,
    snapshot_rx: Arc<Mutex<Receiver<SurfaceSnapshot>>>,
    shared: Arc<Shared>,
) {
    // The pump's own bounded poll timer. It is deliberately separate from the
    // public `SurfaceWake`: that handle is a one-way pump-to-host signal for a
    // sleeping event loop, while this channel only bounds the worker's own
    // sampling cadence. Conflating them would let a host wake reset the
    // sampling interval instead of merely observing new data.
    let (_cadence_tx, cadence_rx) = channel::<()>();
    loop {
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        let reader_for_call = Arc::clone(&reader);
        let result =
            crate::run_on_runtime(
                runtime.as_ref(),
                async move { reader_for_call.read().await },
            );
        let mut snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let mut snapshot = shared.last.lock().expect("surface mirror lock").clone();
                let failure = Failure::from(error);
                snapshot.revision = snapshot.revision.saturating_add(1);
                snapshot.failure = Some(failure.clone());
                snapshot.core.lifecycle = CoreLifecycle::Failed;
                snapshot.core.revision = snapshot.revision;
                snapshot.core.failure = Some(failure);
                snapshot
            }
        };

        let previous_revision = shared.last.lock().expect("surface mirror lock").revision;
        if snapshot.revision <= previous_revision {
            snapshot.revision = previous_revision.saturating_add(1);
        }
        snapshot.core.revision = snapshot.revision;
        snapshot.generation = snapshot.core.generation;

        if !publish_snapshot(
            &snapshot_tx,
            &snapshot_rx,
            &shared.last,
            snapshot,
            &shared.wake,
        ) {
            return;
        }

        if sample_interval.is_zero() {
            return;
        }
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        match cadence_rx.recv_timeout(sample_interval) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

/// Mirror one normalized snapshot as the latest value, then publish it into the
/// bounded queue and signal the host wake.
///
/// On saturation the oldest queued snapshot is displaced so the drained window
/// always ends at the newest revision; `last` is mirrored first regardless.
/// Returns `false` when the receiver has gone away and the worker must stop.
fn publish_snapshot(
    snapshot_tx: &SyncSender<SurfaceSnapshot>,
    snapshot_rx: &Mutex<Receiver<SurfaceSnapshot>>,
    last: &Mutex<SurfaceSnapshot>,
    snapshot: SurfaceSnapshot,
    wake: &SurfaceWake,
) -> bool {
    *last.lock().expect("surface mirror lock") = snapshot.clone();
    let sent = match snapshot_tx.try_send(snapshot) {
        Ok(()) => true,
        Err(TrySendError::Full(snapshot)) => {
            let receiver = snapshot_rx.lock().expect("surface channel lock");
            if receiver.try_recv().is_err() {
                return false;
            }
            drop(receiver);
            matches!(snapshot_tx.try_send(snapshot), Ok(()))
        }
        Err(TrySendError::Disconnected(_)) => return false,
    };
    if sent {
        wake.wake();
    }
    sent
}

/// Explicit fallback for a host that has not yet composed all page readers.
/// It produces typed unavailable state, never fabricated demo data.
pub struct UnavailableSurfaceReader {
    failure: Failure,
}

impl UnavailableSurfaceReader {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            failure: Failure::new(ErrorCode::NotReady, message, true),
        }
    }
}

#[async_trait::async_trait]
impl SurfaceReader for UnavailableSurfaceReader {
    async fn read(&self) -> Result<SurfaceSnapshot, PortError> {
        Err(PortError::Failed(self.failure.message.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use infiltrator_ports::application_runtime::{
        ApplicationFuture, ApplicationRuntime, ApplicationSleep,
    };
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::surface::SurfaceReader;
    #[cfg(test)]
    use std::iter::from_fn;
    #[cfg(test)]
    use std::thread::spawn;
    #[cfg(test)]
    use std::thread::yield_now;
    #[cfg(test)]
    use std::time::Instant;
    #[cfg(test)]
    use tokio::runtime;
    #[cfg(test)]
    use tokio::runtime::Runtime;
    #[cfg(test)]
    use tokio::time::sleep;

    struct TokioRuntime(Runtime);

    impl ApplicationRuntime for TokioRuntime {
        fn block_on(&self, future: ApplicationFuture) {
            self.0.block_on(future);
        }

        fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
            Box::pin(sleep(duration))
        }
    }

    struct StaticReader;

    #[async_trait]
    impl SurfaceReader for StaticReader {
        async fn read(&self) -> Result<SurfaceSnapshot, PortError> {
            Ok(SurfaceSnapshot::unavailable(
                SurfaceKind::BevyDesktop,
                HostKind::Desktop,
                Failure::new(ErrorCode::NotReady, "test snapshot", true),
            ))
        }
    }

    #[test]
    fn zero_interval_pump_delivers_one_bounded_snapshot() {
        let runtime = Arc::new(TokioRuntime(
            runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime"),
        ));
        let initial = SurfaceSnapshot::unavailable(
            SurfaceKind::BevyDesktop,
            HostKind::Desktop,
            Failure::new(ErrorCode::NotReady, "initial", true),
        );
        let pump = SurfacePump::spawn(Arc::new(StaticReader), Duration::ZERO, runtime, initial);
        let bridge = pump.bridge();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let snapshots = bridge.drain();
            if !snapshots.is_empty() {
                assert_eq!(snapshots.len(), 1);
                assert_eq!(snapshots[0].surface, SurfaceKind::BevyDesktop);
                break;
            }
            assert!(Instant::now() < deadline, "pump did not deliver");
            yield_now();
        }
    }

    fn test_runtime() -> Arc<TokioRuntime> {
        Arc::new(TokioRuntime(
            runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime"),
        ))
    }

    fn surface_snapshot(generation: u64, lifecycle: CoreLifecycle) -> SurfaceSnapshot {
        let mut snapshot = SurfaceSnapshot::unavailable(
            SurfaceKind::BevyDesktop,
            HostKind::Desktop,
            Failure::new(ErrorCode::NotReady, "fixture", true),
        );
        snapshot.core.lifecycle = lifecycle;
        snapshot.core.generation = generation;
        snapshot.generation = generation;
        snapshot
    }

    fn drain(receiver: &Mutex<Receiver<SurfaceSnapshot>>) -> Vec<SurfaceSnapshot> {
        let receiver = receiver.lock().expect("surface channel lock");
        from_fn(|| receiver.try_recv().ok()).collect()
    }

    #[test]
    fn enqueue_notifies_host_wake() {
        let wake = SurfaceWake::new();
        let notified = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&notified);
        wake.set_waker(move || {
            counter.fetch_add(1, Ordering::AcqRel);
        });

        let pump = SurfacePump::spawn_with_wake(
            Arc::new(StaticReader),
            Duration::ZERO,
            test_runtime(),
            surface_snapshot(0, CoreLifecycle::Starting),
            wake.clone(),
        );

        let deadline = Instant::now() + Duration::from_secs(2);
        while notified.load(Ordering::Acquire) == 0 {
            assert!(Instant::now() < deadline, "enqueue did not wake the host");
            yield_now();
        }
        assert!(wake.delivered() >= 1);
        assert!(wake.is_pending());
        assert_eq!(pump.wake_handle().delivered(), wake.delivered());
    }

    #[test]
    fn repeated_wake_requests_coalesce() {
        let wake = SurfaceWake::new();
        let notified = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&notified);
        wake.set_waker(move || {
            counter.fetch_add(1, Ordering::AcqRel);
        });

        for _ in 0..8 {
            wake.wake();
        }
        assert_eq!(notified.load(Ordering::Acquire), 1);
        assert_eq!(wake.delivered(), 1);
        assert!(wake.is_pending());

        wake.acknowledge();
        assert!(!wake.is_pending());
        wake.wake();
        assert_eq!(notified.load(Ordering::Acquire), 2);
        assert_eq!(wake.delivered(), 2);
    }

    #[test]
    fn bounded_channel_saturation_drops_oldest_and_mirrors_last() {
        let (tx, rx) = sync_channel(SNAPSHOT_CAPACITY);
        let rx = Mutex::new(rx);
        let last = Mutex::new(surface_snapshot(0, CoreLifecycle::Starting));
        let wake = SurfaceWake::new();
        let notified = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&notified);
        wake.set_waker(move || {
            counter.fetch_add(1, Ordering::AcqRel);
        });

        let total = SNAPSHOT_CAPACITY as u64 + 3;
        for generation in 1..=total {
            assert!(publish_snapshot(
                &tx,
                &rx,
                &last,
                surface_snapshot(generation, CoreLifecycle::Running),
                &wake,
            ));
        }

        // Every publish coalesced into the single still-pending wake.
        assert_eq!(notified.load(Ordering::Acquire), 1);
        assert_eq!(wake.delivered(), 1);

        let drained = drain(&rx);
        assert_eq!(drained.len(), SNAPSHOT_CAPACITY);
        let generations: Vec<u64> = drained
            .iter()
            .map(|snapshot| snapshot.core.generation)
            .collect();
        assert_eq!(
            generations,
            (total - SNAPSHOT_CAPACITY as u64 + 1..=total).collect::<Vec<_>>(),
            "saturation must displace the oldest snapshots"
        );
        assert_eq!(
            last.lock().expect("surface mirror lock").core.generation,
            total,
            "`last` must mirror the newest published snapshot"
        );
    }

    #[test]
    fn terminal_snapshot_wake_is_not_lost() {
        let (tx, rx) = sync_channel(SNAPSHOT_CAPACITY);
        let rx = Mutex::new(rx);
        let last = Mutex::new(surface_snapshot(0, CoreLifecycle::Starting));
        let wake = SurfaceWake::new();

        for generation in 1..=SNAPSHOT_CAPACITY as u64 {
            assert!(publish_snapshot(
                &tx,
                &rx,
                &last,
                surface_snapshot(generation, CoreLifecycle::Running),
                &wake,
            ));
        }
        wake.acknowledge();
        assert!(!wake.is_pending());
        assert_eq!(wake.delivered(), 1);

        // The terminal snapshot must still be published while saturated, and
        // must re-arm the wake after the earlier acknowledgement.
        let terminal = surface_snapshot(99, CoreLifecycle::Failed);
        assert!(publish_snapshot(&tx, &rx, &last, terminal, &wake));

        assert!(wake.is_pending());
        assert_eq!(wake.delivered(), 2);

        let drained = drain(&rx);
        assert_eq!(drained.len(), SNAPSHOT_CAPACITY);
        let last_drained = drained.last().expect("terminal snapshot must be queued");
        assert_eq!(last_drained.core.lifecycle, CoreLifecycle::Failed);
        assert_eq!(last_drained.core.generation, 99);
        assert_eq!(
            last.lock().expect("surface mirror lock").core.lifecycle,
            CoreLifecycle::Failed
        );
    }

    #[test]
    fn wake_survives_acknowledge_drain_race() {
        let (tx, rx) = sync_channel(SNAPSHOT_CAPACITY);
        let rx = Arc::new(Mutex::new(rx));
        let last = Arc::new(Mutex::new(surface_snapshot(0, CoreLifecycle::Starting)));
        let wake = SurfaceWake::new();
        let notified = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&notified);
        wake.set_waker(move || {
            counter.fetch_add(1, Ordering::AcqRel);
        });

        let terminal = surface_snapshot(1, CoreLifecycle::Failed);
        let producer = {
            let tx = tx.clone();
            let rx = Arc::clone(&rx);
            let last = Arc::clone(&last);
            let wake = wake.clone();
            spawn(move || {
                assert!(publish_snapshot(&tx, &rx, &last, terminal, &wake));
            })
        };

        // Host protocol: acknowledge, then drain. A terminal enqueued during
        // the drain must re-arm the wake instead of being swallowed.
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut observed = false;
        while !observed {
            wake.acknowledge();
            observed = drain(&rx)
                .iter()
                .any(|snapshot| snapshot.core.lifecycle == CoreLifecycle::Failed);
            assert!(
                observed || Instant::now() < deadline,
                "terminal wake was lost"
            );
            if !observed {
                yield_now();
            }
        }

        producer.join().expect("producer thread must finish");
        assert!(notified.load(Ordering::Acquire) >= 1);
    }
}
