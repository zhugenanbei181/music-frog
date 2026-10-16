use super::*;
use crate::view::overview_topology::{topology_badge, topology_route_for_stage};
use infiltrator_contract::traffic_topology::{
    TrafficTopologySnapshot, TrafficTopologyStage, TrafficTopologyStatus,
};

#[test]
fn topology_badge_reflects_the_shared_status_and_count() {
    let mut snapshot = TrafficTopologySnapshot::demo_fixture();
    snapshot.active_connections = 3;
    assert_eq!(
        topology_badge(&snapshot, &Lang("zh-CN")),
        "3 连接 · flowing"
    );

    snapshot.status = TrafficTopologyStatus::Empty;
    snapshot.active_connections = 0;
    assert_eq!(topology_badge(&snapshot, &Lang("zh-CN")), "0 连接 · idle");

    snapshot.status = TrafficTopologyStatus::Unsupported;
    assert_eq!(topology_badge(&snapshot, &Lang("zh-CN")), "拓扑不可用");
}

#[test]
fn topology_stage_routes_follow_the_shared_application_mapping() {
    assert_eq!(
        topology_route_for_stage(TrafficTopologyStage::Inbound),
        Some(Route::Settings)
    );
    assert_eq!(
        topology_route_for_stage(TrafficTopologyStage::Sniffer),
        Some(Route::Settings)
    );
    assert_eq!(
        topology_route_for_stage(TrafficTopologyStage::RuleSet),
        Some(Route::Rules)
    );
    assert_eq!(
        topology_route_for_stage(TrafficTopologyStage::ProxyGroup),
        Some(Route::Proxies)
    );
    assert_eq!(
        topology_route_for_stage(TrafficTopologyStage::Outbound),
        Some(Route::Proxies)
    );
}
