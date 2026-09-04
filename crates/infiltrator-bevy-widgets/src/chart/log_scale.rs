//! Continuous logarithmic scale mapper for high dynamic range network telemetry (1 KB/s to 1 GB/s).
//!
//! Charter (docs/BEVY_UI_FRONTEND.md):
//! Pure mathematical model mapping exponential multi-order-of-magnitude traffic rates to [0.0..1.0]
//! visual coordinates, preventing low-rate clipping while cleanly displaying massive burst peaks.

/// Dynamic range logarithmic scale mapper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogarithmicScaleMapper {
    pub max_value: f32,
    pub compression_factor: f32,
    log_denominator: f32,
}

impl LogarithmicScaleMapper {
    /// Create a new mapper with maximum value and sensitivity factor k (typically 1.0 to 10.0).
    pub fn new(max_value: f32, compression_factor: f32) -> Self {
        let max_val = max_value.max(1.0);
        let k = compression_factor.max(0.01);
        let log_denominator = (1.0 + max_val * k).ln();

        Self {
            max_value: max_val,
            compression_factor: k,
            log_denominator,
        }
    }

    /// Map a raw non-negative value to normalized [0.0..1.0] visual coordinate.
    pub fn map_to_normalized(&self, value: f32) -> f32 {
        if value <= 0.0 {
            return 0.0;
        }
        let clamped = value.min(self.max_value);
        let numerator = (1.0 + clamped * self.compression_factor).ln();
        (numerator / self.log_denominator).clamp(0.0, 1.0)
    }

    /// Analytical inverse: reconstruct raw value from visual coordinate in [0.0..1.0].
    pub fn map_from_normalized(&self, normalized: f32) -> f32 {
        let norm = normalized.clamp(0.0, 1.0);
        if norm <= 0.0 {
            return 0.0;
        }
        let exp_val = (norm * self.log_denominator).exp();
        ((exp_val - 1.0) / self.compression_factor).clamp(0.0, self.max_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_scale_mapper_monotonicity_and_round_trip() {
        let mapper = LogarithmicScaleMapper::new(100_000_000.0, 1.0); // 100 MB/s

        // Zero maps to zero
        assert_eq!(mapper.map_to_normalized(0.0), 0.0);
        assert_eq!(mapper.map_from_normalized(0.0), 0.0);

        // Max maps to 1.0
        assert_eq!(mapper.map_to_normalized(100_000_000.0), 1.0);
        assert!((mapper.map_from_normalized(1.0) - 100_000_000.0).abs() < 10.0);

        // Small value (10 KB/s = 10_000) is given visible non-zero height
        let small_y = mapper.map_to_normalized(10_000.0);
        assert!(small_y > 0.4); // compressed logarithmic boost gives low rates visibility

        // Medium value is strictly greater than small
        let med_y = mapper.map_to_normalized(1_000_000.0); // 1 MB/s
        assert!(med_y > small_y);

        // Round-trip test
        let recovered = mapper.map_from_normalized(small_y);
        assert!((recovered - 10_000.0).abs() < 1.0);
    }
}
