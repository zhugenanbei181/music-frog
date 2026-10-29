//! Proxy strategy group and node ping pulse presentation projections (FLPARITY-030).

use infiltrator_contract::proxies::ProxyGroupClassification;
use infiltrator_contract::proxy_strategy::{NodeLatencyHistory, PingStage, StrategyDetail};

/// Visual projection for ping pulse indicator on node cards.
#[derive(Clone, Debug, PartialEq)]
pub struct PingPulseProjection {
    /// Rendered text label (e.g. "45 ms", "测速中…", "超时").
    pub label: String,
    /// Semantic token ("success", "warning", "danger", "accent", "dim").
    pub color_role: &'static str,
    /// Whether breathing pulse or rotating radar animation should be active.
    pub is_animating: bool,
    /// Animation cycle frequency in Hz (default 1.5 Hz during pinging).
    pub pulse_frequency_hz: f32,
}

impl PingPulseProjection {
    pub fn of(stage: &PingStage) -> Self {
        let is_animating = stage.is_pinging();
        let pulse_frequency_hz = if is_animating { 1.5 } else { 0.0 };
        Self {
            label: stage.label_zh(),
            color_role: stage.color_role(),
            is_animating,
            pulse_frequency_hz,
        }
    }
}

/// Visual projection for proxy group strategy header card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrategyCardProjection {
    /// Strategy group classification label.
    pub strategy_name: &'static str,
    /// Detailed description and countdown summary.
    pub summary: String,
    /// Currently elected winning node name.
    pub selected_node: Option<String>,
    /// Countdown to next scheduled benchmark, if configured.
    pub countdown_label: Option<String>,
    /// Whether active health check URL is present.
    pub has_health_check: bool,
}

impl StrategyCardProjection {
    pub fn of(detail: &StrategyDetail) -> Self {
        let strategy_name = match detail.classification {
            ProxyGroupClassification::Selector => "手动选择",
            ProxyGroupClassification::UrlTest => "自动测速选优",
            ProxyGroupClassification::Fallback => "自动故障转移",
            ProxyGroupClassification::LoadBalance => "负载均衡",
            ProxyGroupClassification::Relay => "链式中继",
        };

        let countdown_label = detail
            .next_check_in_secs
            .map(|secs| format!("{secs}s 后重测"));

        Self {
            strategy_name,
            summary: detail.summary_zh(),
            selected_node: detail.selected_node.clone(),
            countdown_label,
            has_health_check: detail.health_check_url.is_some(),
        }
    }
}

/// Format latency jitter delta (e.g. "±12ms") from recent history.
pub fn format_jitter_badge(history: &NodeLatencyHistory) -> Option<String> {
    let jitter = history.jitter_ms()?;
    Some(format!("±{jitter}ms"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_pulse_projection_states() {
        let pinging = PingStage::Pinging {
            started_at_secs: 10,
        };
        let pulse = PingPulseProjection::of(&pinging);
        assert!(pulse.is_animating);
        assert_eq!(pulse.color_role, "accent");
        assert_eq!(pulse.label, "测速中…");

        let ok = PingStage::Success { delay_ms: 120 };
        let ok_pulse = PingPulseProjection::of(&ok);
        assert!(!ok_pulse.is_animating);
        assert_eq!(ok_pulse.color_role, "success");
        assert_eq!(ok_pulse.label, "120 ms");
    }

    #[test]
    fn strategy_card_projection_formatting() {
        let detail = StrategyDetail::url_test(
            Some("JP-01".to_string()),
            "http://www.gstatic.com/generate_204",
            300,
            50,
        );
        let proj = StrategyCardProjection::of(&detail);
        assert_eq!(proj.strategy_name, "自动测速选优");
        assert!(proj.has_health_check);
        assert_eq!(proj.countdown_label.as_deref(), Some("300s 后重测"));
        assert_eq!(proj.selected_node.as_deref(), Some("JP-01"));
    }

    #[test]
    fn jitter_badge_formatting() {
        let mut history = NodeLatencyHistory::new(5);
        assert_eq!(format_jitter_badge(&history), None);

        history.push(100, 1);
        history.push(115, 2);
        assert_eq!(format_jitter_badge(&history), Some("±15ms".to_string()));
    }
}
