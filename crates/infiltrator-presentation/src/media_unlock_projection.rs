//! Streaming media and modern AI unlock badge projections for UI surfaces (FLPARITY-020).

use infiltrator_contract::media_unlock::{
    NodeUnlockReport, ServiceUnlockResult, UnlockServiceKind,
};

/// User-facing badge projection for an individual unlock service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnlockBadgeProjection {
    /// Canonical service ID (e.g. "netflix", "openai").
    pub service_id: &'static str,
    /// Service brand display name (e.g. "Netflix", "ChatGPT").
    pub display_name: &'static str,
    /// Compact badge label (e.g. "NF: US", "AI: OK", "D+: ×").
    pub badge_text: String,
    /// Semantic color role token ("success", "accent", "warning", "danger", "dim").
    pub color_role: &'static str,
    /// Detailed tooltip text explaining the unlock status.
    pub tooltip: String,
    /// Whether the service is unlocked.
    pub is_unlocked: bool,
}

impl UnlockBadgeProjection {
    pub fn from_result(result: &ServiceUnlockResult) -> Self {
        let is_unlocked = result.status.is_unlocked();
        let region_suffix = match &result.region_code {
            Some(reg) => format!(" ({reg})"),
            None => String::new(),
        };
        let latency_suffix = match result.latency_ms {
            Some(ms) => format!(" - {ms}ms"),
            None => String::new(),
        };
        let tooltip = format!(
            "{}: {}{}{}",
            result.service.display_name(),
            result.status.label_zh(),
            region_suffix,
            latency_suffix
        );

        Self {
            service_id: result.service.id(),
            display_name: result.service.display_name(),
            badge_text: result.badge_text(),
            color_role: result.status.badge_color_role(),
            tooltip,
            is_unlocked,
        }
    }
}

/// Project all service unlock badges for a node report.
pub fn project_unlock_badges(report: &NodeUnlockReport) -> Vec<UnlockBadgeProjection> {
    report
        .services
        .iter()
        .map(UnlockBadgeProjection::from_result)
        .collect()
}

/// One-line compact summary of node unlock capabilities (e.g. "4/6 解锁: NF(US), AI, YT").
pub fn summarize_node_unlock(report: &NodeUnlockReport) -> String {
    let unlocked: Vec<String> = report
        .services
        .iter()
        .filter(|s| s.status.is_unlocked())
        .map(|s| match (&s.service, &s.region_code) {
            (UnlockServiceKind::Netflix, Some(r)) => format!("NF({r})"),
            (UnlockServiceKind::Netflix, None) => "NF".to_owned(),
            (UnlockServiceKind::OpenAi, _) => "ChatGPT".to_owned(),
            (UnlockServiceKind::Claude, _) => "Claude".to_owned(),
            (UnlockServiceKind::YouTube, Some(r)) => format!("YT({r})"),
            (UnlockServiceKind::YouTube, None) => "YT".to_owned(),
            (UnlockServiceKind::DisneyPlus, _) => "Disney+".to_owned(),
            (UnlockServiceKind::Spotify, _) => "Spotify".to_owned(),
        })
        .collect();

    if unlocked.is_empty() {
        "全阻断 / 未解锁".to_owned()
    } else {
        format!(
            "{}/{} 解锁: {}",
            report.unlocked_count(),
            report.services.len(),
            unlocked.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::media_unlock::UnlockStatus;

    #[test]
    fn badge_projection_formatting_and_colors() {
        let res = ServiceUnlockResult::new(UnlockServiceKind::Netflix, UnlockStatus::Native)
            .with_region("US")
            .with_latency(88);
        let badge = UnlockBadgeProjection::from_result(&res);
        assert_eq!(badge.service_id, "netflix");
        assert_eq!(badge.display_name, "Netflix");
        assert_eq!(badge.badge_text, "NF: US");
        assert_eq!(badge.color_role, "success");
        assert!(badge.is_unlocked);
        assert!(badge.tooltip.contains("原生解锁 (US) - 88ms"));
    }

    #[test]
    fn summary_formatting() {
        let mut report = NodeUnlockReport::new("Node-US", "2026-10-29T12:00:00Z");
        report.services.push(
            ServiceUnlockResult::new(UnlockServiceKind::Netflix, UnlockStatus::Native)
                .with_region("US"),
        );
        report.services.push(ServiceUnlockResult::new(
            UnlockServiceKind::OpenAi,
            UnlockStatus::Native,
        ));
        report.services.push(ServiceUnlockResult::new(
            UnlockServiceKind::Claude,
            UnlockStatus::Blocked,
        ));

        let summary = summarize_node_unlock(&report);
        assert!(summary.starts_with("2/3 解锁:"));
        assert!(summary.contains("NF(US)"));
        assert!(summary.contains("ChatGPT"));
        assert!(!summary.contains("Claude"));
    }
}
