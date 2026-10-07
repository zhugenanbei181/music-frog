//! One localized fold of DNS cross-source facts for both peer products.
use infiltrator_contract::dns_leak::DnsLeakOperation;
use infiltrator_contract::dns_leak::{DnsLeakConclusion, DnsLeakObservationOutcome, DnsLeakReport};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeakTone {
    Neutral,
    Success,
    Warning,
    Danger,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeakRow {
    pub resolver: String,
    pub authority: String,
    pub question: String,
    pub outcome: String,
    pub tone: LeakTone,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeakDisplay {
    pub conclusion: String,
    pub tone: LeakTone,
    pub sources: String,
    pub rows: Vec<LeakRow>,
    pub empty: String,
    pub feedback: Option<String>,
}
impl LeakDisplay {
    pub fn listing(&self) -> String {
        if self.rows.is_empty() {
            return self.empty.clone();
        }
        self.rows
            .iter()
            .map(|row| format!("{} → {} {}", row.resolver, row.authority, row.outcome))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
pub fn project_leak(report: &DnsLeakReport, code: &str) -> LeakDisplay {
    let (key, params, tone) = match report.conclusion() {
        DnsLeakConclusion::Consistent { identity, facts } => (
            "dns_leak_consistent",
            vec![("count", facts.len().to_string()), ("identity", identity)],
            LeakTone::Success,
        ),
        DnsLeakConclusion::Divergent { facts } => {
            let identities: BTreeSet<_> = facts.iter().map(|fact| &fact.identity).collect();
            (
                "dns_leak_divergent",
                vec![("count", identities.len().to_string())],
                LeakTone::Danger,
            )
        }
        DnsLeakConclusion::Unsupported { reason } => (
            "dns_leak_unsupported",
            vec![("reason", reason)],
            LeakTone::Neutral,
        ),
        DnsLeakConclusion::Failed { reason } => (
            "dns_leak_failed",
            vec![("reason", reason)],
            LeakTone::Warning,
        ),
        DnsLeakConclusion::Unknown => ("dns_leak_unknown", Vec::new(), LeakTone::Neutral),
    };
    let rows = report
        .observations
        .iter()
        .map(|observation| {
            let (key, params, tone) = match &observation.outcome {
                DnsLeakObservationOutcome::Observed { identity } => (
                    "dns_leak_observed",
                    vec![("identity", identity.clone())],
                    LeakTone::Success,
                ),
                DnsLeakObservationOutcome::TimedOut => {
                    ("dns_latency_timeout", Vec::new(), LeakTone::Warning)
                }
                DnsLeakObservationOutcome::InvalidResponse { reason } => (
                    "dns_latency_invalid_response",
                    vec![("reason", reason.clone())],
                    LeakTone::Danger,
                ),
                DnsLeakObservationOutcome::Failed { message } => (
                    "dns_latency_failed",
                    vec![("reason", message.clone())],
                    LeakTone::Danger,
                ),
                DnsLeakObservationOutcome::NotProbed { reason } => (
                    "dns_latency_not_probed",
                    vec![("reason", reason.clone())],
                    LeakTone::Neutral,
                ),
            };
            LeakRow {
                resolver: observation.resolver.clone(),
                authority: observation.authority.clone(),
                question: observation.question.clone(),
                outcome: localize(code, key, &params),
                tone,
            }
        })
        .collect();
    LeakDisplay {
        conclusion: localize(code, key, &params),
        tone,
        sources: localize(
            code,
            "dns_leak_sources",
            &[("count", report.sources.len().to_string())],
        ),
        rows,
        empty: Lang(code).tr("dns_leak_empty").into_owned(),
        feedback: match &report.operation {
            DnsLeakOperation::Running => Some(Lang(code).tr("dns_leak_probing").into_owned()),
            DnsLeakOperation::Failed { failure } => Some(localize(
                code,
                "dns_leak_operation_failed",
                &[("reason", failure.message.clone())],
            )),
            _ => None,
        },
    }
}
#[cfg(test)]
#[path = "dns_leak_projection_test.rs"]
mod tests;
