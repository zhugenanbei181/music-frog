//! Toolkit-neutral rows for one accepted typed speedtest snapshot.

use crate::outbound::identity::OutboundId;
use crate::speedtest::{SpeedtestPhase, SpeedtestResultState, SpeedtestSampleOutcome};

#[derive(Clone, Debug, PartialEq)]
pub struct SpeedtestDetailRow {
    pub outbound_id: OutboundId,
    pub display_name: Option<String>,
    pub protocol_display_name: Option<String>,
    pub delay: Option<u32>,
    pub jitter_ms: Option<f64>,
    pub loss_percent: Option<f64>,
    pub sample_count: usize,
    pub state: SpeedtestResultState,
    pub samples: Vec<SpeedtestSampleOutcome>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpeedtestDetails {
    pub rows: Vec<SpeedtestDetailRow>,
    pub phase: SpeedtestPhase,
}
