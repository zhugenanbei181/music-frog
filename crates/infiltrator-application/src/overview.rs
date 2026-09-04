//! Runtime-owned Overview sampling and bounded event transport.
//!
//! This module owns the Tokio worker over an injected OverviewReader port. UI
//! surfaces consume only `CoreSnapshot` values and standard-library callback
//! channels; no Bevy/Iced/FFI type or concrete HTTP client is part of the
//! seam.

use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::{OverviewReader, OverviewSample};
use std::sync::mpsc::{Receiver, Sender, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SNAPSHOT_CAPACITY: usize = 8;

/// Controller connection and sampling settings, independent of any UI.
#[derive(Clone, Debug)]
pub struct OverviewConfig {
    pub endpoint: String,
    pub secret: Option<String>,
    pub sample_interval: Duration,
}

impl OverviewConfig {
    pub fn new(endpoint: impl Into<String>, secret: Option<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            secret,
            sample_interval: Duration::from_millis(700),
        }
    }
}

struct Shared {
    last: Arc<Mutex<CoreSnapshot>>,
    command_tx: Sender<OverviewCommand>,
}

enum OverviewCommand {
    SetMode {
        mode: ProxyMode,
        responder: Sender<Result<ProxyMode, Failure>>,
    },
}

/// The application-owned Overview source. Clones share one worker and one
/// canonical snapshot mirror.
#[derive(Clone)]
pub struct OverviewPump {
    shared: Arc<Shared>,
    snapshot_rx: Arc<Mutex<Receiver<CoreSnapshot>>>,
}

/// Read-only bridge used by frame-driven surfaces to drain snapshots without
/// touching the worker's channel type.
#[derive(Clone)]
pub struct OverviewPumpBridge {
    snapshot_rx: Arc<Mutex<Receiver<CoreSnapshot>>>,
}

impl OverviewPumpBridge {
    pub fn drain(&self) -> Vec<CoreSnapshot> {
        let receiver = self.snapshot_rx.lock().expect("overview channel lock");
        let mut snapshots = Vec::new();
        while let Ok(snapshot) = receiver.try_recv() {
            snapshots.push(snapshot);
        }
        snapshots
    }
}

impl OverviewPump {
    /// Spawn an application worker over an arbitrary OverviewReader port.
    pub fn spawn(
        reader: Arc<dyn OverviewReader>,
        sample_interval: Duration,
        runtime: Arc<dyn ApplicationRuntime>,
    ) -> Self {
        let (snapshot_tx, snapshot_rx) = sync_channel(SNAPSHOT_CAPACITY);
        let snapshot_rx = Arc::new(Mutex::new(snapshot_rx));
        let (command_tx, command_rx) = std::sync::mpsc::channel();
        let last = Arc::new(Mutex::new(initial_snapshot()));
        let pump = Self {
            shared: Arc::new(Shared {
                last: Arc::clone(&last),
                command_tx,
            }),
            snapshot_rx: Arc::clone(&snapshot_rx),
        };
        std::thread::spawn(move || {
            pump_loop(
                reader,
                sample_interval,
                runtime,
                command_rx,
                snapshot_tx,
                snapshot_rx,
                last,
            )
        });
        pump
    }

    pub fn current(&self) -> CoreSnapshot {
        self.shared
            .last
            .lock()
            .expect("overview mirror lock")
            .clone()
    }

    pub fn bridge(&self) -> OverviewPumpBridge {
        OverviewPumpBridge {
            snapshot_rx: Arc::clone(&self.snapshot_rx),
        }
    }

    /// Enqueue a mode command. Completion is delivered through the standard
    /// library channel supplied by the caller.
    pub fn request_mode(&self, mode: ProxyMode, responder: Sender<Result<ProxyMode, Failure>>) {
        let command = OverviewCommand::SetMode { mode, responder };
        if let Err(std::sync::mpsc::SendError(OverviewCommand::SetMode { responder, .. })) =
            self.shared.command_tx.send(command)
        {
            let _ = responder.send(Err(Failure::new(
                ErrorCode::Internal,
                "overview worker is no longer available",
                true,
            )));
        }
    }
}

/// Fallback reader used by a composition root when adapter construction
/// itself fails. Keeping this here means the failure remains a typed port
/// result and the UI does not need to know how the adapter was built.
pub struct UnavailableOverviewReader {
    failure: PortError,
}

impl UnavailableOverviewReader {
    pub fn new(failure: PortError) -> Self {
        Self { failure }
    }
}

#[async_trait::async_trait]
impl OverviewReader for UnavailableOverviewReader {
    async fn sample(&self) -> Result<OverviewSample, PortError> {
        Err(self.failure.clone())
    }

    async fn set_mode(&self, _mode: ProxyMode) -> Result<ProxyMode, PortError> {
        Err(self.failure.clone())
    }
}

fn initial_snapshot() -> CoreSnapshot {
    CoreSnapshot {
        lifecycle: CoreLifecycle::Starting,
        generation: 0,
        revision: 0,
        proxy_mode: Some(ProxyMode::Rule),
        core_version: None,
        sampled_at_epoch_ms: None,
        failure: Some(Failure::new(
            ErrorCode::NotReady,
            "waiting for the first controller sample",
            true,
        )),
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
    }
}

