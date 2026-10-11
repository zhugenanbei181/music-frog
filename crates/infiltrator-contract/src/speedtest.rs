//! Shared contracts and models for latency testing, jitter variance analysis,
//! packet-loss classification, and identity-fenced result histories.

use crate::core_identity::CoreScope;
use crate::error::Failure;
use crate::outbound::identity::{OutboundContractError, OutboundId, SnapshotRevision};
use crate::outbound::inventory::OutboundProbeState;
use crate::outbound::probe::OutboundProbeBatchScope;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::num::{NonZeroU8, NonZeroU64};

/// Packet loss gradient rating reflecting connection quality tiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketLossRating {
    /// 0% packet loss - optimal lossless transmission.
    Excellent,
    /// (0%, 5%] - low loss within normal operating tolerance.
    Good,
    /// (5%, 20%] - moderate loss, potential stutter and retransmissions.
    Fair,
    /// > 20% - poor connection, heavy degradation.
    Poor,
    /// 100% loss - node unreachable, connection refused, or timed out.
    Dead,
}

impl PacketLossRating {
    /// Classify packet loss percent into strict gradient tiers.
    pub fn from_loss_percent(percent: f64) -> Self {
        if !percent.is_finite() || percent >= 100.0 {
            Self::Dead
        } else if percent <= 0.0001 {
            Self::Excellent
        } else if percent <= 5.0 {
            Self::Good
        } else if percent <= 20.0 {
            Self::Fair
        } else {
            Self::Poor
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Excellent => "极佳 (0%)",
            Self::Good => "良好 (<5%)",
            Self::Fair => "一般 (5-20%)",
            Self::Poor => "较差 (>20%)",
            Self::Dead => "超时 (100%)",
        }
    }

    pub const fn label_en(self) -> &'static str {
        match self {
            Self::Excellent => "Excellent (0%)",
            Self::Good => "Good (<5%)",
            Self::Fair => "Fair (5-20%)",
            Self::Poor => "Poor (>20%)",
            Self::Dead => "Timeout (100%)",
        }
    }

    pub const fn is_healthy(self) -> bool {
        matches!(self, Self::Excellent | Self::Good)
    }
}

/// Precise RTT statistics, sample variance, and jitter calculations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JitterCalculation {
    pub sample_count: usize,
    pub successful_probes: usize,
    pub lost_probes: usize,
    pub loss_percent: f64,
    pub loss_rating: PacketLossRating,
    pub mean_rtt_ms: f64,
    pub min_rtt_ms: Option<u32>,
    pub max_rtt_ms: Option<u32>,
    /// Sample standard deviation of round-trip times (RTT) in milliseconds.
    pub std_dev_ms: f64,
    /// Mean absolute consecutive difference in milliseconds.
    pub jitter_ms: f64,
    /// RFC 3550 EWMA smoothed jitter estimator in milliseconds.
    pub rfc3550_jitter_ms: f64,
    /// Chronological probe results (None indicates a dropped/timed-out packet).
    pub samples: Vec<Option<u32>>,
    /// 0..=100 composite node stability score.
    pub stability_score: u8,
    /// 1..=5 star rating.
    pub star_rating: u8,
}

