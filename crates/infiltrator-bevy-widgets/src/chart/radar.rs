//! Multi-dimensional proxy node health radar and bandwidth saturation detector.
//!
//! Charter (docs/BEVY_UI_FRONTEND.md §8.1.7 & §8.1.10):
//! Mathematical pure core computing normalized n-dimensional polygon radar metrics,
//! composite node health grading, and real-time bandwidth saturation burst alerts.

use std::f32::consts::PI;

/// A single dimensional metric on the radar chart.
#[derive(Clone, Debug, PartialEq)]
pub struct RadarMetric {
    pub name: String,
    pub raw_value: f32,
    pub max_value: f32,
    pub higher_is_better: bool,
}

impl RadarMetric {
    pub fn new(
        name: impl Into<String>,
        raw_value: f32,
        max_value: f32,
        higher_is_better: bool,
    ) -> Self {
        Self {
            name: name.into(),
            raw_value,
            max_value: max_value.max(1e-5),
            higher_is_better,
        }
    }

    /// Normalized score between 0.0 (worst) and 1.0 (best).
    pub fn normalized(&self) -> f32 {
        let ratio = (self.raw_value / self.max_value).clamp(0.0, 1.0);
        if self.higher_is_better {
            ratio
        } else {
            1.0 - ratio
        }
    }
}

/// Composite qualitative health grade for a proxy node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthGrade {
    Excellent,
    Good,
    Fair,
    Poor,
}

impl HealthGrade {
    pub fn from_score(score: f32) -> Self {
        if score >= 85.0 {
            Self::Excellent
        } else if score >= 70.0 {
            Self::Good
        } else if score >= 50.0 {
            Self::Fair
        } else {
            Self::Poor
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Excellent => "优异 (Excellent)",
            Self::Good => "良好 (Good)",
            Self::Fair => "一般 (Fair)",
            Self::Poor => "劣质 (Poor)",
        }
    }
}

/// Comprehensive multi-metric assessment of a proxy node.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeHealthAssessment {
    pub metrics: Vec<RadarMetric>,
    pub composite_score: f32,
    pub grade: HealthGrade,
}

impl NodeHealthAssessment {
    /// Assess a node based on latency (ms), jitter (ms), packet loss (%), and uptime (%).
    pub fn evaluate(
        latency_ms: f32,
        jitter_ms: f32,
        packet_loss_pct: f32,
        uptime_pct: f32,
    ) -> Self {
        let metrics = vec![
            RadarMetric::new("延迟", latency_ms, 500.0, false),
            RadarMetric::new("抖动", jitter_ms, 60.0, false),
            RadarMetric::new("丢包", packet_loss_pct, 50.0, false),
            RadarMetric::new("可用率", uptime_pct, 100.0, true),
        ];

        let weights = [0.35, 0.20, 0.25, 0.20];
        let mut composite = 0.0;
        for (m, w) in metrics.iter().zip(weights.iter()) {
            composite += m.normalized() * w;
        }
        let score = (composite * 100.0).clamp(0.0, 100.0);
        let grade = HealthGrade::from_score(score);

        Self {
            metrics,
            composite_score: score,
            grade,
        }
    }
}

/// Computes 2D polygon vertices for an N-axis radar chart in a normalized coordinate space.
pub struct RadarGeometry;

impl RadarGeometry {
    /// Computes (x, y) coordinates for polygon vertices around center (cx, cy) with radius R.
    /// Values must be in [0.0, 1.0].
    pub fn compute_vertices(cx: f32, cy: f32, radius: f32, values: &[f32]) -> Vec<(f32, f32)> {
        let n = values.len();
        if n < 3 {
            return Vec::new();
        }

        let angle_step = 2.0 * PI / (n as f32);
        let mut vertices = Vec::with_capacity(n);

        for (i, &v) in values.iter().enumerate() {
            let clamped_v = v.clamp(0.0, 1.0);
            let angle = -PI / 2.0 + (i as f32) * angle_step;
            let r = radius * clamped_v;
            let x = cx + r * angle.cos();
            let y = cy + r * angle.sin();
            vertices.push((x, y));
        }

        vertices
    }

    /// Computes polygon area using the Shoelace formula.
    pub fn polygon_area(vertices: &[(f32, f32)]) -> f32 {
        let n = vertices.len();
        if n < 3 {
            return 0.0;
        }
        let mut area = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            area += vertices[i].0 * vertices[j].1;
            area -= vertices[j].0 * vertices[i].1;
        }
        area.abs() * 0.5
    }
}

