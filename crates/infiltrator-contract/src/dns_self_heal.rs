//! DUAL-14-13: the shared DNS self-heal contract.
//!
//! The snapshot is assembled from two host facts and one static audit, never
//! from a guess:
//!
//! * the `dns.listen` port observation of the existing port-conflict probe;
//! * the real per-nameserver latency report of the DUAL-14-10 prober;
//! * the domain `validate_dns_topology` audit of the configured profile.
//!
//! Each check carries a typed repair so both surfaces offer the same next
//! step, and a check that could not be observed stays `Unknown` instead of
//! claiming health.

use serde::{Deserialize, Serialize};

/// Which DNS health fact a check observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DnsSelfHealKind {
    /// `dns.listen` port occupation.
    ListenPort,
    /// Upstream reachability as measured by the latency prober.
    UpstreamResolution,
    /// Static topology / anti-leak audit of the configured profile.
    Topology,
}

impl DnsSelfHealKind {
    pub const ALL: [Self; 3] = [Self::ListenPort, Self::UpstreamResolution, Self::Topology];

    /// Stable identifier used by tests and diagnostics.
    pub const fn key(self) -> &'static str {
        match self {
            Self::ListenPort => "listen_port",
            Self::UpstreamResolution => "upstream_resolution",
            Self::Topology => "topology",
        }
    }
}

/// How severe a check is. `Unknown` is the honest answer when the host
/// exposed no fact, and is never rendered as healthy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DnsSelfHealState {
    Healthy,
    Warning,
    Critical,
    #[default]
    Unknown,
}

impl DnsSelfHealState {
    /// Rank used to fold the checks into one overall state (higher wins).
    const fn rank(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Healthy => 1,
            Self::Warning => 2,
            Self::Critical => 3,
        }
    }

    pub const fn is_healthy(self) -> bool {
        matches!(self, Self::Healthy)
    }
}

/// The typed repair a check suggests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DnsSelfHealFix {
    /// Relocate the taken `dns.listen` port through the port-conflict repair.
    RepairDnsListenPort,
    /// Re-run the per-nameserver probe before changing anything.
    RecheckUpstreams,
    /// Edit the DNS workbench form (the topology audit's static finding).
    ApplyDnsSettings,
}

impl DnsSelfHealFix {
    pub const fn key(self) -> &'static str {
        match self {
            Self::RepairDnsListenPort => "repair_dns_listen_port",
            Self::RecheckUpstreams => "recheck_upstreams",
            Self::ApplyDnsSettings => "apply_dns_settings",
        }
    }
}

/// One observed DNS health fact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsSelfHealCheck {
    pub kind: DnsSelfHealKind,
    pub state: DnsSelfHealState,
    /// The host-observed detail (English diagnostics; each surface renders its
    /// own localized summary line from `kind` + `state`).
    pub detail: String,
    pub fix: Option<DnsSelfHealFix>,
}

/// The shared self-heal read model published to both surfaces.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsSelfHealSnapshot {
    pub checks: Vec<DnsSelfHealCheck>,
}

impl DnsSelfHealSnapshot {
    pub fn new(checks: Vec<DnsSelfHealCheck>) -> Self {
        Self { checks }
    }

    pub fn check(&self, kind: DnsSelfHealKind) -> Option<&DnsSelfHealCheck> {
        self.checks.iter().find(|check| check.kind == kind)
    }

    /// The worst observed state; an empty snapshot is honestly `Unknown`.
    pub fn overall_state(&self) -> DnsSelfHealState {
        self.checks
            .iter()
            .map(|check| check.state)
            .max_by_key(|state| state.rank())
            .unwrap_or(DnsSelfHealState::Unknown)
    }

    /// Whether any check suggests a repair.
    pub fn needs_repair(&self) -> bool {
        self.checks.iter().any(|check| check.fix.is_some())
    }

    /// The distinct suggested repairs, in observation order.
    pub fn fixes(&self) -> Vec<DnsSelfHealFix> {
        let mut fixes: Vec<DnsSelfHealFix> = Vec::new();
        for check in &self.checks {
            if let Some(fix) = check.fix
                && !fixes.contains(&fix)
            {
                fixes.push(fix);
            }
        }
        fixes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(
        kind: DnsSelfHealKind,
        state: DnsSelfHealState,
        fix: Option<DnsSelfHealFix>,
    ) -> DnsSelfHealCheck {
        DnsSelfHealCheck {
            kind,
            state,
            detail: format!("{} is {state:?}", kind.key()),
            fix,
        }
    }

    #[test]
    fn an_empty_snapshot_is_unknown_not_healthy() {
        let snapshot = DnsSelfHealSnapshot::default();
        assert_eq!(snapshot.overall_state(), DnsSelfHealState::Unknown);
        assert!(!snapshot.needs_repair());
        assert!(snapshot.fixes().is_empty());
        assert!(snapshot.check(DnsSelfHealKind::ListenPort).is_none());
    }

    #[test]
    fn the_overall_state_is_the_worst_observed_check() {
        let snapshot = DnsSelfHealSnapshot::new(vec![
            check(DnsSelfHealKind::Topology, DnsSelfHealState::Healthy, None),
            check(
                DnsSelfHealKind::UpstreamResolution,
                DnsSelfHealState::Warning,
                Some(DnsSelfHealFix::RecheckUpstreams),
            ),
        ]);
        assert_eq!(snapshot.overall_state(), DnsSelfHealState::Warning);
        assert!(snapshot.needs_repair());
        assert_eq!(snapshot.fixes(), vec![DnsSelfHealFix::RecheckUpstreams]);
        assert!(DnsSelfHealState::Critical.rank() > DnsSelfHealState::Warning.rank());
        assert!(DnsSelfHealState::Healthy.rank() > DnsSelfHealState::Unknown.rank());
    }

    #[test]
    fn a_missing_fact_never_outweighs_a_healthy_one() {
        // Unknown must not beat Healthy: a host that observed a good port
        // cannot be downgraded merely because another probe was absent.
        let snapshot = DnsSelfHealSnapshot::new(vec![
            check(DnsSelfHealKind::ListenPort, DnsSelfHealState::Healthy, None),
            check(
                DnsSelfHealKind::UpstreamResolution,
                DnsSelfHealState::Unknown,
                None,
            ),
        ]);
        assert_eq!(snapshot.overall_state(), DnsSelfHealState::Healthy);
        assert!(!snapshot.needs_repair());

        let critical = DnsSelfHealSnapshot::new(vec![check(
            DnsSelfHealKind::ListenPort,
            DnsSelfHealState::Critical,
            Some(DnsSelfHealFix::RepairDnsListenPort),
        )]);
        assert_eq!(critical.overall_state(), DnsSelfHealState::Critical);
        assert_eq!(critical.fixes(), vec![DnsSelfHealFix::RepairDnsListenPort]);
    }

    #[test]
    fn every_kind_and_fix_has_a_distinct_stable_key() {
        let mut keys: Vec<&str> = DnsSelfHealKind::ALL.iter().map(|kind| kind.key()).collect();
        let unique = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), unique);
        assert_eq!(DnsSelfHealKind::ListenPort.key(), "listen_port");

        let fix_keys = [
            DnsSelfHealFix::RepairDnsListenPort.key(),
            DnsSelfHealFix::RecheckUpstreams.key(),
            DnsSelfHealFix::ApplyDnsSettings.key(),
        ];
        assert_eq!(fix_keys[0], "repair_dns_listen_port");
        assert_eq!(fix_keys[2], "apply_dns_settings");
    }
}