impl JitterCalculation {
    /// Compute exact jitter, sample standard deviation, and packet loss rating from probe samples.
    pub fn from_samples(samples: &[Option<u32>]) -> Self {
        let sample_count = samples.len();
        if sample_count == 0 {
            return Self {
                sample_count: 0,
                successful_probes: 0,
                lost_probes: 0,
                loss_percent: 0.0,
                loss_rating: PacketLossRating::Excellent,
                mean_rtt_ms: 0.0,
                min_rtt_ms: None,
                max_rtt_ms: None,
                std_dev_ms: 0.0,
                jitter_ms: 0.0,
                rfc3550_jitter_ms: 0.0,
                samples: Vec::new(),
                stability_score: 100,
                star_rating: 5,
            };
        }

        let mut valid_rtts = Vec::with_capacity(sample_count);
        let mut lost_probes = 0;

        for &s in samples {
            if let Some(rtt) = s {
                valid_rtts.push(rtt as f64);
            } else {
                lost_probes += 1;
            }
        }

        let successful_probes = valid_rtts.len();
        let loss_percent = (lost_probes as f64 / sample_count as f64) * 100.0;
        let loss_rating = PacketLossRating::from_loss_percent(loss_percent);

        if successful_probes == 0 {
            return Self {
                sample_count,
                successful_probes: 0,
                lost_probes,
                loss_percent: 100.0,
                loss_rating: PacketLossRating::Dead,
                mean_rtt_ms: 0.0,
                min_rtt_ms: None,
                max_rtt_ms: None,
                std_dev_ms: 0.0,
                jitter_ms: 0.0,
                rfc3550_jitter_ms: 0.0,
                samples: samples.to_vec(),
                stability_score: 0,
                star_rating: 1,
            };
        }

        let sum: f64 = valid_rtts.iter().sum();
        let mean_rtt_ms = sum / successful_probes as f64;
        let min_rtt_ms = valid_rtts.iter().map(|&x| x as u32).min();
        let max_rtt_ms = valid_rtts.iter().map(|&x| x as u32).max();

        // Standard deviation (sample standard deviation with Bessel's correction)
        let std_dev_ms = if successful_probes > 1 {
            let variance = valid_rtts
                .iter()
                .map(|&x| (x - mean_rtt_ms).powi(2))
                .sum::<f64>()
                / (successful_probes - 1) as f64;
            variance.sqrt()
        } else {
            0.0
        };

        // Mean consecutive difference jitter
        let jitter_ms = if successful_probes > 1 {
            let diff_sum: f64 = valid_rtts.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
            diff_sum / (successful_probes - 1) as f64
        } else {
            0.0
        };

        // RFC 3550 EWMA smoothed jitter: J = J + (|D| - J) / 16.0
        let mut rfc3550 = 0.0;
        for w in valid_rtts.windows(2) {
            let diff = (w[1] - w[0]).abs();
            rfc3550 += (diff - rfc3550) / 16.0;
        }

        // Multi-dimensional stability scoring (0..=100)
        let mut score = 100.0;
        if mean_rtt_ms > 50.0 {
            score -= ((mean_rtt_ms - 50.0) / 5.0).min(40.0);
        }
        score -= (std_dev_ms * 1.5).min(30.0);
        score -= (loss_percent * 2.0).min(50.0);
        let stability_score = score.clamp(0.0, 100.0).round() as u8;

        let star_rating = match stability_score {
            85..=100 => 5,
            70..=84 => 4,
            50..=69 => 3,
            30..=49 => 2,
            _ => 1,
        };

        Self {
            sample_count,
            successful_probes,
            lost_probes,
            loss_percent,
            loss_rating,
            mean_rtt_ms,
            min_rtt_ms,
            max_rtt_ms,
            std_dev_ms,
            jitter_ms,
            rfc3550_jitter_ms: rfc3550,
            samples: samples.to_vec(),
            stability_score,
            star_rating,
        }
    }

    pub fn demo_fixture() -> Self {
        Self::from_samples(&[Some(38), Some(41), Some(36), Some(39), Some(37)])
    }
}

/// A target set bound to the exact Core activation and inventory revision the
/// surface observed. Outbound identities are never inferred from display names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeedtestTarget {
    core_scope: CoreScope,
    expected_inventory_revision: SnapshotRevision,
    outbound_scope: OutboundProbeBatchScope,
    rounds: SpeedtestRounds,
}

impl SpeedtestTarget {
    pub fn new(
        core_scope: CoreScope,
        expected_inventory_revision: SnapshotRevision,
        outbound_scope: OutboundProbeBatchScope,
        rounds: SpeedtestRounds,
    ) -> Result<Self, SpeedtestTargetError> {
        outbound_scope
            .validate_scope(&core_scope)
            .map_err(SpeedtestTargetError::InvalidOutboundScope)?;
        Ok(Self {
            core_scope,
            expected_inventory_revision,
            outbound_scope,
            rounds,
        })
    }

    /// Recheck the boundary invariants after deserialization.
    pub fn validate(&self) -> Result<(), SpeedtestTargetError> {
        self.outbound_scope
            .validate_scope(&self.core_scope)
            .map_err(SpeedtestTargetError::InvalidOutboundScope)
    }

    pub fn core_scope(&self) -> &CoreScope {
        &self.core_scope
    }

    pub const fn expected_inventory_revision(&self) -> SnapshotRevision {
        self.expected_inventory_revision
    }

    pub fn outbound_scope(&self) -> &OutboundProbeBatchScope {
        &self.outbound_scope
    }

