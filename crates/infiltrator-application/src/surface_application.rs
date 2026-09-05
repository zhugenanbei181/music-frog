//! Application-owned complete surface snapshot pump.
//!
//! The pump is deliberately toolkit-neutral. A host supplies one
//! [`SurfaceReader`] and one [`ApplicationRuntime`]; Iced and Bevy only drain
//! owned `SurfaceSnapshot` values and project them into their native widgets.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::{SurfaceEvent, SurfaceSnapshot};
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SNAPSHOT_CAPACITY: usize = 4;

struct Shared {
    last: Arc<Mutex<SurfaceSnapshot>>,
    stop: Arc<AtomicBool>,
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
        let (snapshot_tx, snapshot_rx) = sync_channel(SNAPSHOT_CAPACITY);
        let snapshot_rx = Arc::new(Mutex::new(snapshot_rx));
        let last = Arc::new(Mutex::new(initial));
        let stop = Arc::new(AtomicBool::new(false));
        let pump = Self {
            shared: Arc::new(Shared {
                last: Arc::clone(&last),
                stop: Arc::clone(&stop),
            }),
            snapshot_rx: Arc::clone(&snapshot_rx),
        };
        std::thread::Builder::new()
            .name("infiltrator-surface".to_owned())
            .spawn(move || {
                pump_loop(
                    reader,
                    sample_interval,
                    runtime,
                    snapshot_tx,
                    snapshot_rx,
                    last,
                    stop,
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
    last: Arc<Mutex<SurfaceSnapshot>>,
    stop: Arc<AtomicBool>,
) {
    // A standard-library timeout keeps the worker executor-neutral. The
    // injected ApplicationRuntime still owns every async port call.
    let (_wake_tx, wake_rx) = std::sync::mpsc::channel::<()>();
    loop {
        if stop.load(Ordering::Acquire) {
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
                let mut snapshot = last.lock().expect("surface mirror lock").clone();
                let failure = Failure::from(error);
                snapshot.revision = snapshot.revision.saturating_add(1);
                snapshot.failure = Some(failure.clone());
                snapshot.core.lifecycle = infiltrator_contract::snapshot::CoreLifecycle::Failed;
                snapshot.core.revision = snapshot.revision;
                snapshot.core.failure = Some(failure);
                snapshot
            }
        };

        let previous_revision = last.lock().expect("surface mirror lock").revision;
        if snapshot.revision <= previous_revision {
            snapshot.revision = previous_revision.saturating_add(1);
        }
        snapshot.core.revision = snapshot.revision;
        snapshot.generation = snapshot.core.generation;
        *last.lock().expect("surface mirror lock") = snapshot.clone();

        match snapshot_tx.try_send(snapshot) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                let receiver = snapshot_rx.lock().expect("surface channel lock");
                if receiver.try_recv().is_err() {
                    return;
                }
            }
            Err(TrySendError::Disconnected(_)) => return,
        }

        if sample_interval.is_zero() {
            return;
        }
        if stop.load(Ordering::Acquire) {
            return;
        }
        match wake_rx.recv_timeout(sample_interval) {
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
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
    async fn read(&self) -> Result<SurfaceSnapshot, infiltrator_ports::error::PortError> {
        Err(infiltrator_ports::error::PortError::Failed(
            self.failure.message.clone(),
        ))
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

    struct TokioRuntime(tokio::runtime::Runtime);

    impl ApplicationRuntime for TokioRuntime {
        fn block_on(&self, future: ApplicationFuture) {
            self.0.block_on(future);
        }

        fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
            Box::pin(tokio::time::sleep(duration))
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
            tokio::runtime::Builder::new_current_thread()
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
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let snapshots = bridge.drain();
            if !snapshots.is_empty() {
                assert_eq!(snapshots.len(), 1);
                assert_eq!(snapshots[0].surface, SurfaceKind::BevyDesktop);
                break;
            }
            assert!(std::time::Instant::now() < deadline, "pump did not deliver");
            std::thread::yield_now();
        }
    }
}
