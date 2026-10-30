//! Request tracker list row and page projections for UI surfaces (FLPARITY-003).

use chrono::{DateTime, Utc};
use infiltrator_contract::request_tracker::{
    RequestHistoryFilter, RequestRecord, RequestTrackerSnapshot, RequestTrackingState,
};
use infiltrator_shared::byte_units::format_bytes;

/// User-facing projection for a single request table row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestRowProjection {
    /// Request identifier.
    pub id: String,
    /// Formatted occurrence time (e.g. "12:00:01").
    pub time_text: String,
    /// Target host or endpoint.
    pub host: String,
    /// Transport protocol label (e.g. "TCP · HTTPS").
    pub network_label: String,
    /// Matched rule display label (e.g. "DomainSuffix(github.com)").
    pub rule_label: String,
    /// Outbound proxy node or chain display label.
    pub outbound_label: String,
    /// Formatted upload traffic (e.g. "1.0 KB").
    pub upload_text: String,
    /// Formatted download traffic (e.g. "8.0 KB").
    pub download_text: String,
    /// Formatted duration text (e.g. "450ms").
    pub duration_text: String,
    /// Client process label.
    pub process_text: String,
    /// Localized status badge text.
    pub status_label: &'static str,
    /// Semantic color role for status badge.
    pub status_role: &'static str,
    /// Detailed tooltip summarizing all request facts.
    pub tooltip: String,
}

impl RequestRowProjection {
    pub fn from_record(record: &RequestRecord) -> Self {
        let time_text = i64::try_from(record.timestamp_epoch_ms)
            .ok()
            .and_then(DateTime::<Utc>::from_timestamp_millis)
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "--:--:--".to_owned());

        let network_label = if record.type_name.is_empty()
            || record
                .type_name
                .eq_ignore_ascii_case(record.network.as_str())
        {
            record.network.as_str().to_owned()
        } else {
            format!("{} · {}", record.network.as_str(), record.type_name)
        };

        let rule_label = if record.rule_payload.is_empty() {
            record.rule.clone()
        } else {
            format!("{}({})", record.rule, record.rule_payload)
        };

        let outbound_label = if record.outbound_chain.is_empty() {
            record.outbound.clone()
        } else {
            record.outbound_chain.join(" → ")
        };

        let upload_text = format_bytes(record.upload_bytes);
        let download_text = format_bytes(record.download_bytes);
        let duration_text = format!("{}ms", record.duration_ms);
        let process_text = record
            .process
            .clone()
            .unwrap_or_else(|| "系统/未知".to_owned());

        let mut tooltip_parts = vec![
            format!("目标: {}", record.host),
            format!("协议: {network_label}"),
            format!("规则: {rule_label}"),
            format!("出站: {outbound_label}"),
            format!("状态: {}", record.status.label_zh()),
            format!("上传/下载: {upload_text} / {download_text}"),
        ];
        if let Some(proc) = &record.process {
            tooltip_parts.push(format!("进程: {proc}"));
        }

        Self {
            id: record.id.clone(),
            time_text,
            host: record.host.clone(),
            network_label,
            rule_label,
            outbound_label,
            upload_text,
            download_text,
            duration_text,
            process_text,
            status_label: record.status.label_zh(),
            status_role: record.status.badge_color_role(),
            tooltip: tooltip_parts.join(" · "),
        }
    }
}

/// Filtered and projected view model for the Requests page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestPageProjection {
    pub rows: Vec<RequestRowProjection>,
    pub total_count: usize,
    pub filtered_count: usize,
    pub active_count: usize,
    pub tracking_state: RequestTrackingState,
    pub summary_text: String,
}

impl RequestPageProjection {
    pub fn is_paused(&self) -> bool {
        self.tracking_state.is_paused()
    }
}

/// Project the Requests page model given a snapshot and active filters.
pub fn project_request_page(
    snapshot: &RequestTrackerSnapshot,
    filter: &RequestHistoryFilter,
) -> RequestPageProjection {
    let filtered: Vec<&RequestRecord> = snapshot.filter(filter);
    let rows: Vec<RequestRowProjection> = filtered
        .iter()
        .rev() // Newest requests first
        .map(|r| RequestRowProjection::from_record(r))
        .collect();

    let total_count = snapshot.records.len();
    let filtered_count = rows.len();
    let paused_suffix = if snapshot.is_tracking_paused() {
        " (已暂停记录)"
    } else {
        ""
    };
    let summary_text = format!(
        "共记录 {} 条（活跃 {}），当前显示 {} 条{}",
        total_count, snapshot.active_count, filtered_count, paused_suffix
    );

    RequestPageProjection {
        rows,
        total_count,
        filtered_count,
        active_count: snapshot.active_count,
        tracking_state: snapshot.tracking_state,
        summary_text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::request_tracker::{
        RequestNetwork, RequestStatus, RequestStatusFilter,
    };

    #[test]
    fn row_projection_formatting() {
        let rec = RequestRecord::new(
            "req-42",
            1741300000000,
            "api.openai.com:443",
            RequestNetwork::Tcp,
            "US-Node-01",
        )
        .with_type_name("HTTPS")
        .with_rule("DomainSuffix", "openai.com")
        .with_chain(vec!["Proxy".to_owned(), "US-Node-01".to_owned()])
        .with_traffic(2048, 16384, 320)
        .with_process("curl", None)
        .with_status(RequestStatus::Closed);

        let proj = RequestRowProjection::from_record(&rec);
        assert_eq!(proj.id, "req-42");
        assert_eq!(proj.host, "api.openai.com:443");
        assert_eq!(proj.network_label, "TCP · HTTPS");
        assert_eq!(proj.rule_label, "DomainSuffix(openai.com)");
        assert_eq!(proj.outbound_label, "Proxy → US-Node-01");
        assert_eq!(proj.duration_text, "320ms");
        assert_eq!(proj.process_text, "curl");
        assert_eq!(proj.status_label, "已关闭");
        assert_eq!(proj.status_role, "dim");
        assert!(proj.tooltip.contains("进程: curl"));
    }

    #[test]
    fn page_projection_filtering_and_ordering() {
        let snap = RequestTrackerSnapshot::demo_fixture();
        let filter = RequestHistoryFilter::default();
        let page = project_request_page(&snap, &filter);

        assert_eq!(page.total_count, 3);
        assert_eq!(page.filtered_count, 3);
        assert_eq!(page.active_count, 1);
        // Newest first ordering
        assert_eq!(page.rows[0].id, "req-3");
        assert_eq!(page.rows[2].id, "req-1");
        assert_eq!(page.summary_text, "共记录 3 条（活跃 1），当前显示 3 条");
        assert!(!page.is_paused());

        let active_filter = RequestHistoryFilter {
            status_filter: RequestStatusFilter::ActiveOnly,
            ..Default::default()
        };
        let active_page = project_request_page(&snap, &active_filter);
        assert_eq!(active_page.filtered_count, 1);
        assert_eq!(active_page.rows[0].id, "req-2");
    }
}
