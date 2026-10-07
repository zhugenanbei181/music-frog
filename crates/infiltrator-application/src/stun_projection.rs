//! One fold of the UDP observation and its separate egress comparison.
use crate::dns_observation_projection::DnsObservationTone;
use infiltrator_contract::stun_probe::{StunEgressComparison, StunProbeReport, StunProbeStatus};
use infiltrator_shared::i18n_interpolator::localize;

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
pub fn project_stun(report: &StunProbeReport, code: &str) -> StunDisplay {
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
    let status = localize(code, key, &params);
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
            code,
            "dns_stun_server",
            &[("server", report.server.clone())],
        ),
        comparison: localize(code, key, &params),
        boundary: localize(code, "dns_stun_not_webrtc", &[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::stun_probe::{StunMappedAddress, StunProbeObservation};

    #[test]
    fn observation_and_comparison_keep_separate_facts_and_opaque_identity_parameters() {
        let observed = StunProbeReport::from_observation(
            StunProbeObservation::observed(
                "stun{mapping}.example:3478",
                StunMappedAddress::new("2001:db8::7", 51000),
            ),
            Some(StunMappedAddress::new("2001:db8::7", 0)),
        );
        for code in ["en-US", "zh-CN"] {
            let display = project_stun(&observed, code);
            assert_eq!(display.tone, DnsObservationTone::Success);
            assert!(display.status.contains("[2001:db8::7]:51000"));
            assert!(display.server.ends_with("stun{mapping}.example:3478"));
            assert_eq!(display.listing().lines().count(), 3);
            let unknown = StunProbeReport::from_observation(
                StunProbeObservation::observed(
                    "stun{mapping}.example:3478",
                    StunMappedAddress::new("2001:db8::7", 51000),
                ),
                None,
            );
            assert_ne!(project_stun(&unknown, code).comparison, display.comparison);
            let missing = StunProbeReport::from_observation(
                StunProbeObservation {
                    status: StunProbeStatus::Observed,
                    ..StunProbeObservation::default()
                },
                None,
            );
            assert_eq!(
                project_stun(&missing, code).tone,
                DnsObservationTone::Neutral
            );
            let failed = StunProbeReport::from_observation(
                StunProbeObservation::failed("stun.example:3478", "socket {mapping}"),
                None,
            );
            let failure = project_stun(&failed, code);
            assert_eq!(failure.tone, DnsObservationTone::Danger);
            assert!(failure.status.ends_with("socket {mapping}"));
        }
    }
}
