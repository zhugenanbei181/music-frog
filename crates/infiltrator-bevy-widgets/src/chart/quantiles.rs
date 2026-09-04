//! Statistical latency quantiles (P50, P90, P99) and empirical CDF distribution curves.
//!
//! Charter (docs/BEVY_UI_FRONTEND.md):
//! Pure mathematical model analyzing latency distribution curves for proxy nodes and DNS servers.

use bevy::color::Color;

/// Qualitative latency tier for visual color coding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LatencyTier {
    Fast,
    Moderate,
    Slow,
    Timeout,
}

impl LatencyTier {
    pub fn from_latency_ms(ms: f32) -> Self {
        if ms < 100.0 {
            Self::Fast
        } else if ms < 250.0 {
            Self::Moderate
        } else if ms < 500.0 {
            Self::Slow
        } else {
            Self::Timeout
        }
    }

    pub fn color_rgba(&self) -> Color {
        match self {
            Self::Fast => Color::srgba(0.15, 0.85, 0.35, 1.0), // Green
            Self::Moderate => Color::srgba(0.95, 0.80, 0.20, 1.0), // Yellow
            Self::Slow => Color::srgba(0.95, 0.50, 0.15, 1.0), // Orange
            Self::Timeout => Color::srgba(0.90, 0.25, 0.25, 1.0), // Red
        }
    }
}

/// Comprehensive statistical quantiles of a latency sample set.
#[derive(Clone, Debug, PartialEq)]
pub struct LatencyQuantiles {
    pub count: usize,
    pub min: f32,
    pub p50: f32,
    pub p90: f32,
    pub p95: f32,
    pub p99: f32,
    pub max: f32,
    pub mean: f32,
    pub std_dev: f32,
}

impl LatencyQuantiles {
    /// Compute quantiles from an unsorted slice of latency samples (milliseconds).
    pub fn compute(samples: &[f32]) -> Option<Self> {
        let valid_samples: Vec<f32> = samples
            .iter()
            .copied()
            .filter(|&v| v.is_finite() && v >= 0.0)
            .collect();

        let count = valid_samples.len();
        if count == 0 {
            return None;
        }

        let mut sorted = valid_samples;
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let min = sorted[0];
        let max = sorted[count - 1];

        let sum: f32 = sorted.iter().sum();
        let mean = sum / (count as f32);

        let variance: f32 =
            sorted.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / (count as f32);
        let std_dev = variance.sqrt();

        let p50 = Self::percentile_sorted(&sorted, 0.50);
        let p90 = Self::percentile_sorted(&sorted, 0.90);
        let p95 = Self::percentile_sorted(&sorted, 0.95);
        let p99 = Self::percentile_sorted(&sorted, 0.99);

        Some(Self {
            count,
            min,
            p50,
            p90,
            p95,
            p99,
            max,
            mean,
            std_dev,
        })
    }

    fn percentile_sorted(sorted: &[f32], fraction: f32) -> f32 {
        let n = sorted.len();
        if n == 1 {
            return sorted[0];
        }
        let rank = fraction * (n - 1) as f32;
        let lower = rank.floor() as usize;
        let upper = rank.ceil() as usize;
        let weight = rank - lower as f32;

        sorted[lower] * (1.0 - weight) + sorted[upper] * weight
    }
}

/// A point on an empirical Cumulative Distribution Function (CDF) curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CdfPoint {
    pub latency_ms: f32,
    pub cumulative_fraction: f32,
}

/// Computes empirical CDF curve points from latency samples.
pub fn compute_empirical_cdf(samples: &[f32], steps: usize) -> Vec<CdfPoint> {
    let mut sorted: Vec<f32> = samples
        .iter()
        .copied()
        .filter(|&v| v.is_finite() && v >= 0.0)
        .collect();

    let count = sorted.len();
    if count == 0 {
        return Vec::new();
    }
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let steps = steps.clamp(2, count);
    let mut cdf = Vec::with_capacity(steps);

    for i in 0..steps {
        let idx = (i * (count - 1)) / (steps - 1);
        let val = sorted[idx];
        let cumulative = (idx + 1) as f32 / (count as f32);
        cdf.push(CdfPoint {
            latency_ms: val,
            cumulative_fraction: cumulative,
        });
    }

    cdf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_quantiles_computation() {
        let samples = vec![
            20.0, 30.0, 50.0, 70.0, 90.0, 100.0, 150.0, 200.0, 300.0, 1000.0,
        ];
        let quantiles = LatencyQuantiles::compute(&samples).expect("valid quantiles");

        assert_eq!(quantiles.count, 10);
        assert_eq!(quantiles.min, 20.0);
        assert_eq!(quantiles.max, 1000.0);
        // p50 is median
        assert!(quantiles.p50 >= 90.0 && quantiles.p50 <= 100.0);
        // p90 should be high
        assert!(quantiles.p90 >= 300.0);
        assert!(quantiles.std_dev > 0.0);
    }

    #[test]
    fn test_empirical_cdf_monotonicity() {
        let samples = vec![10.0, 20.0, 25.0, 40.0, 60.0, 80.0, 120.0, 250.0];
        let cdf = compute_empirical_cdf(&samples, 5);

        assert_eq!(cdf.len(), 5);
        assert_eq!(cdf[0].latency_ms, 10.0);
        assert_eq!(cdf.last().unwrap().latency_ms, 250.0);
        assert_eq!(cdf.last().unwrap().cumulative_fraction, 1.0);

        // Monotonically increasing
        for window in cdf.windows(2) {
            assert!(window[1].latency_ms >= window[0].latency_ms);
            assert!(window[1].cumulative_fraction >= window[0].cumulative_fraction);
        }
    }
}