/// Bandwidth saturation alert levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaturationLevel {
    Normal,
    Elevated,
    Saturated,
    Bursting,
}

/// Real-time bandwidth saturation and burst spike detector.
#[derive(Clone, Debug, PartialEq)]
pub struct BandwidthSaturationDetector {
    pub capacity_bps: u64,
    pub current_down_bps: u64,
    pub current_up_bps: u64,
}

impl BandwidthSaturationDetector {
    pub fn new(capacity_bps: u64) -> Self {
        Self {
            capacity_bps: capacity_bps.max(1_000_000), // minimum 1Mbps
            current_down_bps: 0,
            current_up_bps: 0,
        }
    }

    pub fn update(&mut self, down_bps: u64, up_bps: u64) {
        self.current_down_bps = down_bps;
        self.current_up_bps = up_bps;
    }

    /// Combined utilization ratio [0.0, 1.0+].
    pub fn utilization_ratio(&self) -> f32 {
        let total = self.current_down_bps + self.current_up_bps;
        (total as f32) / (self.capacity_bps as f32)
    }

    /// Check if current throughput represents a sudden burst (>2.5x of baseline).
    pub fn is_bursting(&self, baseline_bps: u64) -> bool {
        let total = self.current_down_bps + self.current_up_bps;
        if baseline_bps == 0 {
            total > (self.capacity_bps / 4)
        } else {
            total > (baseline_bps * 5 / 2) && total > 1_000_000
        }
    }

    /// Evaluate current alert level.
    pub fn alert_level(&self, baseline_bps: u64) -> SaturationLevel {
        if self.is_bursting(baseline_bps) {
            SaturationLevel::Bursting
        } else {
            let ratio = self.utilization_ratio();
            if ratio >= 0.90 {
                SaturationLevel::Saturated
            } else if ratio >= 0.70 {
                SaturationLevel::Elevated
            } else {
                SaturationLevel::Normal
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radar_metric_normalization() {
        let lower_better = RadarMetric::new("延迟", 50.0, 500.0, false);
        assert!((lower_better.normalized() - 0.9).abs() < 1e-4);

        let higher_better = RadarMetric::new("可用率", 95.0, 100.0, true);
        assert!((higher_better.normalized() - 0.95).abs() < 1e-4);

        // Clamping check
        let extreme = RadarMetric::new("超时", 600.0, 500.0, false);
        assert_eq!(extreme.normalized(), 0.0);
    }

    #[test]
    fn test_composite_node_health_scoring() {
        let high_perf = NodeHealthAssessment::evaluate(35.0, 5.0, 0.0, 99.9);
        assert!(high_perf.composite_score >= 85.0);
        assert_eq!(high_perf.grade, HealthGrade::Excellent);

        let degraded = NodeHealthAssessment::evaluate(380.0, 45.0, 25.0, 80.0);
        assert!(degraded.composite_score < 60.0);
        assert_eq!(degraded.grade, HealthGrade::Poor);
    }

    #[test]
    fn test_radar_geometry_polygon_vertices() {
        let center = (100.0, 100.0);
        let radius = 50.0;
        let values = [1.0, 1.0, 1.0, 1.0];
        let vertices = RadarGeometry::compute_vertices(center.0, center.1, radius, &values);
        assert_eq!(vertices.len(), 4);

        // Top vertex is at angle -PI/2 -> (100, 50)
        assert!((vertices[0].0 - 100.0).abs() < 1e-4);
        assert!((vertices[0].1 - 50.0).abs() < 1e-4);

        // Area of regular diamond/square with radius 50: 2 * R^2 = 5000
        let area = RadarGeometry::polygon_area(&vertices);
        assert!((area - 5000.0).abs() < 1.0);
    }

    #[test]
    fn test_bandwidth_burst_detection() {
        let mut detector = BandwidthSaturationDetector::new(100_000_000); // 100 Mbps
        detector.update(20_000_000, 5_000_000); // 25 Mbps
        assert_eq!(detector.alert_level(20_000_000), SaturationLevel::Normal);

        detector.update(72_000_000, 3_000_000); // 75 Mbps
        assert_eq!(detector.alert_level(70_000_000), SaturationLevel::Elevated);

        detector.update(92_000_000, 3_000_000); // 95 Mbps
        assert_eq!(detector.alert_level(90_000_000), SaturationLevel::Saturated);

        // Burst from baseline 10 Mbps to 80 Mbps (> 2.5x)
        detector.update(75_000_000, 5_000_000);
        assert_eq!(detector.alert_level(10_000_000), SaturationLevel::Bursting);
    }
}
