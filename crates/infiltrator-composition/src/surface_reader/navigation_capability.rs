use infiltrator_contract::navigation_capability::{
    NavigationCapabilitySnapshot, NavigationFeatureState,
};
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_contract::surface_snapshot::ProfilesPageSnapshot;
use infiltrator_domain::settings::AppSettings;
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState,
};
use lurker_core_contract::page::PageData;

pub(super) fn project_navigation_capability(
    surface: SurfaceKind,
    requested_destination: &str,
    settings: Option<&AppSettings>,
    profiles: Option<&ProfilesPageSnapshot>,
    outbound_inventory: Option<&OutboundInventorySnapshot>,
) -> NavigationCapabilitySnapshot {
    let has_proxies = profiles.is_some_and(|page| !page.profiles.is_empty())
        || outbound_inventory.is_some_and(|inventory| {
            matches!(inventory.inventory(), OutboundInventoryState::Ready { .. })
        });
    let features = NavigationFeatureState::new(
        settings.is_some_and(|settings| settings.open_logs),
        has_proxies,
    );
    NavigationCapabilitySnapshot::resolve(surface, features, requested_destination)
}

/// The capability answer the snapshot publishes, wrapped in the one typed
/// readback every surface consumes.
///
/// The owner publishes the answer or the surface reads an honest "not ready";
/// no surface ever receives a guessed capability set.
pub(super) fn navigation_capability_readback(
    surface: SurfaceKind,
    requested_destination: &str,
    settings: Option<&AppSettings>,
    profiles: Option<&ProfilesPageSnapshot>,
    outbound_inventory: Option<&OutboundInventorySnapshot>,
) -> PageData<NavigationCapabilitySnapshot> {
    PageData::ready(project_navigation_capability(
        surface,
        requested_destination,
        settings,
        profiles,
        outbound_inventory,
    ))
}

#[cfg(test)]
#[path = "navigation_capability_tests.rs"]
mod tests;
