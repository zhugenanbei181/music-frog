//! Measurement provenance and semantic bands shared by native renderers.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencyReading {
    NotObserved,
    Measured(u32),
    ControllerHistory(u32),
}

/// Canonical latency classification bands (GAP-7.6).
///
/// Threshold partition:
/// - `< 100ms`: [`Self::Fast`] (Emerald green / Healthy)
/// - `100..250ms`: [`Self::Medium`] (Warm amber / Degraded warning)
/// - `≥ 250ms`: [`Self::Slow`] (Alert red / Failed)
/// - `0ms`: [`Self::UnconfirmedZero`] in controller history / timeout
/// - `None`: [`Self::NotObserved`]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencyBand {
    NotObserved,
    UnconfirmedZero,
    Fast,
    Medium,
    Slow,
}

impl LatencyBand {
    /// Classify a controller-reported delay (0ms = unconfirmed / timeout).
    pub const fn classify(delay_ms: Option<u32>) -> Self {
        match delay_ms {
            None => Self::NotObserved,
            Some(0) => Self::UnconfirmedZero,
            Some(ms) if ms < 100 => Self::Fast,
            Some(ms) if ms < 250 => Self::Medium,
            Some(_) => Self::Slow,
        }
    }

    /// Classify a live measurement (0ms = valid loopback/cache response).
    pub const fn classify_measured(delay_ms: Option<u32>) -> Self {
        match delay_ms {
            None => Self::NotObserved,
            Some(ms) if ms < 100 => Self::Fast,
            Some(ms) if ms < 250 => Self::Medium,
            Some(_) => Self::Slow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_band_classification_aligns_across_thresholds() {
        assert_eq!(LatencyBand::classify(None), LatencyBand::NotObserved);
        assert_eq!(LatencyBand::classify(Some(0)), LatencyBand::UnconfirmedZero);
        assert_eq!(LatencyBand::classify(Some(1)), LatencyBand::Fast);
        assert_eq!(LatencyBand::classify(Some(45)), LatencyBand::Fast);
        assert_eq!(LatencyBand::classify(Some(99)), LatencyBand::Fast);
        assert_eq!(LatencyBand::classify(Some(100)), LatencyBand::Medium);
        assert_eq!(LatencyBand::classify(Some(158)), LatencyBand::Medium);
        assert_eq!(LatencyBand::classify(Some(249)), LatencyBand::Medium);
        assert_eq!(LatencyBand::classify(Some(250)), LatencyBand::Slow);
        assert_eq!(LatencyBand::classify(Some(500)), LatencyBand::Slow);

        assert_eq!(LatencyBand::classify_measured(Some(0)), LatencyBand::Fast);
        assert_eq!(LatencyBand::classify_measured(Some(45)), LatencyBand::Fast);
        assert_eq!(
            LatencyBand::classify_measured(Some(158)),
            LatencyBand::Medium
        );
        assert_eq!(LatencyBand::classify_measured(Some(300)), LatencyBand::Slow);
        assert_eq!(
            LatencyBand::classify_measured(None),
            LatencyBand::NotObserved
        );
    }
}
