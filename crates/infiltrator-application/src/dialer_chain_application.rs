//! DUAL-05-09/10: shared dialer-chain analysis.
//!
//! The static topology lives in `infiltrator_domain::proxy_nodes::dialer`; this
//! module turns it into the contract read model both surfaces render
//! ([`DialerChainReport`]): resolved `node -> dialer -> ...` chains with a
//! shared chain line, and typed loop findings.
//!
//! Two honesty rules are enforced here, not in the views:
//!
//! * a chain that closes a loop is carried as
//!   [`DialerChainEnd::Cycle`](infiltrator_contract::dialer_chain::DialerChainEnd::Cycle)
//!   and is never part of [`DialerChainReport::valid_chains`];
//! * a chain that ends at a proxy group stops there, because the selected
//!   member is a runtime fact the document cannot know.

use infiltrator_contract::dialer_chain::{
    DialerChainEnd, DialerChainReport, DialerChainView, DialerHopKind, DialerHopView,
    DialerLoopFinding, DialerLoopKind, hop_type_label, loop_message,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::proxy_nodes::dialer::{
    DialerChainEnd as DomainChainEnd, DialerCycle, DialerCycleKind, DialerGraphFacts,
    DialerTopology, ResolvedDialerChain,
};

/// DUAL-05-09/10 shared analyzer.
pub struct DialerChainApplication;

impl DialerChainApplication {
    /// Analyze a whole profile document (nodes + proxy groups).
    pub fn analyze_profile(profile_yaml: &str) -> Result<DialerChainReport, Failure> {
        let facts = DialerGraphFacts::from_profile_yaml(profile_yaml).map_err(|error| {
            Failure::new(
                ErrorCode::InvalidInput,
                format!("profile is not a valid dialer graph: {error}"),
                false,
            )
        })?;
        Ok(Self::report_from_facts(&facts))
    }

    /// Project domain facts into the shared read model.
    pub fn report_from_facts(facts: &DialerGraphFacts) -> DialerChainReport {
        let chains = DialerTopology::resolve_all(facts)
            .into_iter()
            .map(chain_view)
            .collect();
        let loops = DialerTopology::detect_cycles(facts)
            .into_iter()
            .map(loop_finding)
            .collect();
        DialerChainReport {
            chains,
            loops,
            warnings: DialerTopology::group_warnings(facts),
        }
    }
}

fn chain_view(chain: ResolvedDialerChain) -> DialerChainView {
    let hops = chain
        .hops
        .iter()
        .map(|hop| DialerHopView {
            kind: if hop.is_group {
                DialerHopKind::Group
            } else {
                DialerHopKind::Node
            },
            type_label: hop_type_label(&hop.node_type, hop.is_group),
            name: hop.name.clone(),
            node_type: hop.node_type.clone(),
            candidates: hop.candidates.clone(),
        })
        .collect();
    let end = match chain.end {
        DomainChainEnd::Complete => DialerChainEnd::Complete,
        DomainChainEnd::MissingTarget(name) => DialerChainEnd::MissingTarget { name },
        DomainChainEnd::GroupBoundary(name) => DialerChainEnd::GroupBoundary { name },
        DomainChainEnd::Cycle(path) => DialerChainEnd::Cycle { path },
    };
    DialerChainView {
        root: chain.root,
        hops,
        end,
    }
}

fn loop_finding(cycle: DialerCycle) -> DialerLoopFinding {
    let kind = match cycle.kind {
        DialerCycleKind::SelfLoop => DialerLoopKind::SelfLoop,
        DialerCycleKind::Mutual => DialerLoopKind::Mutual,
        DialerCycleKind::GroupCycle => DialerLoopKind::RelayGroupCycle,
        DialerCycleKind::Circuit => DialerLoopKind::Circuit,
    };
    DialerLoopFinding {
        kind,
        message: loop_message(kind, &cycle.path),
        path: cycle.path,
        spans_dialer_proxy: cycle.spans_dialer_proxy,
    }
}

#[cfg(test)]
#[path = "dialer_chain_application_test.rs"]
mod dialer_chain_application_test;
