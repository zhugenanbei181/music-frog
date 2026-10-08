//! Shared traffic-routing topology read model.
//!
//! The topology is a product-level observation, not a widget graph. It is
//! deliberately made from facts exposed by the Mihomo controller: current
//! connections, configured sniffer/inbound settings, selected proxy groups,
//! and aggregate core traffic. Iced, Bevy, Android, and REST adapters can
//! render this model differently without re-deriving routing semantics or
//! inventing a live path.

use serde::{Deserialize, Serialize};

/// The canonical stages shown by the Overview flow:
/// Inbound → Sniffer → RuleSet → Proxy Group → Outbound.
pub const TRAFFIC_TOPOLOGY_STAGE_COUNT: usize = 5;

/// Stable stage identity used by links and surface adapters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrafficTopologyStage {
    #[default]
    Inbound,
    Sniffer,
    RuleSet,
    ProxyGroup,
    Outbound,
}

/// Semantic destination requested by a topology-node drill-down. Toolkit
/// route enums stay outside the shared contract; the application maps these
/// targets to the canonical page vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrafficTopologyNavigationTarget {
    Settings,
    Rules,
    Proxies,
}

impl TrafficTopologyStage {
    /// Ordered stages in the user-visible flow.
    pub const ALL: [Self; TRAFFIC_TOPOLOGY_STAGE_COUNT] = [
        Self::Inbound,
        Self::Sniffer,
        Self::RuleSet,
        Self::ProxyGroup,
        Self::Outbound,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Sniffer => "sniffer",
            Self::RuleSet => "rule_set",
            Self::ProxyGroup => "proxy_group",
            Self::Outbound => "outbound",
        }
    }
}

/// Availability of the topology data source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrafficTopologyStatus {
    /// No controller observation has arrived yet.
    #[default]
    Unknown,
    /// A running core reported at least one active connection.
    Ready,
    /// The core and configuration are readable, but there are no active
    /// connections to animate.
    Empty,
    /// This host/application composition has no topology source.
    Unsupported,
    /// A composed source failed while reading the topology facts.
    Failed,
}

/// One stage in the shared routing chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrafficTopologyNodeSnapshot {
    pub stage: TrafficTopologyStage,
    /// Stable technical label. Localized shells may translate it while
    /// retaining the stage identity.
    pub label: String,
    /// Controller-derived detail such as `Mixed :7890`, a rule name, or a
    /// selected outbound node. Empty means the fact was not reported.
    pub detail: String,
    /// Number of active connections observed at this stage.
    pub active_connections: u32,
    /// Whether this stage currently participates in an observed flow.
    pub active: bool,
}

/// One directed link between adjacent topology stages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrafficTopologyLinkSnapshot {
    pub from: TrafficTopologyStage,
    pub to: TrafficTopologyStage,
    pub active_connections: u32,
    /// Aggregate core bytes per second represented by the link. Mihomo does
    /// not expose per-edge rates, so this is intentionally aggregate rather
    /// than a fabricated per-connection measurement.
    pub flow_bps: f64,
    pub active: bool,
}

/// RTT jitter bucket distribution across observed probe results (CORE-040-05).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RttJitterHistogram {
    pub bucket_0_50_ms: u32,
    pub bucket_50_100_ms: u32,
    pub bucket_100_200_ms: u32,
    pub bucket_200_500_ms: u32,
    pub bucket_over_500_ms: u32,
    pub jitter_p95_ms: u32,
}

/// Complete topology observation shared by both primary surfaces.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrafficTopologySnapshot {
    pub generation: u64,
    pub revision: u64,
    pub status: TrafficTopologyStatus,
    pub failure: Option<String>,
    pub nodes: Vec<TrafficTopologyNodeSnapshot>,
    pub links: Vec<TrafficTopologyLinkSnapshot>,
    pub active_connections: u32,
    pub flow_bps: f64,
    /// `None` means the running config did not report a sniffer block.
    pub sniffer_enabled: Option<bool>,
    /// RTT delay jitter distribution across observed proxies.
    pub jitter_histogram: Option<RttJitterHistogram>,
}