fn pump_loop(
    reader: Arc<dyn OverviewReader>,
    sample_interval: Duration,
    runtime: Arc<dyn ApplicationRuntime>,
    command_rx: Receiver<OverviewCommand>,
    snapshot_tx: SyncSender<CoreSnapshot>,
    snapshot_rx: Arc<Mutex<Receiver<CoreSnapshot>>>,
    last: Arc<Mutex<CoreSnapshot>>,
) {
    let mut previous_totals: Option<(u64, u64, Instant)> = None;
    let mut mode: Option<ProxyMode> = None;
    let mut version: Option<String> = None;
    let mut memory_bytes: Option<u64> = None;
    let mut revision = 0_u64;

    loop {
        match command_rx.recv_timeout(sample_interval) {
            Ok(OverviewCommand::SetMode {
                mode: wanted,
                responder,
            }) => {
                let reader_for_call = Arc::clone(&reader);
                let result = crate::run_on_runtime(runtime.as_ref(), async move {
                    reader_for_call.set_mode(wanted).await
                });
                if let Ok(actual) = result.as_ref() {
                    mode = Some(*actual);
                }
                let _ = responder.send(result.map_err(Failure::from));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }

        let reader_for_call = Arc::clone(&reader);
        let snapshot =
            match crate::run_on_runtime(
                runtime.as_ref(),
                async move { reader_for_call.sample().await },
            ) {
                Ok(sample) => {
                    mode = sample.mode.or(mode);
                    version = sample.core_version.or(version);
                    memory_bytes = sample.memory_bytes.or(memory_bytes);
                    let now = Instant::now();
                    let (upload_bps, download_bps) = rates_from_totals(
                        previous_totals,
                        sample.upload_total,
                        sample.download_total,
                        now,
                    );
                    previous_totals = Some((sample.upload_total, sample.download_total, now));
                    revision = revision.saturating_add(1);
                    CoreSnapshot {
                        lifecycle: sample.lifecycle,
                        generation: 1,
                        revision,
                        proxy_mode: mode,
                        core_version: version.clone(),
                        sampled_at_epoch_ms: sample.sampled_at_epoch_ms,
                        failure: None,
                        upload_bps,
                        download_bps,
                        active_connections: sample.active_connections,
                        memory_bytes,
                    }
                }
                Err(error) => {
                    previous_totals = None;
                    revision = revision.saturating_add(1);
                    CoreSnapshot {
                        lifecycle: CoreLifecycle::Failed,
                        generation: 1,
                        revision,
                        proxy_mode: mode,
                        core_version: version.clone(),
                        sampled_at_epoch_ms: None,
                        failure: Some(Failure::from(error)),
                        upload_bps: 0.0,
                        download_bps: 0.0,
                        active_connections: 0,
                        memory_bytes,
                    }
                }
            };

        *last.lock().expect("overview mirror lock") = snapshot.clone();
        match snapshot_tx.try_send(snapshot) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                let receiver = snapshot_rx.lock().expect("overview channel lock");
                if receiver.try_recv().is_err() {
                    return;
                }
            }
            Err(TrySendError::Disconnected(_)) => return,
        }

        if sample_interval.is_zero() {
            return;
        }
    }
}

/// Pure rate calculation from cumulative controller counters.
pub fn rates_from_totals(
    previous: Option<(u64, u64, Instant)>,
    upload_total: u64,
    download_total: u64,
    now: Instant,
) -> (f64, f64) {
    let Some((previous_upload, previous_download, previous_at)) = previous else {
        return (0.0, 0.0);
    };
    let elapsed = now.saturating_duration_since(previous_at).as_secs_f64();
    if !elapsed.is_finite() || elapsed <= 0.0 {
        return (0.0, 0.0);
    }
    let rate = |previous: u64, current: u64| {
        if current < previous {
            0.0
        } else {
            (current - previous) as f64 / elapsed
        }
    };
    (
        rate(previous_upload, upload_total),
        rate(previous_download, download_total),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_counter_sample_has_no_fabricated_rate() {
        let now = Instant::now();
        assert_eq!(rates_from_totals(None, 100, 200, now), (0.0, 0.0));
    }

    #[test]
    fn counter_reset_restarts_the_rate_window() {
        let now = Instant::now();
        let previous = Some((1_000, 2_000, now - Duration::from_secs(2)));
        assert_eq!(rates_from_totals(previous, 5, 5, now), (0.0, 0.0));
    }

    #[test]
    fn initial_snapshot_is_explicitly_not_ready() {
        let snapshot = initial_snapshot();
        assert_eq!(snapshot.lifecycle, CoreLifecycle::Starting);
        assert_eq!(snapshot.proxy_mode, Some(ProxyMode::Rule));
        assert_eq!(snapshot.failure.as_ref().unwrap().code, ErrorCode::NotReady);
    }
}
