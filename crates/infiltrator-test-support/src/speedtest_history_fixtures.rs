//! Isolated speedtest-history port fixture.
//!
//! The store keeps the bounded run history in memory and publishes it through
//! the typed [`SpeedtestHistoryStore`] port. The seeded history holds two
//! comparable runs of the same target, so a comparison capture can render a
//! real difference instead of a single row.

use infiltrator_contract::speedtest::{
    HistoricalSpeedtestRecord, SpeedtestConcurrency, SpeedtestProgress, SpeedtestRounds,
    SpeedtestRunId, SpeedtestRunState, SpeedtestSnapshot, SpeedtestTarget,
};
use infiltrator_ports::speedtest_history::SpeedtestHistoryStore;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_contract::outbound::probe::OutboundProbeBatchScope;
use lurker_core_ports::error::PortError;
use std::sync::Mutex;

/// The engine the captured runs belong to.
pub const CAPTURE_ENGINE: &str = "mihomo";
/// The core instance the captured runs belong to.
pub const CAPTURE_INSTANCE: &str = "capture-core";
/// The first captured run.
pub const CAPTURE_FIRST_RUN: u64 = 1;
/// The second captured run.
pub const CAPTURE_SECOND_RUN: u64 = 2;
/// The timestamp of the first captured run.
pub const CAPTURE_FIRST_TIMESTAMP_MS: u64 = 1_760_000_000_000;
/// The timestamp of the second captured run.
pub const CAPTURE_SECOND_TIMESTAMP_MS: u64 = 1_760_003_600_000;

/// The core scope every captured run belongs to.
pub fn capture_core_scope() -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new(CAPTURE_ENGINE).expect("valid capture engine ID"),
            CoreInstanceId::new(CAPTURE_INSTANCE).expect("valid capture instance ID"),
            Some(CoreVersion::new("1.19.30").expect("valid capture core version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

/// The target every captured run probes.
pub fn capture_speedtest_target() -> SpeedtestTarget {
    SpeedtestTarget::new(
        capture_core_scope(),
        SnapshotRevision::new(1),
        OutboundProbeBatchScope::All,
        SpeedtestRounds::DEFAULT,
    )
    .expect("valid capture speedtest target")
}

/// The two comparable runs the fixture publishes, oldest first.
pub fn capture_speedtest_history() -> Vec<HistoricalSpeedtestRecord> {
    let target = capture_speedtest_target();
    vec![
        HistoricalSpeedtestRecord {
            run_id: SpeedtestRunId::new(CAPTURE_FIRST_RUN).expect("valid capture run ID"),
            timestamp_epoch_ms: CAPTURE_FIRST_TIMESTAMP_MS,
            target: Some(target.clone()),
            total_targets: 2,
            completed_targets: 2,
            avg_latency_ms: Some(184.0),
            avg_jitter_ms: Some(12.5),
        },
        HistoricalSpeedtestRecord {
            run_id: SpeedtestRunId::new(CAPTURE_SECOND_RUN).expect("valid capture run ID"),
            timestamp_epoch_ms: CAPTURE_SECOND_TIMESTAMP_MS,
            target: Some(target),
            total_targets: 2,
            completed_targets: 2,
            avg_latency_ms: Some(121.0),
            avg_jitter_ms: Some(7.25),
        },
    ]
}

/// A snapshot of a speedtest run that is still probing.
///
/// A cancel capture needs a run that is genuinely in flight: the phase is
/// `Probing`, the progress is partial and the run identity is the one a
/// cancel intent must name.
pub fn running_speedtest_snapshot() -> SpeedtestSnapshot {
    SpeedtestSnapshot {
        revision: 1,
        state: SpeedtestRunState::Probing {
            run_id: SpeedtestRunId::new(CAPTURE_SECOND_RUN).expect("valid capture run ID"),
            target: capture_speedtest_target(),
            progress: SpeedtestProgress {
                completed_targets: 1,
                total_targets: 2,
            },
        },
        concurrency: SpeedtestConcurrency::default(),
        results: Vec::new(),
        recent_history: capture_speedtest_history(),
    }
}

/// An isolated speedtest-history store that publishes a real run history.
pub struct SpeedtestHistoryCapture {
    records: Mutex<Vec<HistoricalSpeedtestRecord>>,
}

impl Default for SpeedtestHistoryCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeedtestHistoryCapture {
    /// A store seeded with the two comparable capture runs.
    pub fn new() -> Self {
        Self {
            records: Mutex::new(capture_speedtest_history()),
        }
    }

    /// A store with no run at all.
    pub fn empty() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
        }
    }

    /// The records the store currently publishes, oldest first.
    pub fn records(&self) -> Vec<HistoricalSpeedtestRecord> {
        self.records
            .lock()
            .expect("speedtest capture history")
            .clone()
    }
}

impl SpeedtestHistoryStore for SpeedtestHistoryCapture {
    fn load(&self) -> Result<Vec<HistoricalSpeedtestRecord>, PortError> {
        Ok(self.records())
    }

    fn save(&self, records: &[HistoricalSpeedtestRecord]) -> Result<(), PortError> {
        *self.records.lock().expect("speedtest capture history") = records.to_vec();
        Ok(())
    }
}