impl TrafficTopologySnapshot {
    /// Explicit deterministic fixture used only by demo/screenshot hosts.
    /// Production sources must construct the snapshot through the
    /// application/domain path instead.
    pub fn demo_fixture() -> Self {
        let nodes = vec![
            node(
                TrafficTopologyStage::Inbound,
                "Client / Inbound",
                "Mixed :7890",
                12,
                true,
            ),
            node(
                TrafficTopologyStage::Sniffer,
                "Sniffer",
                "enabled",
                12,
                true,
            ),
            node(
                TrafficTopologyStage::RuleSet,
                "RuleSet",
                "MRS / GeoIP",
                12,
                true,
            ),
            node(
                TrafficTopologyStage::ProxyGroup,
                "Proxy Group",
                "GLOBAL / PROXIES",
                12,
                true,
            ),
            node(
                TrafficTopologyStage::Outbound,
                "Outbound Node",
                "香港 01 · BGP 专线",
                12,
                true,
            ),
        ];
        let flow_bps = 10_486_437.8;
        let links = adjacent_links(12, flow_bps, true);
        let jitter_histogram = Some(RttJitterHistogram {
            bucket_0_50_ms: 12,
            bucket_50_100_ms: 8,
            bucket_100_200_ms: 4,
            bucket_200_500_ms: 1,
            bucket_over_500_ms: 0,
            jitter_p95_ms: 185,
        });
        Self {
            generation: 1,
            revision: 1,
            status: TrafficTopologyStatus::Ready,
            failure: None,
            nodes,
            links,
            active_connections: 12,
            flow_bps,
            sniffer_enabled: Some(true),
            jitter_histogram,
        }
    }

