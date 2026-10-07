//! Localized topology status from explicit observations, shared by both peers.
use infiltrator_contract::traffic_topology::{TrafficTopologySnapshot, TrafficTopologyStatus};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn status_label(snapshot: &TrafficTopologySnapshot, locale: &str) -> String {
    let key = match snapshot.status {
        TrafficTopologyStatus::Ready => {
            return localize(
                locale,
                "overview_topology_flowing",
                &[("count", snapshot.active_connections.to_string())],
            );
        }
        TrafficTopologyStatus::Empty => "overview_topology_idle",
        TrafficTopologyStatus::Unknown => "overview_topology_unknown",
        TrafficTopologyStatus::Unsupported => "overview_topology_unavailable",
        TrafficTopologyStatus::Failed => "overview_topology_failed",
    };
    Lang(locale).tr(key).into_owned()
}
