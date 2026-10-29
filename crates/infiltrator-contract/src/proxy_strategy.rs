//! Proxy strategy group inspection, ping stages, and node latency jitter history (FLPARITY-030).
//!
//! Provides strongly-typed models for:
//! * `PingStage` (Idle / Pinging / Success / Timeout / Failed) with pulse animations;
//! * `StrategyDetail` (auto-selected node, tolerance, health check URL, countdown);
//! * `NodeLatencyHistory` (recent latency samples, jitter computation, sparkline normalization).

use crate::proxies::ProxyGroupClassification;
use serde::{Deserialize, Serialize};

/// Interactive state of a node or group ping operation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PingStage {
    /// No test in progress.
    Idle,
    /// Probe request is currently in-flight (triggers radar rotation / breathing pulse).
    Pinging { started_at_secs: u64 },
    /// Successfully measured round-trip delay.
    Success { delay_ms: u32 },
    /// Probe timed out without response.
    Timeout,
    /// Probe encountered network error or refusal.
    Failed { reason: String },
}

impl Default for PingStage {
    fn default() -> Self {
        Self::Idle
    }
}

impl PingStage {
    pub const fn is_pinging(&self) -> bool {
        matches!(self, Self::Pinging { .. })
    }

    pub const fn delay_ms(&self) -> Option<u32> {
        match self {
            Self::Success { delay_ms } => Some(*delay_ms),
            _ => None,
        }
    }

    /// User-facing short status label in Chinese.
    pub fn label_zh(&self) -> String {
        match self {
            Self::Idle => "未测速".to_owned(),
            Self::Pinging { .. } => "测速中…".to_owned(),
            Self::Success { delay_ms } => format!("{delay_ms} ms"),
            Self::Timeout => "超时".to_owned(),
            Self::Failed { .. } => "失败".to_owned(),
        }
    }

    /// Semantic color role token for badges / indicators.
    pub fn color_role(&self) -> &'static str {
        match self {
            Self::Idle => "dim",
            Self::Pinging { .. } => "accent",
            Self::Success { delay_ms } => {
                if *delay_ms < 200 {
                    "success"
                } else if *delay_ms < 500 {
                    "warning"
                } else {
                    "danger"
                }
            }
            Self::Timeout | Self::Failed { .. } => "danger",
        }
    }
}

/// Inspection metadata for proxy group selection strategies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyDetail {
    /// Classification of the strategy group.
    pub classification: ProxyGroupClassification,
    /// Currently elected winning node, if determined.
    pub selected_node: Option<String>,
    /// Health check benchmark endpoint URL.
    pub health_check_url: Option<String>,
    /// Scheduled health check interval in seconds.
    pub interval_secs: u32,
    /// Tolerance delta in milliseconds (e.g. 50ms) for url-test groups.
    pub tolerance_ms: Option<u32>,
    /// Unix timestamp of the last executed health check.
    pub last_checked_at_secs: Option<u64>,
    /// Estimated seconds remaining until next scheduled probe.
    pub next_check_in_secs: Option<u32>,
}

impl StrategyDetail {
    pub fn selector(selected: Option<String>) -> Self {
        Self {
            classification: ProxyGroupClassification::Selector,
            selected_node: selected,
            health_check_url: None,
            interval_secs: 0,
            tolerance_ms: None,
            last_checked_at_secs: None,
            next_check_in_secs: None,
        }
    }

    pub fn url_test(
        selected: Option<String>,
        health_check_url: impl Into<String>,
        interval_secs: u32,
        tolerance_ms: u32,
    ) -> Self {
        Self {
            classification: ProxyGroupClassification::UrlTest,
            selected_node: selected,
            health_check_url: Some(health_check_url.into()),
            interval_secs,
            tolerance_ms: Some(tolerance_ms),
            last_checked_at_secs: None,
            next_check_in_secs: Some(interval_secs),
        }
    }

    pub fn fallback(
        selected: Option<String>,
        health_check_url: impl Into<String>,
        interval_secs: u32,
    ) -> Self {
        Self {
            classification: ProxyGroupClassification::Fallback,
            selected_node: selected,
            health_check_url: Some(health_check_url.into()),
            interval_secs,
            tolerance_ms: None,
            last_checked_at_secs: None,
            next_check_in_secs: Some(interval_secs),
        }
    }

