//! Application mapping for topology-node drill-down navigation.

use infiltrator_contract::surface_snapshot::PageId;
use infiltrator_contract::traffic_topology::{
    TrafficTopologyNavigationTarget, TrafficTopologyStage,
};

/// Resolves a shared topology stage into a canonical page without exposing
/// Iced or Bevy route types to the application layer.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrafficTopologyNavigationApplication;

impl TrafficTopologyNavigationApplication {
    pub const fn target_for_stage(
        stage: TrafficTopologyStage,
    ) -> Option<TrafficTopologyNavigationTarget> {
        match stage {
            TrafficTopologyStage::Inbound | TrafficTopologyStage::Sniffer => {
                Some(TrafficTopologyNavigationTarget::Settings)
            }
            TrafficTopologyStage::RuleSet => Some(TrafficTopologyNavigationTarget::Rules),
            TrafficTopologyStage::ProxyGroup | TrafficTopologyStage::Outbound => {
                Some(TrafficTopologyNavigationTarget::Proxies)
            }
        }
    }

    pub const fn page_for_stage(stage: TrafficTopologyStage) -> Option<PageId> {
        match Self::target_for_stage(stage) {
            Some(TrafficTopologyNavigationTarget::Settings) => Some(PageId::Settings),
            Some(TrafficTopologyNavigationTarget::Rules) => Some(PageId::Rules),
            Some(TrafficTopologyNavigationTarget::Proxies) => Some(PageId::Proxies),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::traffic_topology::TrafficTopologyNavigationTarget as Target;

    #[test]
    fn every_topology_stage_has_one_shared_destination() {
        assert_eq!(
            TrafficTopologyNavigationApplication::target_for_stage(
                TrafficTopologyStage::Inbound
            ),
            Some(Target::Settings)
        );
        assert_eq!(
            TrafficTopologyNavigationApplication::target_for_stage(
                TrafficTopologyStage::Sniffer
            ),
            Some(Target::Settings)
        );
        assert_eq!(
            TrafficTopologyNavigationApplication::page_for_stage(TrafficTopologyStage::RuleSet),
            Some(PageId::Rules)
        );
        assert_eq!(
            TrafficTopologyNavigationApplication::page_for_stage(
                TrafficTopologyStage::ProxyGroup
            ),
            Some(PageId::Proxies)
        );
        assert_eq!(
            TrafficTopologyNavigationApplication::page_for_stage(
                TrafficTopologyStage::Outbound
            ),
            Some(PageId::Proxies)
        );
    }
}