    pub const fn rounds(&self) -> SpeedtestRounds {
        self.rounds
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpeedtestTargetError {
    InvalidOutboundScope(OutboundContractError),
}

impl std::fmt::Display for SpeedtestTargetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOutboundScope(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SpeedtestTargetError {}

/// Number of latency samples to collect for every target in one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SpeedtestRounds(NonZeroU8);

impl SpeedtestRounds {
    pub const DEFAULT: Self = Self(NonZeroU8::new(5).expect("non-zero rounds"));

    pub fn new(value: u8) -> Result<Self, SpeedtestRoundsError> {
        if value > 50 {
            return Err(SpeedtestRoundsError::ExceedsMaximum);
        }
        NonZeroU8::new(value)
            .map(Self)
            .ok_or(SpeedtestRoundsError::Zero)
    }

    pub const fn get(self) -> u8 {
        self.0.get()
    }
}

impl Default for SpeedtestRounds {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl<'de> serde::Deserialize<'de> for SpeedtestRounds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        Self::new(u8::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedtestRoundsError {
    Zero,
    ExceedsMaximum,
}

impl std::fmt::Display for SpeedtestRoundsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Zero => "speedtest rounds must be greater than zero",
            Self::ExceedsMaximum => "speedtest rounds must not exceed 50",
        })
    }
}

impl std::error::Error for SpeedtestRoundsError {}

/// Bounded parallelism for a multi-round speedtest run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SpeedtestConcurrency(NonZeroU8);

impl SpeedtestConcurrency {
    pub fn new(value: u8) -> Result<Self, SpeedtestConcurrencyError> {
        if value > 30 {
            return Err(SpeedtestConcurrencyError::ExceedsMaximum);
        }
        NonZeroU8::new(value)
            .map(Self)
            .ok_or(SpeedtestConcurrencyError::Zero)
    }

    pub const fn get(self) -> u8 {
        self.0.get()
    }
}

impl Default for SpeedtestConcurrency {
    fn default() -> Self {
        Self(NonZeroU8::new(30).expect("non-zero concurrency"))
    }
}

impl<'de> serde::Deserialize<'de> for SpeedtestConcurrency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        Self::new(u8::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedtestConcurrencyError {
    Zero,
    ExceedsMaximum,
}

impl std::fmt::Display for SpeedtestConcurrencyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Zero => "speedtest concurrency must be greater than zero",
            Self::ExceedsMaximum => "speedtest concurrency must not exceed 30",
        })
    }
}

impl std::error::Error for SpeedtestConcurrencyError {}

/// The outcome of one sample in the multi-round speedtest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedtestSampleOutcome {
    Measured { delay_ms: u32 },
    TimedOut,
    Unsupported,
    Failed { failure: Failure },
    Cancelled,
}

impl From<&OutboundProbeState> for SpeedtestSampleOutcome {
    fn from(state: &OutboundProbeState) -> Self {
        match state {
            OutboundProbeState::Measured { delay_ms } => Self::Measured {
                delay_ms: *delay_ms,
            },
            OutboundProbeState::TimedOut => Self::TimedOut,
            OutboundProbeState::Unsupported => Self::Unsupported,
            OutboundProbeState::Failed { failure } => Self::Failed {
                failure: failure.clone(),
            },
            OutboundProbeState::Unknown | OutboundProbeState::Probing => Self::Failed {
                failure: Failure::new(
                    crate::error::ErrorCode::InvalidState,
                    "outbound probe returned a nonterminal sample",
                    false,
                ),
            },
        }
    }
}

/// Explicit terminal state for one outbound's multi-round probe sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedtestResultState {
    Completed,
    TimedOut,
    Unsupported,
    Failed { failure: Failure },
    Cancelled,
}

/// One target result, identified by the engine-owned outbound ID.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeedtestOutboundResult {
    pub outbound_id: OutboundId,
    pub display_name: Option<String>,
    pub protocol_display_name: Option<String>,
    pub samples: Vec<SpeedtestSampleOutcome>,
    pub state: SpeedtestResultState,
    pub tested_at_epoch_ms: u64,
}

impl SpeedtestOutboundResult {
    pub fn calculation(&self) -> Option<JitterCalculation> {
        let samples = self
            .samples
            .iter()
            .filter_map(|sample| match sample {
                SpeedtestSampleOutcome::Measured { delay_ms } => Some(Some(*delay_ms)),
                SpeedtestSampleOutcome::TimedOut | SpeedtestSampleOutcome::Failed { .. } => {
                    Some(None)
                }
                SpeedtestSampleOutcome::Unsupported | SpeedtestSampleOutcome::Cancelled => None,
            })
            .collect::<Vec<_>>();
        (!samples.is_empty()).then(|| JitterCalculation::from_samples(&samples))
    }

