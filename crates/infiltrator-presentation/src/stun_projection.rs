//! One fold of the UDP observation and its separate egress comparison.
use crate::text::localize;
use infiltrator_contract::stun_probe::{StunEgressComparison, StunProbeReport, StunProbeStatus};
use infiltrator_shared::dns_observation::DnsObservationTone;
use lurker_ui_presentation::copy::TextCatalog;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StunDisplay {
    pub status: String,
    pub tone: DnsObservationTone,
    pub server: String,
    pub comparison: String,
    pub boundary: String,
}
impl StunDisplay {
    pub fn listing(&self) -> String {
        [&self.server, &self.comparison, &self.boundary]
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }
}
pub fn project_stun(report: &StunProbeReport, catalog: &dyn TextCatalog) -> StunDisplay {
    let (key, params, tone) = match &report.status {
        StunProbeStatus::Observed => match report.mapping() {
            Some(mapping) => (
                "dns_stun_observed",
                vec![("mapping", mapping.display())],
                DnsObservationTone::Success,
            ),
            None => ("dns_stun_unknown", Vec::new(), DnsObservationTone::Neutral),
        },
        StunProbeStatus::TimedOut => (
            "dns_stun_timed_out",
            Vec::new(),
            DnsObservationTone::Warning,
        ),
        StunProbeStatus::Failed { message } => (
            "dns_stun_failed",
            vec![("reason", message.clone())],
            DnsObservationTone::Danger,
        ),
        StunProbeStatus::Unsupported { reason } => (
            "dns_stun_unsupported",
            vec![("reason", reason.clone())],
            DnsObservationTone::Neutral,
        ),
        StunProbeStatus::Unknown => ("dns_stun_unknown", Vec::new(), DnsObservationTone::Neutral),
    };
    let status = localize(catalog, key, &params);
    let (key, params) = match report.comparison() {
        StunEgressComparison::Consistent => ("dns_stun_consistent", Vec::new()),
        StunEgressComparison::Divergent => (
            "dns_stun_divergent",
            vec![
                (
                    "observed",
                    report
                        .mapping()
                        .map(|mapping| mapping.display())
                        .unwrap_or_default(),
                ),
                (
                    "expected",
                    report.expected_egress_ip().unwrap_or_default().to_string(),
                ),
            ],
        ),
        StunEgressComparison::Unknown { reason } => {
            ("dns_stun_unknown_comparison", vec![("reason", reason)])
        }
    };
    StunDisplay {
        status,
        tone,
        server: localize(
            catalog,
            "dns_stun_server",
            &[("server", report.server.clone())],
        ),
        comparison: localize(catalog, key, &params),
        boundary: localize(catalog, "dns_stun_not_webrtc", &[]),
    }
}