    pub fn unsupported(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: TrafficTopologyStatus::Unsupported,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn failed(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: TrafficTopologyStatus::Failed,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn unavailable(generation: u64, revision: u64, reason: impl Into<String>) -> Self {
        Self {
            generation,
            revision,
            status: TrafficTopologyStatus::Unknown,
            failure: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn is_drawable(&self) -> bool {
        matches!(
            self.status,
            TrafficTopologyStatus::Ready | TrafficTopologyStatus::Empty
        ) && self.nodes.len() == TRAFFIC_TOPOLOGY_STAGE_COUNT
            && self.links.len() == TRAFFIC_TOPOLOGY_STAGE_COUNT - 1
    }

    pub fn is_flowing(&self) -> bool {
        self.status == TrafficTopologyStatus::Ready
            && self.active_connections > 0
            && self.flow_bps.is_finite()
            && self.flow_bps > 0.0
    }

    pub fn node(&self, stage: TrafficTopologyStage) -> Option<&TrafficTopologyNodeSnapshot> {
        self.nodes.iter().find(|node| node.stage == stage)
    }

    pub fn link(
        &self,
        from: TrafficTopologyStage,
        to: TrafficTopologyStage,
    ) -> Option<&TrafficTopologyLinkSnapshot> {
        self.links
            .iter()
            .find(|link| link.from == from && link.to == to)
    }

    /// A bounded animation speed derived from aggregate observed traffic,
    /// linearly scaled with throughput bandwidth (UI-04-05).
    pub fn linear_flow_speed_hz(&self) -> f32 {
        if !self.is_flowing() {
            return 0.0;
        }
        compute_linear_topology_speed(self.flow_bps)
    }

    /// Legacy alias forwarding to [`linear_flow_speed_hz`].
    pub fn flow_speed_hz(&self) -> f32 {
        self.linear_flow_speed_hz()
    }
}

/// Compute animation flow frequency linearly scaled with throughput bandwidth (UI-04-05).
///
/// Bounded between `0.6` Hz (gentle idle flow) and `2.8` Hz (saturated high-speed pipe),
/// scaling linearly across reference bandwidth up to 100 Mbps (12.5 MB/s).
pub fn compute_linear_topology_speed(bandwidth_bps: f64) -> f32 {
    if !bandwidth_bps.is_finite() || bandwidth_bps <= 0.0 {
        return 0.0;
    }
    const MIN_SPEED_HZ: f32 = 0.6;
    const MAX_SPEED_HZ: f32 = 2.8;
    const REF_BANDWIDTH_BPS: f64 = 100_000_000.0; // 100 Mbps

    let ratio = (bandwidth_bps / REF_BANDWIDTH_BPS).clamp(0.0, 1.0) as f32;
    MIN_SPEED_HZ + ratio * (MAX_SPEED_HZ - MIN_SPEED_HZ)
}

fn node(
    stage: TrafficTopologyStage,
    label: &str,
    detail: &str,
    active_connections: u32,
    active: bool,
) -> TrafficTopologyNodeSnapshot {
    TrafficTopologyNodeSnapshot {
        stage,
        label: label.to_owned(),
        detail: detail.to_owned(),
        active_connections,
        active,
    }
}

/// Construct the four adjacent links for a complete five-stage chain.
pub fn adjacent_links(
    active_connections: u32,
    flow_bps: f64,
    active: bool,
) -> Vec<TrafficTopologyLinkSnapshot> {
    TrafficTopologyStage::ALL
        .windows(2)
        .map(|stages| TrafficTopologyLinkSnapshot {
            from: stages[0],
            to: stages[1],
            active_connections,
            flow_bps,
            active,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_has_five_ordered_stages_and_four_links() {
        let snapshot = TrafficTopologySnapshot::demo_fixture();
        assert_eq!(snapshot.nodes.len(), 5);
        assert_eq!(snapshot.links.len(), 4);
        assert_eq!(
            snapshot
                .nodes
                .iter()
                .map(|node| node.stage)
                .collect::<Vec<_>>(),
            TrafficTopologyStage::ALL
        );
        assert!(snapshot.is_drawable());
        assert!(snapshot.is_flowing());
    }

    #[test]
    fn unsupported_and_empty_snapshots_never_report_flowing() {
        let unsupported = TrafficTopologySnapshot::unsupported(3, 4, "not composed");
        assert!(!unsupported.is_drawable());
        assert!(!unsupported.is_flowing());

        let fixture = TrafficTopologySnapshot::demo_fixture();
        let empty = TrafficTopologySnapshot {
            generation: 3,
            revision: 5,
            status: TrafficTopologyStatus::Empty,
            nodes: fixture.nodes,
            links: adjacent_links(0, 0.0, false),
            ..Default::default()
        };
        assert!(empty.is_drawable());
        assert!(!empty.is_flowing());
    }

    #[test]
    fn animation_speed_is_bounded_and_uses_only_live_flow() {
        let mut snapshot = TrafficTopologySnapshot::demo_fixture();
        let speed = snapshot.flow_speed_hz();
        assert!((0.6..=2.8).contains(&speed));
        snapshot.flow_bps = f64::NAN;
        assert_eq!(snapshot.flow_speed_hz(), 0.0);
    }

    #[test]
    fn linear_bandwidth_flow_speed_scaling() {
        assert_eq!(compute_linear_topology_speed(0.0), 0.0);
        assert_eq!(compute_linear_topology_speed(-100.0), 0.0);
        assert_eq!(compute_linear_topology_speed(f64::NAN), 0.0);

        let speed_low = compute_linear_topology_speed(1_000_000.0); // 1 Mbps
        let speed_mid = compute_linear_topology_speed(50_000_000.0); // 50 Mbps
        let speed_high = compute_linear_topology_speed(100_000_000.0); // 100 Mbps
        let speed_saturated = compute_linear_topology_speed(1_000_000_000.0); // 1 Gbps

        assert!(speed_low < speed_mid);
        assert!(speed_mid < speed_high);
        assert!((speed_high - 2.8).abs() < 1e-5);
        assert!((speed_saturated - 2.8).abs() < 1e-5);
        assert!((speed_mid - 1.7).abs() < 0.01);
    }
}
