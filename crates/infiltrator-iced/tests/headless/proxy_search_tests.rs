//! test-intent: behavior
//! Typed demo SurfaceSnapshot search with Unicode display names and pinyin matching.
use crate::test_support::demo_env;
use infiltrator_contract::outbound::identity::SnapshotRevision;
use infiltrator_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState,
};
use infiltrator_contract::surface_snapshot::PageData;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;

#[test]
fn typed_demo_search_keeps_unicode_names_and_filters_observed_outbounds() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    let inventory = state
        .runtime
        .outbound_inventory
        .data
        .as_ref()
        .expect("typed demo inventory")
        .clone();
    let OutboundInventoryState::Ready {
        mut entries,
        collections,
    } = inventory.inventory().clone()
    else {
        panic!("typed demo inventory is ready")
    };
    for entry in &mut entries {
        if matches!(entry.id.value(), "HK-01" | "HK-02") {
            entry.display_name = Some(format!("香港 {}", entry.id.value()));
        }
    }
    let inventory = OutboundInventorySnapshot::new(
        inventory.scope().clone(),
        SnapshotRevision::new(inventory.revision().value() + 1),
        OutboundInventoryState::Ready {
            entries,
            collections,
        },
    )
    .expect("valid Unicode inventory readback");
    let mut surface = state.surface.latest().unwrap().clone();
    surface.revision += 1;
    surface.outbound_inventory = PageData::ready(inventory);
    assert!(state.apply_shared_surface_snapshot(surface));

    let initial = state
        .runtime
        .filtered_groups
        .iter()
        .map(|(_, members)| members.len())
        .sum::<usize>();
    let _ = state.update(Message::FilterProxies("香港".into()));
    let filtered = state
        .runtime
        .filtered_groups
        .iter()
        .map(|(_, members)| members.len())
        .sum::<usize>();
    assert!(filtered > 0 && filtered < initial);
    for name in state
        .runtime
        .filtered_groups
        .iter()
        .flat_map(|(_, members)| members)
    {
        let display_name = state
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .and_then(|inventory| match inventory.inventory() {
                OutboundInventoryState::Ready { entries, .. } => entries
                    .iter()
                    .find(|entry| entry.id.value() == name)
                    .and_then(|entry| entry.display_name.as_deref()),
                _ => None,
            })
            .expect("visible ID has a typed display name");
        assert!(display_name.contains("香港"));
    }
}
