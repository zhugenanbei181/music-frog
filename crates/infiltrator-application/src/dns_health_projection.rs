//! Shared health severity and human-readable suggested actions.
use crate::dns_observation_projection::DnsObservationTone;
use infiltrator_contract::dns_self_heal::{
    DnsSelfHealFix, DnsSelfHealKind, DnsSelfHealSnapshot, DnsSelfHealState,
};
use infiltrator_shared::i18n_interpolator::localize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsHealthRow {
    pub kind: String,
    pub state: String,
    pub tone: DnsObservationTone,
    pub detail: String,
    pub fix: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsHealthDisplay {
    pub overall: String,
    pub tone: DnsObservationTone,
    pub rows: Vec<DnsHealthRow>,
    pub empty: String,
}
impl DnsHealthDisplay {
    pub fn listing(&self) -> String {
        if self.rows.is_empty() {
            return self.empty.clone();
        }
        self.rows
            .iter()
            .map(|row| {
                format!(
                    "{} [{}] {}{}",
                    row.kind,
                    row.state,
                    row.detail,
                    row.fix
                        .as_ref()
                        .map(|fix| format!(" · {fix}"))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
fn state(state: DnsSelfHealState, code: &str) -> (String, DnsObservationTone) {
    let (key, tone) = match state {
        DnsSelfHealState::Healthy => ("dns_self_heal_healthy", DnsObservationTone::Success),
        DnsSelfHealState::Warning => ("dns_self_heal_warning", DnsObservationTone::Warning),
        DnsSelfHealState::Critical => ("dns_self_heal_critical", DnsObservationTone::Danger),
        DnsSelfHealState::Unknown => ("dns_self_heal_unknown", DnsObservationTone::Neutral),
    };
    (localize(code, key, &[]), tone)
}
pub fn project_dns_health(snapshot: &DnsSelfHealSnapshot, code: &str) -> DnsHealthDisplay {
    let (overall, tone) = state(snapshot.overall_state(), code);
    let rows = snapshot
        .checks
        .iter()
        .map(|check| {
            let (state, tone) = state(check.state, code);
            let fix = check.fix.map(|fix| {
                localize(
                    code,
                    "dns_self_heal_fix",
                    &[(
                        "fix",
                        localize(
                            code,
                            match fix {
                                DnsSelfHealFix::RepairDnsListenPort => "dns_self_heal_fix_listen",
                                DnsSelfHealFix::RecheckUpstreams => "dns_self_heal_fix_probe",
                                DnsSelfHealFix::ApplyDnsSettings => "dns_self_heal_fix_settings",
                            },
                            &[],
                        ),
                    )],
                )
            });
            DnsHealthRow {
                kind: localize(
                    code,
                    match check.kind {
                        DnsSelfHealKind::ListenPort => "dns_self_heal_listen_port",
                        DnsSelfHealKind::UpstreamResolution => "dns_self_heal_upstream",
                        DnsSelfHealKind::Topology => "dns_self_heal_topology",
                    },
                    &[],
                ),
                state,
                tone,
                detail: check.detail.clone(),
                fix,
            }
        })
        .collect();
    DnsHealthDisplay {
        overall,
        tone,
        rows,
        empty: localize(code, "dns_self_heal_empty", &[]),
    }
}
