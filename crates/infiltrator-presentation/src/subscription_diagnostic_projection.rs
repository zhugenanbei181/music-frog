//! Presentation read-model projection for subscription fetch failure diagnostics and node regex wizard (FLPARITY-033).

use infiltrator_contract::subscription_diagnostic::{
    NodeRegexTransformPipeline, SubscriptionFetchDiagnostic,
};
use infiltrator_domain::subscription_regex::{CompiledNodeRegexPipeline, NodeRegexPreviewItem};

/// Metric item displayed in the diagnostic details grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticMetricItem {
    pub label: String,
    pub value: String,
}

/// Presentation model for the subscription failure diagnostic dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionDiagnosticDialogProjection {
    pub target_url: String,
    pub title: String,
    pub recommendation: String,
    pub status_badge: Option<String>,
    pub metrics: Vec<DiagnosticMetricItem>,
    pub markdown_report: String,
    pub raw_snippet: String,
}

impl SubscriptionDiagnosticDialogProjection {
    pub fn of(diag: &SubscriptionFetchDiagnostic) -> Self {
        let mut metrics = Vec::new();
        if let Some(status) = diag.http_status {
            metrics.push(DiagnosticMetricItem {
                label: "HTTP 状态码".to_owned(),
                value: status.to_string(),
            });
        }
        if let Some(ms) = diag.dns_latency_ms {
            metrics.push(DiagnosticMetricItem {
                label: "DNS 解析延迟".to_owned(),
                value: format!("{ms} ms"),
            });
        }
        if let Some(ms) = diag.connect_latency_ms {
            metrics.push(DiagnosticMetricItem {
                label: "TCP 握手延迟".to_owned(),
                value: format!("{ms} ms"),
            });
        }
        if let Some(tls) = &diag.tls_version {
            metrics.push(DiagnosticMetricItem {
                label: "TLS 协议版本".to_owned(),
                value: tls.clone(),
            });
        }
        if let Some(server) = &diag.server_banner {
            metrics.push(DiagnosticMetricItem {
                label: "Server 服务端".to_owned(),
                value: server.clone(),
            });
        }

        let status_badge = diag.http_status.map(|s| format!("HTTP {s}"));

        Self {
            target_url: diag.target_url_redacted.clone(),
            title: diag.category.title_zh(),
            recommendation: diag.category.recommendation_zh().to_owned(),
            status_badge,
            metrics,
            markdown_report: diag.to_markdown_report(),
            raw_snippet: diag.raw_error_snippet.clone(),
        }
    }
}

/// A preview item projected for UI consumption in the regex wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRegexPreviewProjection {
    pub original: String,
    pub transformed: Option<String>,
    pub is_excluded: bool,
    pub is_modified: bool,
    pub badge_label: &'static str,
}

impl From<NodeRegexPreviewItem> for NodeRegexPreviewProjection {
    fn from(item: NodeRegexPreviewItem) -> Self {
        let badge_label = if item.is_excluded {
            "已剔除"
        } else if item.is_modified {
            "已重命名"
        } else {
            "无改动"
        };

        Self {
            original: item.original,
            transformed: item.result,
            is_excluded: item.is_excluded,
            is_modified: item.is_modified,
            badge_label,
        }
    }
}

/// Presentation projection for the node regex cleaning wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRegexWizardProjection {
    pub rules_count: usize,
    pub is_valid: bool,
    pub previews: Vec<NodeRegexPreviewProjection>,
    pub summary_text: String,
}

impl NodeRegexWizardProjection {
    pub fn of(pipeline: &NodeRegexTransformPipeline, sample_nodes: &[&str]) -> Self {
        let compiled = CompiledNodeRegexPipeline::compile(pipeline);
        let is_valid = compiled.is_all_valid();
        let previews_domain = compiled.preview(sample_nodes);
        let previews: Vec<NodeRegexPreviewProjection> =
            previews_domain.into_iter().map(Into::into).collect();

        let excluded_count = previews.iter().filter(|p| p.is_excluded).count();
        let modified_count = previews.iter().filter(|p| p.is_modified).count();
        let kept_count = previews.len() - excluded_count;

        let summary_text = format!(
            "样本 {} 个：保留 {} 个（其中 {} 个重命名），剔除 {} 个",
            previews.len(),
            kept_count,
            modified_count,
            excluded_count
        );

        Self {
            rules_count: pipeline.rules.len(),
            is_valid,
            previews,
            summary_text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::subscription_diagnostic::SubscriptionDiagnosticCategory;

    #[test]
    fn diagnostic_dialog_projection_formats_correctly() {
        let diag = SubscriptionFetchDiagnostic {
            target_url_redacted: "https://sub.example.com/api?token=***".to_owned(),
            category: SubscriptionDiagnosticCategory::HttpError(502),
            http_status: Some(502),
            dns_latency_ms: Some(12),
            connect_latency_ms: Some(85),
            tls_version: Some("TLSv1.3".to_owned()),
            server_banner: Some("nginx/1.24".to_owned()),
            raw_error_snippet: "502 Bad Gateway".to_owned(),
        };

        let proj = SubscriptionDiagnosticDialogProjection::of(&diag);
        assert_eq!(proj.title, "HTTP 状态异常 (502)");
        assert_eq!(proj.status_badge.as_deref(), Some("HTTP 502"));
        assert!(
            proj.metrics
                .iter()
                .any(|m| m.label == "HTTP 状态码" && m.value == "502")
        );
        assert!(
            proj.metrics
                .iter()
                .any(|m| m.label == "DNS 解析延迟" && m.value == "12 ms")
        );
        assert!(proj.markdown_report.contains("502"));
    }

    #[test]
    fn regex_wizard_projection_computes_preview_and_summary() {
        let rules = vec![
            CompiledNodeRegexPipeline::preset_strip_provider_tags(),
            CompiledNodeRegexPipeline::preset_exclude_announcements(),
        ];
        let pipeline = NodeRegexTransformPipeline::new(rules);
        let samples = ["[Airport] 香港 01", "官网通知: 维护中", "日本 02"];

        let proj = NodeRegexWizardProjection::of(&pipeline, &samples);
        assert!(proj.is_valid);
        assert_eq!(proj.rules_count, 2);
        assert_eq!(proj.previews.len(), 3);
        assert_eq!(proj.previews[0].badge_label, "已重命名");
        assert_eq!(proj.previews[1].badge_label, "已剔除");
        assert_eq!(proj.previews[2].badge_label, "无改动");
        assert_eq!(
            proj.summary_text,
            "样本 3 个：保留 2 个（其中 1 个重命名），剔除 1 个"
        );
    }
}