    pub fn delay_ms(&self) -> Option<u32> {
        self.calculation()
            .and_then(|calculation| calculation.min_rtt_ms)
    }

    pub fn is_available(&self) -> bool {
        self.calculation()
            .is_some_and(|calculation| calculation.successful_probes > 0)
    }
}

/// Stable identifier for one accepted speedtest run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpeedtestRunId(NonZeroU64);

impl SpeedtestRunId {
    pub fn new(value: u64) -> Result<Self, SpeedtestRunIdError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(SpeedtestRunIdError::Zero)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedtestRunIdError {
    Zero,
}

/// Mutually exclusive lifecycle of the current speedtest run. Run identity,
/// target and progress live in the variant that owns them, so impossible
/// combinations cannot be represented in a snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedtestRunState {
    Idle,
    Probing {
        run_id: SpeedtestRunId,
        target: SpeedtestTarget,
        progress: SpeedtestProgress,
    },
    CancelRequested {
        run_id: SpeedtestRunId,
        target: SpeedtestTarget,
        progress: SpeedtestProgress,
    },
    Completed {
        run_id: SpeedtestRunId,
        target: SpeedtestTarget,
        progress: SpeedtestProgress,
    },
    Cancelled {
        run_id: SpeedtestRunId,
        target: SpeedtestTarget,
        progress: SpeedtestProgress,
    },
    Failed {
        run_id: SpeedtestRunId,
        target: SpeedtestTarget,
        failure: Failure,
    },
    Unavailable {
        failure: Failure,
    },
}

impl SpeedtestRunState {
    pub fn run_id(&self) -> Option<SpeedtestRunId> {
        match self {
            Self::Idle | Self::Unavailable { .. } => None,
            Self::Probing { run_id, .. }
            | Self::CancelRequested { run_id, .. }
            | Self::Completed { run_id, .. }
            | Self::Cancelled { run_id, .. }
            | Self::Failed { run_id, .. } => Some(*run_id),
        }
    }

    pub fn target(&self) -> Option<&SpeedtestTarget> {
        match self {
            Self::Idle => None,
            Self::Probing { target, .. }
            | Self::CancelRequested { target, .. }
            | Self::Completed { target, .. }
            | Self::Cancelled { target, .. }
            | Self::Failed { target, .. } => Some(target),
            Self::Unavailable { .. } => None,
        }
    }

    pub fn progress(&self) -> Option<&SpeedtestProgress> {
        match self {
            Self::Probing { progress, .. }
            | Self::CancelRequested { progress, .. }
            | Self::Completed { progress, .. }
            | Self::Cancelled { progress, .. } => Some(progress),
            Self::Idle | Self::Failed { .. } | Self::Unavailable { .. } => None,
        }
    }

    pub fn phase(&self) -> SpeedtestPhase {
        match self {
            Self::Idle => SpeedtestPhase::Idle,
            Self::Probing { .. } => SpeedtestPhase::Probing,
            Self::CancelRequested { .. } => SpeedtestPhase::CancelRequested,
            Self::Completed { .. } => SpeedtestPhase::Completed,
            Self::Cancelled { .. } => SpeedtestPhase::Cancelled,
            Self::Failed { failure, .. } => SpeedtestPhase::Failed {
                failure: failure.clone(),
            },
            Self::Unavailable { failure, .. } => SpeedtestPhase::Unavailable {
                failure: failure.clone(),
            },
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self, Self::Probing { .. } | Self::CancelRequested { .. })
    }
}

/// Presentation phase derived from [`SpeedtestRunState`]. It deliberately
/// carries no independent run identity or target fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedtestPhase {
    Idle,
    Probing,
    CancelRequested,
    Completed,
    Cancelled,
    Failed { failure: Failure },
    Unavailable { failure: Failure },
}

/// Receipt that a run passed preflight and was scheduled by its Core owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeedtestStartReceipt {
    pub run_id: SpeedtestRunId,
}

/// Result of requesting cancellation from the shared speedtest owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedtestCancelOutcome {
    Requested {
        run_id: SpeedtestRunId,
    },
    AlreadyRequested {
        run_id: SpeedtestRunId,
    },
    RunAlreadyFinished {
        run_id: SpeedtestRunId,
    },
    StaleRun {
        requested_run_id: SpeedtestRunId,
        current_run_id: Option<SpeedtestRunId>,
    },
}

