//! DNS query history list row and summary projections for UI surfaces (FLPARITY-004).

use chrono::{DateTime, Utc};
use infiltrator_contract::dns_query_history::{
    DnsHistoryRecordingState, DnsQueryHistoryFilter, DnsQueryHistoryRecord, DnsQueryHistorySnapshot,
};

/// User-facing projection for a single DNS query history table row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsQueryRowProjection {
    /// Record identifier.
    pub id: u64,
    /// Formatted time string (e.g. "12:00:01").
    pub time_text: String,
    /// Queried domain name.
    pub domain: String,
    /// Record type label (e.g. "A", "AAAA", "HTTPS").
    pub type_label: &'static str,
    /// Localized status badge text (e.g. "解析成功", "命中缓存", "域名不存在").
    pub status_label: &'static str,
    /// Semantic color role for status badge.
    pub status_role: &'static str,
    /// Formatted latency text (e.g. "18ms").
    pub latency_text: String,
    /// Joined answer summary (e.g. "140.82.121.4" or "2 records").
    pub answer_summary: String,
    /// Upstream server text.
    pub server_text: String,
    /// Detailed tooltip summarizing all record facts.
    pub tooltip: String,
}

impl DnsQueryRowProjection {
    pub fn from_record(record: &DnsQueryHistoryRecord) -> Self {
        let time_text = i64::try_from(record.timestamp_epoch_ms)
            .ok()
            .and_then(DateTime::<Utc>::from_timestamp_millis)
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "--:--:--".to_owned());

        let latency_text = format!("{}ms", record.latency_ms);

        let answer_summary = match record.answers.len() {
            0 => "--".to_owned(),
            1 => record.answers[0].clone(),
            n => format!("{} 等 {} 条记录", record.answers[0], n),
        };

        let server_text = record
            .server
            .clone()
            .unwrap_or_else(|| "本地缓存/默认".to_owned());

        let mut tooltip_parts = vec![
            format!("域名: {}", record.name),
            format!("类型: {}", record.record_type.wire()),
            format!("状态: {}", record.status.label_zh()),
            format!("耗时: {}ms", record.latency_ms),
        ];
        if !record.answers.is_empty() {
            tooltip_parts.push(format!("解析结果: {}", record.answers.join(", ")));
        }
        if let Some(srv) = &record.server {
            tooltip_parts.push(format!("上游服务器: {srv}"));
        }

        Self {
            id: record.id,
            time_text,
            domain: record.name.clone(),
            type_label: record.record_type.wire(),
            status_label: record.status.label_zh(),
            status_role: record.status.badge_color_role(),
            latency_text,
            answer_summary,
            server_text,
            tooltip: tooltip_parts.join(" · "),
        }
    }
}

/// Filtered and projected view model for the DNS query history list page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsQueryHistoryPageProjection {
    pub rows: Vec<DnsQueryRowProjection>,
    pub total_count: usize,
    pub filtered_count: usize,
    pub recording_state: DnsHistoryRecordingState,
    pub summary_text: String,
}

impl DnsQueryHistoryPageProjection {
    pub fn is_paused(&self) -> bool {
        self.recording_state.is_paused()
    }
}

/// Project the DNS query history page model given a snapshot and active filters.
pub fn project_dns_history_page(
    snapshot: &DnsQueryHistorySnapshot,
    filter: &DnsQueryHistoryFilter,
) -> DnsQueryHistoryPageProjection {
    let filtered: Vec<&DnsQueryHistoryRecord> = snapshot.filter(filter);
    let rows: Vec<DnsQueryRowProjection> = filtered
        .iter()
        .rev() // Newest records first
        .map(|r| DnsQueryRowProjection::from_record(r))
        .collect();

    let total_count = snapshot.records.len();
    let filtered_count = rows.len();
    let paused_suffix = if snapshot.is_recording_paused() {
        " (已暂停记录)"
    } else {
        ""
    };
    let summary_text = format!(
        "共记录 {} 条，当前显示 {} 条{}",
        total_count, filtered_count, paused_suffix
    );

    DnsQueryHistoryPageProjection {
        rows,
        total_count,
        filtered_count,
        recording_state: snapshot.recording_state,
        summary_text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::dns_query::DnsRecordType;
    use infiltrator_contract::dns_query_history::{DnsQueryStatus, DnsQueryStatusFilter};

    #[test]
    fn row_projection_formatting() {
        let rec = DnsQueryHistoryRecord::new(
            42,
            1741300000000,
            "api.openai.com",
            DnsRecordType::A,
            DnsQueryStatus::Success,
            25,
        )
        .with_answers(vec!["104.18.3.161".to_owned(), "104.18.2.161".to_owned()])
        .with_server("tls://1.1.1.1");

        let proj = DnsQueryRowProjection::from_record(&rec);
        assert_eq!(proj.id, 42);
        assert_eq!(proj.domain, "api.openai.com");
        assert_eq!(proj.type_label, "A");
        assert_eq!(proj.status_label, "解析成功");
        assert_eq!(proj.status_role, "success");
        assert_eq!(proj.latency_text, "25ms");
        assert_eq!(proj.answer_summary, "104.18.3.161 等 2 条记录");
        assert_eq!(proj.server_text, "tls://1.1.1.1");
        assert!(proj.tooltip.contains("上游服务器: tls://1.1.1.1"));
    }

    #[test]
    fn page_projection_filtering_and_ordering() {
        let snap = DnsQueryHistorySnapshot::demo_fixture();
        let filter = DnsQueryHistoryFilter::default();
        let page = project_dns_history_page(&snap, &filter);

        assert_eq!(page.total_count, 3);
        assert_eq!(page.filtered_count, 3);
        // Newest first ordering
        assert_eq!(page.rows[0].domain, "nonexistent-test-host.local");
        assert_eq!(page.rows[2].domain, "api.github.com");
        assert_eq!(page.summary_text, "共记录 3 条，当前显示 3 条");
        assert!(!page.is_paused());

        let err_filter = DnsQueryHistoryFilter {
            status_filter: DnsQueryStatusFilter::ErrorsOnly,
            ..Default::default()
        };
        let err_page = project_dns_history_page(&snap, &err_filter);
        assert_eq!(err_page.filtered_count, 1);
        assert_eq!(err_page.rows[0].domain, "nonexistent-test-host.local");
    }
}