    /// User-facing summary explaining strategy behavior and countdown.
    pub fn summary_zh(&self) -> String {
        match self.classification {
            ProxyGroupClassification::Selector => {
                let node = self.selected_node.as_deref().unwrap_or("未选定");
                format!("手动选择 · 当前: {node}")
            }
            ProxyGroupClassification::UrlTest => {
                let tol = self.tolerance_ms.unwrap_or(50);
                let next = self
                    .next_check_in_secs
                    .map_or("—".to_owned(), |s| format!("{s}s"));
                format!("自动选优 (公差 {tol}ms) · 下次检测: {next}")
            }
            ProxyGroupClassification::Fallback => {
                let next = self
                    .next_check_in_secs
                    .map_or("—".to_owned(), |s| format!("{s}s"));
                format!("故障回退 (顺序保活) · 下次检测: {next}")
            }
            ProxyGroupClassification::LoadBalance => "负载均衡 (散列轮询)".to_owned(),
            ProxyGroupClassification::Relay => "多跳链式中继".to_owned(),
        }
    }
}

/// One latency observation sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatencyJitterSample {
    pub delay_ms: u32,
    pub tested_at_secs: u64,
}

/// Recent latency history and jitter computation for an individual proxy node.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeLatencyHistory {
    /// Ring buffer of recent latency samples (typically last 5~10 tests).
    pub samples: Vec<LatencyJitterSample>,
    /// Maximum capacity of samples retained.
    pub capacity: usize,
}

impl NodeLatencyHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity.max(5)),
            capacity: capacity.max(5),
        }
    }

    pub fn push(&mut self, delay_ms: u32, tested_at_secs: u64) {
        if self.samples.len() >= self.capacity {
            self.samples.remove(0);
        }
        self.samples.push(LatencyJitterSample {
            delay_ms,
            tested_at_secs,
        });
    }

    pub fn min_delay(&self) -> Option<u32> {
        self.samples.iter().map(|s| s.delay_ms).min()
    }

    pub fn max_delay(&self) -> Option<u32> {
        self.samples.iter().map(|s| s.delay_ms).max()
    }

    pub fn average_delay(&self) -> Option<u32> {
        if self.samples.is_empty() {
            return None;
        }
        let sum: u64 = self.samples.iter().map(|s| s.delay_ms as u64).sum();
        Some((sum / self.samples.len() as u64) as u32)
    }

    /// Latency jitter (max - min delta) in milliseconds.
    pub fn jitter_ms(&self) -> Option<u32> {
        let (min, max) = (self.min_delay()?, self.max_delay()?);
        Some(max.saturating_sub(min))
    }

    /// Normalized sample values in `[0.1, 1.0]` for rendering a mini sparkline chart.
    pub fn normalized_bars(&self) -> Vec<f32> {
        if self.samples.is_empty() {
            return Vec::new();
        }
        let max = self.max_delay().unwrap_or(1).max(1) as f32;
        self.samples
            .iter()
            .map(|s| (s.delay_ms as f32 / max).clamp(0.1, 1.0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_stage_lifecycle_and_labels() {
        let idle = PingStage::Idle;
        assert!(!idle.is_pinging());
        assert_eq!(idle.label_zh(), "未测速");
        assert_eq!(idle.color_role(), "dim");

        let pinging = PingStage::Pinging {
            started_at_secs: 100,
        };
        assert!(pinging.is_pinging());
        assert_eq!(pinging.label_zh(), "测速中…");
        assert_eq!(pinging.color_role(), "accent");

        let ok_fast = PingStage::Success { delay_ms: 88 };
        assert_eq!(ok_fast.delay_ms(), Some(88));
        assert_eq!(ok_fast.label_zh(), "88 ms");
        assert_eq!(ok_fast.color_role(), "success");

        let ok_slow = PingStage::Success { delay_ms: 650 };
        assert_eq!(ok_slow.color_role(), "danger");

        let timeout = PingStage::Timeout;
        assert_eq!(timeout.label_zh(), "超时");
        assert_eq!(timeout.color_role(), "danger");
    }

    #[test]
    fn strategy_detail_summary_and_fields() {
        let url_test = StrategyDetail::url_test(
            Some("JP-Tokyo-01".to_string()),
            "http://www.gstatic.com/generate_204",
            300,
            50,
        );
        assert_eq!(url_test.classification, ProxyGroupClassification::UrlTest);
        let summary = url_test.summary_zh();
        assert!(summary.contains("自动选优"));
        assert!(summary.contains("50ms"));
        assert!(summary.contains("300s"));

        let selector = StrategyDetail::selector(Some("US-Node".to_string()));
        assert!(selector.summary_zh().contains("US-Node"));
    }

    #[test]
    fn latency_history_and_jitter_calculation() {
        let mut history = NodeLatencyHistory::new(5);
        history.push(50, 1000);
        history.push(70, 1005);
        history.push(60, 1010);

        assert_eq!(history.min_delay(), Some(50));
        assert_eq!(history.max_delay(), Some(70));
        assert_eq!(history.average_delay(), Some(60));
        assert_eq!(history.jitter_ms(), Some(20));

        let bars = history.normalized_bars();
        assert_eq!(bars.len(), 3);
        assert!((bars[1] - 1.0).abs() < 1e-4); // max sample is 1.0
    }
}
