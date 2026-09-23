use super::*;

use crate::view::overview_hero::overview_speedtest_button;
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
    assert_eq!(
        topology_badge(&snapshot, &Lang("zh-CN")),
        "topology unavailable"
    );
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

#[test]
fn overview_speedtest_button_renders_when_idle_and_testing() {
    let mut state = AppState::empty();
    let lang = Lang("zh-CN");
    let _btn_idle = overview_speedtest_button(&state, &lang);

    // Shared engine phase drives the label/progress and the cancel action.
    state.diag.speedtest.phase = infiltrator_contract::speedtest::SpeedtestPhase::ProbingLatency;
    state.diag.speedtest.progress = infiltrator_contract::speedtest::SpeedtestProgress {
        completed_nodes: 3,
        total_nodes: 10,
        percent: 30.0,
        current_node: None,
        current_mbps: None,
    };
    let _btn_testing = overview_speedtest_button(&state, &lang);

    state.runtime.runtime_testing_all_delays = true;
    let _btn_legacy_flag = overview_speedtest_button(&state, &lang);
}
