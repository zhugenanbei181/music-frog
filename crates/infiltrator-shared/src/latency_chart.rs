//! Latency analytics, statistics calculation, and GPU/SVG waveform generation.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LatencyPoint {
    pub timestamp_secs: u64,
    pub delay_ms: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LatencySeries {
    pub node_name: String,
    pub points: Vec<LatencyPoint>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LatencyStats {
    pub min: u32,
    pub max: u32,
    pub avg: f64,
    pub p50: u32,
    pub p95: u32,
    pub p99: u32,
}

pub fn calculate_stats(points: &[LatencyPoint]) -> Option<LatencyStats> {
    if points.is_empty() {
        return None;
    }

    let mut delays: Vec<u32> = points.iter().map(|p| p.delay_ms).collect();
    delays.sort_unstable();

    let n = delays.len();
    let min = delays[0];
    let max = delays[n - 1];
    let sum: u64 = delays.iter().map(|&d| d as u64).sum();
    let avg = sum as f64 / n as f64;

    let p50 = percentile(&delays, 0.50);
    let p95 = percentile(&delays, 0.95);
    let p99 = percentile(&delays, 0.99);

    Some(LatencyStats {
        min,
        max,
        avg,
        p50,
        p95,
        p99,
    })
}

fn percentile(sorted_data: &[u32], p: f64) -> u32 {
    let n = sorted_data.len();
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return sorted_data[0];
    }

    let index = (p * (n as f64 - 1.0)).round() as usize;
    sorted_data[index]
}

pub fn generate_svg_sparkline(points: &[LatencyPoint], width: u32, height: u32) -> String {
    if points.is_empty() {
        return format!(
            "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\"><path d=\"\" fill=\"none\" stroke=\"currentColor\"/></svg>",
            width, height
        );
    }

    let min_ts = points.first().unwrap().timestamp_secs;
    let max_ts = points.last().unwrap().timestamp_secs;
    let ts_range = max_ts.saturating_sub(min_ts);

    let max_delay = points.iter().map(|p| p.delay_ms).max().unwrap_or(0);

    let mut path_data = String::new();

    for (i, point) in points.iter().enumerate() {
        let x = if ts_range == 0 {
            width / 2
        } else {
            let x_ratio = (point.timestamp_secs - min_ts) as f64 / ts_range as f64;
            (x_ratio * width as f64).round() as u32
        };

        let y = if max_delay == 0 {
            height
        } else {
            let y_ratio = point.delay_ms as f64 / max_delay as f64;
            height.saturating_sub((y_ratio * height as f64).round() as u32)
        };

        if i == 0 {
            path_data.push_str(&format!("M {} {}", x, y));
        } else {
            path_data.push_str(&format!(" L {} {}", x, y));
        }
    }

    format!(
        "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\"><path d=\"{}\" fill=\"none\" stroke=\"currentColor\"/></svg>",
        width, height, path_data
    )
}

/// Dual-channel (upload and download) throughput waveform generator for real-time traffic charts.
pub struct ThroughputWaveformGenerator;

impl ThroughputWaveformGenerator {
    pub fn generate_dual_waveform_svg(
        up_samples: &[u64],
        down_samples: &[u64],
        width: u32,
        height: u32,
    ) -> String {
        let up_path = Self::build_channel_path(up_samples, width, height);
        let down_path = Self::build_channel_path(down_samples, width, height);

        format!(
            "<svg width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" xmlns=\"http://www.w3.org/2000/svg\">\
            <path d=\"{up_path}\" fill=\"none\" stroke=\"#10b981\" stroke-width=\"2\" stroke-linecap=\"round\"/>\
            <path d=\"{down_path}\" fill=\"none\" stroke=\"#3b82f6\" stroke-width=\"2\" stroke-linecap=\"round\"/>\
            </svg>"
        )
    }

