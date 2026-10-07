//! Shared labels, severity and ordered rows of the actual nameserver report.
use crate::dns_observation_projection::DnsObservationTone;
use crate::latency_projection::project_measured_latency;
use infiltrator_contract::dns::DnsServerTag;
use infiltrator_contract::dns_latency::{DnsLatencyReport, DnsLatencySummary, DnsProbeOutcome};
use infiltrator_contract::latency_display::LatencyBand;
use infiltrator_shared::i18n_interpolator::localize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsLatencyRow {
    pub address: String,
    pub tier: String,
    pub outcome: String,
    pub tone: DnsObservationTone,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsLatencyDisplay {
    pub summary: String,
    pub tone: DnsObservationTone,
    pub rows: Vec<DnsLatencyRow>,
    pub empty: String,
}
impl DnsLatencyDisplay {
    pub fn listing(&self) -> String {
        if self.rows.is_empty() {
            return self.empty.clone();
        }
        self.rows
            .iter()
            .map(|row| format!("{} [{}] {}", row.address, row.tier, row.outcome))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
pub fn project_dns_latency(report: &DnsLatencyReport, code: &str) -> DnsLatencyDisplay {
    let (key, params, tone) = match report.summary() {
        DnsLatencySummary::AllMeasured {
            count,
            best_ms,
            worst_ms,
        } => (
            "dns_latency_all_measured",
            vec![
                ("count", count.to_string()),
                ("best_ms", best_ms.to_string()),
                ("worst_ms", worst_ms.to_string()),
            ],
            DnsObservationTone::Success,
        ),
        DnsLatencySummary::Partial { measured, total } => (
            "dns_latency_partial",
            vec![
                ("measured", measured.to_string()),
                ("total", total.to_string()),
            ],
            DnsObservationTone::Warning,
        ),
        DnsLatencySummary::NoneReachable { total } => (
            "dns_latency_none_reachable",
            vec![("total", total.to_string())],
            DnsObservationTone::Danger,
        ),
        DnsLatencySummary::NotProbed => (
            "dns_latency_not_probed_yet",
            vec![],
            DnsObservationTone::Neutral,
        ),
        DnsLatencySummary::Unsupported { reason } => (
            "dns_latency_unsupported_detail",
            vec![("reason", reason)],
            DnsObservationTone::Neutral,
        ),
    };
    let summary = localize(code, key, &params);
    let rows = if report.status.is_ready() {
        report
            .results
            .iter()
            .map(|result| {
                let (key, params, tone) = match &result.outcome {
                    DnsProbeOutcome::Measured { rtt_ms } => (
                        "dns_latency_measured",
                        vec![("ms", rtt_ms.to_string())],
                        match project_measured_latency(Some(*rtt_ms)).band {
                            LatencyBand::Fast => DnsObservationTone::Success,
                            LatencyBand::Medium => DnsObservationTone::Warning,
                            _ => DnsObservationTone::Danger,
                        },
                    ),
                    DnsProbeOutcome::TimedOut => {
                        ("dns_latency_timeout", vec![], DnsObservationTone::Danger)
                    }
                    DnsProbeOutcome::InvalidResponse { reason } => (
                        "dns_latency_invalid_response",
                        vec![("reason", reason.clone())],
                        DnsObservationTone::Danger,
                    ),
                    DnsProbeOutcome::Failed { message } => (
                        "dns_latency_failed",
                        vec![("reason", message.clone())],
                        DnsObservationTone::Danger,
                    ),
                    DnsProbeOutcome::NotProbed { reason } => (
                        "dns_latency_not_probed",
                        vec![("reason", reason.clone())],
                        DnsObservationTone::Neutral,
                    ),
                };
                DnsLatencyRow {
                    address: result.address.clone(),
                    tier: localize(
                        code,
                        if result.is_fallback {
                            "dns_latency_fallback"
                        } else {
                            "dns_latency_primary"
                        },
                        &[],
                    ),
                    outcome: localize(code, key, &params),
                    tone,
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    DnsLatencyDisplay {
        empty: if report.status.is_ready() {
            localize(code, "dns_latency_not_probed_yet", &[])
        } else {
            summary.clone()
        },
        summary,
        tone,
        rows,
    }
}
pub fn server_tags_text(tags: &[DnsServerTag], code: &str) -> String {
    tags.iter()
        .map(|tag| {
            localize(
                code,
                match tag {
                    DnsServerTag::Domestic => "dns_tag_domestic",
                    DnsServerTag::Fallback => "dns_tag_fallback",
                    DnsServerTag::Encrypted => "dns_tag_encrypted",
                    DnsServerTag::Plain => "dns_tag_plain",
                },
                &[],
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
}
