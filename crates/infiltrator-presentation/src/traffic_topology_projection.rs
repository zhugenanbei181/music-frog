//! Localized topology status from explicit observations, shared by both peers.
use crate::text::localize;
use infiltrator_contract::surface_snapshot::PageId;
use infiltrator_contract::traffic_topology::{
    TrafficTopologyNavigationTarget, TrafficTopologySnapshot, TrafficTopologyStage,
    TrafficTopologyStatus,
};
use lurker_ui_presentation::copy::TextCatalog;

pub fn status_label(snapshot: &TrafficTopologySnapshot, catalog: &dyn TextCatalog) -> String {
    let key = match snapshot.status {
        TrafficTopologyStatus::Ready => {
            return localize(
                catalog,
                "overview_topology_flowing",
                &[("count", snapshot.active_connections.to_string())],
            );
        }
        TrafficTopologyStatus::Empty => "overview_topology_idle",
        TrafficTopologyStatus::Unknown => "overview_topology_unknown",
        TrafficTopologyStatus::Unsupported => "overview_topology_unavailable",
        TrafficTopologyStatus::Failed => "overview_topology_failed",
    };
    catalog.text(key).into_owned()
}

pub const fn navigation_target_for_stage(
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
    match navigation_target_for_stage(stage) {
        Some(TrafficTopologyNavigationTarget::Settings) => Some(PageId::Settings),
        Some(TrafficTopologyNavigationTarget::Rules) => Some(PageId::Rules),
        Some(TrafficTopologyNavigationTarget::Proxies) => Some(PageId::Proxies),
        None => None,
    }
}

#[cfg(test)]
#[path = "traffic_topology_projection_tests.rs"]
mod tests;