    fn build_channel_path(samples: &[u64], width: u32, height: u32) -> String {
        if samples.is_empty() {
            return String::new();
        }

        let max_val = *samples.iter().max().unwrap_or(&1).max(&1);
        let count = samples.len();
        let step = if count > 1 {
            width as f64 / (count - 1) as f64
        } else {
            width as f64
        };

        let mut path = String::new();
        for (i, &val) in samples.iter().enumerate() {
            let x = (i as f64 * step).round() as u32;
            let y_ratio = val as f64 / max_val as f64;
            let y = height.saturating_sub((y_ratio * (height as f64 * 0.9)).round() as u32);

            if i == 0 {
                path.push_str(&format!("M {x} {y}"));
            } else {
                path.push_str(&format!(" L {x} {y}"));
            }
        }
        path
    }
}

pub fn filter_by_window(
    points: &[LatencyPoint],
    window_secs: u64,
    now_secs: u64,
) -> Vec<LatencyPoint> {
    let cutoff = now_secs.saturating_sub(window_secs);
    points
        .iter()
        .filter(|p| p.timestamp_secs >= cutoff)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_stats_empty() {
        assert!(calculate_stats(&[]).is_none());
    }

    #[test]
    fn test_calculate_stats_single() {
        let points = vec![LatencyPoint {
            timestamp_secs: 100,
            delay_ms: 42,
        }];
        let stats = calculate_stats(&points).unwrap();
        assert_eq!(stats.min, 42);
        assert_eq!(stats.max, 42);
        assert_eq!(stats.avg, 42.0);
        assert_eq!(stats.p50, 42);
        assert_eq!(stats.p95, 42);
        assert_eq!(stats.p99, 42);
    }

    #[test]
    fn test_calculate_stats_multiple() {
        let points: Vec<LatencyPoint> = (1..=100)
            .map(|i| LatencyPoint {
                timestamp_secs: i as u64,
                delay_ms: i as u32,
            })
            .collect();

        let stats = calculate_stats(&points).unwrap();
        assert_eq!(stats.min, 1);
        assert_eq!(stats.max, 100);
        assert_eq!(stats.avg, 50.5);
        assert_eq!(stats.p50, 51);
        assert_eq!(stats.p95, 95);
        assert_eq!(stats.p99, 99);
    }

    #[test]
    fn test_generate_svg_sparkline_empty() {
        let svg = generate_svg_sparkline(&[], 100, 50);
        assert!(svg.contains("<svg width=\"100\" height=\"50\""));
        assert!(svg.contains("d=\"\""));
    }

    #[test]
    fn test_generate_svg_sparkline() {
        let points = vec![
            LatencyPoint {
                timestamp_secs: 10,
                delay_ms: 10,
            },
            LatencyPoint {
                timestamp_secs: 20,
                delay_ms: 20,
            },
            LatencyPoint {
                timestamp_secs: 30,
                delay_ms: 0,
            },
        ];
        let svg = generate_svg_sparkline(&points, 100, 50);
        assert!(svg.contains("<svg width=\"100\" height=\"50\""));
        assert!(svg.contains("path d=\"M 0 25 L 50 0 L 100 50\""));
    }

    #[test]
    fn test_dual_waveform_svg() {
        let up = vec![1000, 2000, 1500, 3000, 2500];
        let down = vec![5000, 8000, 12000, 15000, 11000];
        let svg = ThroughputWaveformGenerator::generate_dual_waveform_svg(&up, &down, 200, 60);
        assert!(svg.contains("<svg width=\"200\" height=\"60\""));
        assert!(svg.contains("#10b981"));
        assert!(svg.contains("#3b82f6"));
    }

    #[test]
    fn test_filter_by_window() {
        let points = vec![
            LatencyPoint {
                timestamp_secs: 10,
                delay_ms: 10,
            },
            LatencyPoint {
                timestamp_secs: 20,
                delay_ms: 20,
            },
            LatencyPoint {
                timestamp_secs: 30,
                delay_ms: 0,
            },
            LatencyPoint {
                timestamp_secs: 40,
                delay_ms: 0,
            },
        ];
        let filtered = filter_by_window(&points, 15, 40);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].timestamp_secs, 30);
        assert_eq!(filtered[1].timestamp_secs, 40);
    }
}
