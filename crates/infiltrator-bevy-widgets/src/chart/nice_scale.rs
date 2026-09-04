//! Wilkinson's "Nice Numbers" algorithm for clean, ergonomic chart axis grid lines and tick labels.
//!
//! Charter (docs/BEVY_UI_FRONTEND.md):
//! Pure mathematical model generating human-readable rounded tick steps (1, 2, 5, 10 * 10^p).

/// Calculates a "nice" rounded number approximately equal to x.
pub fn nice_number(value: f32, round: bool) -> f32 {
    let exponent = value.log10().floor();
    let fraction = value / 10.0f32.powf(exponent);

    let nice_fraction = if round {
        if fraction < 1.5 {
            1.0
        } else if fraction < 3.0 {
            2.0
        } else if fraction < 7.0 {
            5.0
        } else {
            10.0
        }
    } else if fraction <= 1.0 {
        1.0
    } else if fraction <= 2.0 {
        2.0
    } else if fraction <= 5.0 {
        5.0
    } else {
        10.0
    };

    nice_fraction * 10.0f32.powf(exponent)
}

/// Resolved ergonomic chart axis scale with clean tick steps.
#[derive(Clone, Debug, PartialEq)]
pub struct NiceScale {
    pub min: f32,
    pub max: f32,
    pub tick_spacing: f32,
    pub ticks: Vec<f32>,
}

impl NiceScale {
    /// Compute a nice scale covering [min_val, max_val] with approximately `target_ticks` divisions.
    pub fn compute(min_val: f32, max_val: f32, target_ticks: usize) -> Self {
        let max_ticks = target_ticks.max(2);
        let raw_range = (max_val - min_val).max(1e-4);
        let raw_spacing = raw_range / (max_ticks as f32);
        let tick_spacing = nice_number(raw_spacing, true).max(1e-4);

        let nice_min = (min_val / tick_spacing).floor() * tick_spacing;
        let nice_max = (max_val / tick_spacing).ceil() * tick_spacing;

        let mut ticks = Vec::new();
        let mut curr = nice_min;
        while curr <= nice_max + tick_spacing * 0.5 {
            ticks.push(curr);
            curr += tick_spacing;
        }

        Self {
            min: nice_min,
            max: nice_max,
            tick_spacing,
            ticks,
        }
    }

    /// Format tick value into a clean human-readable string.
    pub fn format_tick(&self, val: f32, unit: &str) -> String {
        if val.fract().abs() < 1e-3 {
            format!("{} {}", val as i64, unit)
        } else {
            format!("{:.1} {}", val, unit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nice_scale_ticks_generation() {
        // Range 0 to 95 with 5 divisions -> nice ticks at 0, 20, 40, 60, 80, 100
        let scale = NiceScale::compute(0.0, 95.0, 5);
        assert_eq!(scale.min, 0.0);
        assert_eq!(scale.max, 100.0);
        assert_eq!(scale.tick_spacing, 20.0);
        assert_eq!(scale.ticks, vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);

        // Formats cleanly
        assert_eq!(scale.format_tick(40.0, "MB/s"), "40 MB/s");
    }

    #[test]
    fn test_nice_scale_small_numbers() {
        // Latency range 0.0 to 14.5 ms with 4 divisions
        let scale = NiceScale::compute(0.0, 14.5, 4);
        assert_eq!(scale.min, 0.0);
        assert_eq!(scale.max, 15.0);
        assert_eq!(scale.tick_spacing, 5.0);
        assert_eq!(scale.ticks, vec![0.0, 5.0, 10.0, 15.0]);
    }
}