/// Real-time progress update; completion is represented by the enclosing phase.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeedtestProgress {
    pub completed_targets: usize,
    pub total_targets: usize,
}

impl SpeedtestProgress {
    pub fn percent(&self) -> f64 {
        if self.total_targets == 0 {
            0.0
        } else {
            (self.completed_targets.min(self.total_targets) as f64 / self.total_targets as f64)
                * 100.0
        }
    }
}

/// Historical summary entry. Older runs may have aggregate metrics without a
/// recoverable outbound identity; those remain explicitly unattributed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoricalSpeedtestRecord {
    pub run_id: SpeedtestRunId,
    pub timestamp_epoch_ms: u64,
    pub target: Option<SpeedtestTarget>,
    pub total_targets: usize,
    pub completed_targets: usize,
    pub avg_latency_ms: Option<f64>,
    pub avg_jitter_ms: Option<f64>,
}

/// Shared canonical read model for multi-round typed outbound probing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeedtestSnapshot {
    pub revision: u64,
    pub state: SpeedtestRunState,
    pub concurrency: SpeedtestConcurrency,
    /// Preserves observed target order for stable ties and deterministic UI.
    pub results: Vec<SpeedtestOutboundResult>,
    pub recent_history: Vec<HistoricalSpeedtestRecord>,
}

impl Default for SpeedtestSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            state: SpeedtestRunState::Idle,
            concurrency: SpeedtestConcurrency::default(),
            results: Vec::new(),
            recent_history: Vec::new(),
        }
    }
}

impl SpeedtestSnapshot {
    pub fn is_running(&self) -> bool {
        self.state.is_running()
    }

    pub fn is_drawable(&self) -> bool {
        !self.results.is_empty() || self.is_running()
    }

    pub fn phase(&self) -> SpeedtestPhase {
        self.state.phase()
    }

    pub fn unavailable(failure: Failure) -> Self {
        Self {
            state: SpeedtestRunState::Unavailable { failure },
            ..Self::default()
        }
    }

    /// Deterministic typed fixture for demo/capture surfaces. Product surfaces
    /// must consume an owner-authored snapshot instead of using this fixture.
    pub fn demo_fixture() -> Self {
        use crate::core_identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};

