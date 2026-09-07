//! Shared dynamic scale for upload/download rate charts.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrafficRateUnit {
    #[default]
    Bytes,
    KibiBytes,
    MebiBytes,
    GibiBytes,
}

impl TrafficRateUnit {
    pub const fn factor(self) -> f64 {
        match self {
            Self::Bytes => 1.0,
            Self::KibiBytes => 1024.0,
            Self::MebiBytes => 1024.0 * 1024.0,
            Self::GibiBytes => 1024.0 * 1024.0 * 1024.0,
        }
    }

    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Bytes => "B/s",
            Self::KibiBytes => "KiB/s",
            Self::MebiBytes => "MiB/s",
            Self::GibiBytes => "GiB/s",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrafficScaleSnapshot {
    pub peak_bps: f64,
    pub max_bps: f64,
    pub unit: TrafficRateUnit,
    pub unit_factor: f64,
    pub ticks: Vec<f64>,
    pub revision: u64,
}

impl Default for TrafficScaleSnapshot {
    fn default() -> Self {
        Self {
            peak_bps: 0.0,
            max_bps: 1.0,
            unit: TrafficRateUnit::Bytes,
            unit_factor: TrafficRateUnit::Bytes.factor(),
            ticks: vec![0.0, 0.25, 0.5, 0.75, 1.0],
            revision: 0,
        }
    }
}

impl TrafficScaleSnapshot {
    pub fn format_max(&self) -> String {
        format_rate(self.max_bps / self.unit_factor, self.unit.suffix())
    }

    pub fn format_tick(&self, fraction: f64) -> String {
        format_rate(
            (self.max_bps * fraction.clamp(0.0, 1.0)) / self.unit_factor,
            self.unit.suffix(),
        )
    }
}

fn format_rate(value: f64, suffix: &str) -> String {
    let value = if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    };
    if value >= 100.0 {
        format!("{value:.0} {suffix}")
    } else if value >= 10.0 {
        format!("{value:.1} {suffix}")
    } else {
        format!("{value:.2} {suffix}")
    }
}
