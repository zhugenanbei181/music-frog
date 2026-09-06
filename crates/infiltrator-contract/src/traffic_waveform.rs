//! Shared live traffic waveform samples.

use serde::{Deserialize, Serialize};

pub const TRAFFIC_WAVEFORM_CAPACITY: usize = 60;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrafficSample {
    pub sampled_at_epoch_ms: Option<i64>,
    pub upload_bps: f64,
    pub download_bps: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrafficWaveformSnapshot {
    pub generation: u64,
    pub revision: u64,
    pub samples: Vec<TrafficSample>,
}

impl TrafficWaveformSnapshot {
    pub fn is_drawable(&self) -> bool {
        self.samples.len() >= 2
    }
}