        let core_scope = CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("fixture").expect("fixture engine"),
                CoreInstanceId::new("speedtest-demo").expect("fixture instance"),
                None,
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        };
        let outbound_engine = core_scope.identity.engine.clone();
        let outbound_id = move |value: &str| {
            OutboundId::new(outbound_engine.clone(), value).expect("fixture outbound")
        };
        let target = SpeedtestTarget::new(
            core_scope,
            SnapshotRevision::new(1),
            OutboundProbeBatchScope::All,
            SpeedtestRounds::DEFAULT,
        )
        .expect("fixture target");
        let run_id = SpeedtestRunId::new(1).expect("fixture run identity");
        let result =
            |id: &str, name: &str, delays: &[Option<u32>], state| SpeedtestOutboundResult {
                outbound_id: outbound_id(id),
                display_name: Some(name.to_owned()),
                protocol_display_name: Some("Shadowsocks".to_owned()),
                samples: delays
                    .iter()
                    .map(|delay| match delay {
                        Some(delay_ms) => SpeedtestSampleOutcome::Measured {
                            delay_ms: *delay_ms,
                        },
                        None => SpeedtestSampleOutcome::TimedOut,
                    })
                    .collect(),
                state,
                tested_at_epoch_ms: 1_700_000_000_000,
            };
        Self {
            revision: 1,
            state: SpeedtestRunState::Completed {
                run_id,
                target: target.clone(),
                progress: SpeedtestProgress {
                    completed_targets: 3,
                    total_targets: 3,
                },
            },
            concurrency: SpeedtestConcurrency::default(),
            results: vec![
                result(
                    "hk-primary",
                    "🇭🇰 香港 01 · BGP 专线",
                    &[Some(36), Some(38), Some(35), Some(37)],
                    SpeedtestResultState::Completed,
                ),
                result(
                    "jp-primary",
                    "🇯🇵 日本东京 02 · 极速",
                    &[Some(65), Some(68), Some(64), Some(66)],
                    SpeedtestResultState::Completed,
                ),
                result(
                    "unavailable-node",
                    "💀 超时不可用节点 01",
                    &[None, None, None],
                    SpeedtestResultState::TimedOut,
                ),
            ],
            recent_history: vec![HistoricalSpeedtestRecord {
                run_id,
                timestamp_epoch_ms: 1_700_000_000_000,
                target: Some(target),
                total_targets: 3,
                completed_targets: 3,
                avg_latency_ms: Some(50.5),
                avg_jitter_ms: Some(1.5),
            }],
        }
    }

    pub fn result(&self, outbound_id: &OutboundId) -> Option<&SpeedtestOutboundResult> {
        self.results
            .iter()
            .find(|result| &result.outbound_id == outbound_id)
    }

    pub fn node_count(&self) -> usize {
        self.results.len()
    }

    pub fn available_count(&self) -> usize {
        self.results
            .iter()
            .filter(|result| result.is_available())
            .count()
    }

    pub fn timed_out(&self) -> Vec<&SpeedtestOutboundResult> {
        self.results
            .iter()
            .filter(|result| matches!(&result.state, SpeedtestResultState::TimedOut))
            .collect()
    }

    pub fn fastest(&self) -> Option<&SpeedtestOutboundResult> {
        self.results
            .iter()
            .filter(|result| result.is_available())
            .min_by_key(|result| result.delay_ms())
    }

    pub fn sorted_by_latency(&self) -> Vec<&SpeedtestOutboundResult> {
        let mut list = self.results.iter().collect::<Vec<_>>();
        list.sort_by(|left, right| match (left.delay_ms(), right.delay_ms()) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        });
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_calculation_preserves_multi_round_samples_and_loss() {
        let calculation =
            JitterCalculation::from_samples(&[Some(50), None, Some(60), Some(55), None]);
        assert_eq!(calculation.sample_count, 5);
        assert_eq!(calculation.successful_probes, 3);
        assert_eq!(calculation.lost_probes, 2);
        assert_eq!(calculation.loss_percent, 40.0);
        assert_eq!(calculation.loss_rating, PacketLossRating::Poor);
        assert_eq!(calculation.min_rtt_ms, Some(50));
        assert_eq!(calculation.max_rtt_ms, Some(60));
    }

    #[test]
    fn speedtest_limits_fail_closed_at_construction_and_deserialization() {
        assert!(matches!(
            SpeedtestRounds::new(0),
            Err(SpeedtestRoundsError::Zero)
        ));
        assert!(matches!(
            SpeedtestRounds::new(51),
            Err(SpeedtestRoundsError::ExceedsMaximum)
        ));
        assert!(matches!(
            SpeedtestConcurrency::new(0),
            Err(SpeedtestConcurrencyError::Zero)
        ));
        assert!(matches!(
            SpeedtestConcurrency::new(31),
            Err(SpeedtestConcurrencyError::ExceedsMaximum)
        ));
        assert!(serde_json::from_str::<SpeedtestRounds>("0").is_err());
        assert!(serde_json::from_str::<SpeedtestConcurrency>("31").is_err());
    }

    #[test]
    fn stable_latency_sort_keeps_inventory_order_for_equal_and_missing_samples() {
        let engine = crate::core_identity::EngineId::new("fixture").expect("engine");
        let first_id = OutboundId::new(engine.clone(), "first").expect("outbound");
        let second_id = OutboundId::new(engine.clone(), "second").expect("outbound");
        let third_id = OutboundId::new(engine, "third").expect("outbound");
        let result = |outbound_id: OutboundId, delay: Option<u32>| SpeedtestOutboundResult {
            outbound_id,
            display_name: None,
            protocol_display_name: None,
            samples: delay
                .map(|delay_ms| vec![SpeedtestSampleOutcome::Measured { delay_ms }])
                .unwrap_or_default(),
            state: if delay.is_some() {
                SpeedtestResultState::Completed
            } else {
                SpeedtestResultState::TimedOut
            },
            tested_at_epoch_ms: 1,
        };
        let snapshot = SpeedtestSnapshot {
            results: vec![
                result(first_id.clone(), Some(40)),
                result(second_id.clone(), Some(40)),
                result(third_id.clone(), None),
            ],
            ..SpeedtestSnapshot::default()
        };
        assert_eq!(
            snapshot
                .sorted_by_latency()
                .into_iter()
                .map(|entry| entry.outbound_id.clone())
                .collect::<Vec<_>>(),
            vec![first_id, second_id, third_id]
        );
    }
}
