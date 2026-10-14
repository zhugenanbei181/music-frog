use super::{navigation_target_for_stage, page_for_stage};
use infiltrator_contract::surface_snapshot::PageId;
use infiltrator_contract::traffic_topology::{
    TrafficTopologyNavigationTarget, TrafficTopologyStage,
};

#[test]
fn every_observed_topology_stage_maps_to_a_canonical_page() {
    assert_eq!(
        navigation_target_for_stage(TrafficTopologyStage::Inbound),
        Some(TrafficTopologyNavigationTarget::Settings)
    );
    assert_eq!(
        navigation_target_for_stage(TrafficTopologyStage::Sniffer),
        Some(TrafficTopologyNavigationTarget::Settings)
    );
    assert_eq!(
        page_for_stage(TrafficTopologyStage::RuleSet),
        Some(PageId::Rules)
    );
    assert_eq!(
        page_for_stage(TrafficTopologyStage::ProxyGroup),
        Some(PageId::Proxies)
    );
    assert_eq!(
        page_for_stage(TrafficTopologyStage::Outbound),
        Some(PageId::Proxies)
    );
}
