//! The sort strip's owner seam: the four shared sort orders the page offers.
//!
//! The strip itself is the official headless tab list, so the selection, the
//! keyboard navigation and the accessibility roles belong to the widget layer.
//! This module owns what the page owes the owner: the order a strip position
//! means, the preference it submits, and the rule that keeps the strip's
//! selection on the owner's published order.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::proxies::{
    ProxySortAdopted, ProxySortMode, ProxySortPill, ProxySortTabs, outbound_ui_preference_request,
    outbound_ui_preferences,
};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::ui_widgets::{SelectedTab, ValueChange};
use infiltrator_bevy_widgets::tabs::SegmentedTab;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::proxies_page_model::sort_option_index;
use lurker_core_contract::outbound::preferences::{OutboundSortOrder, OutboundUiPreferenceIntent};

/// The owner's sort order one strip position selects.
pub fn proxy_sort_order(mode: ProxySortMode) -> OutboundSortOrder {
    match mode {
        ProxySortMode::LatencyAsc => OutboundSortOrder::LatencyAscending,
        ProxySortMode::LatencyDesc => OutboundSortOrder::LatencyDescending,
        ProxySortMode::NameAsc => OutboundSortOrder::NameAscending,
        ProxySortMode::NameDesc => OutboundSortOrder::NameDescending,
    }
}

/// Submit the owner's sort preference for one strip position.
pub fn submit_proxy_sort_order(
    mode: ProxySortMode,
    handle: &CommandSinkHandle,
    snapshot: &SurfaceSnapshot,
) {
    let order = proxy_sort_order(mode);
    if let Some(request) =
        outbound_ui_preference_request(snapshot, OutboundUiPreferenceIntent::SetSortOrder(order))
    {
        handle.submit(UiCommand::ApplyOutboundUiPreference { request });
    }
}

/// The sort strip is the official tab list: the widget layer owns the
/// selection, so the page reads the selected tab and submits the order.
pub fn on_proxy_sort_selected(
    change: On<ValueChange<Option<Entity>>>,
    strips: Query<(), With<ProxySortTabs>>,
    pills: Query<&ProxySortPill>,
    mut adopted: Query<&mut ProxySortAdopted>,
    handle: Option<Res<CommandSinkHandle>>,
    latest: Option<Res<LatestSurfaceSnapshot>>,
) {
    if !strips.contains(change.source) {
        return;
    }
    let Some(selected) = change.value else {
        return;
    };
    let Ok(pill) = pills.get(selected) else {
        return;
    };
    if let Ok(mut adopted) = adopted.get_mut(change.source) {
        adopted.0 = proxy_sort_order(pill.0);
    }
    let (Some(handle), Some(latest)) = (handle, latest) else {
        return;
    };
    submit_proxy_sort_order(pill.0, &handle, &latest.0);
}

/// Keep the strip's selection on the owner's published sort order.
///
/// The official tab list selects optimistically, so the strip only follows
/// the owner when the owner's published order disagrees with what the strip
/// shows: a pending click is never undone, and a refused order snaps the
/// strip back to the owner's truth.
pub fn sync_proxy_sort_selection(
    latest: Option<Res<LatestSurfaceSnapshot>>,
    mut strips: Query<(Entity, &mut ProxySortAdopted, &mut SelectedTab), With<ProxySortTabs>>,
    tabs: Query<(&SegmentedTab, &ChildOf)>,
) {
    let Some(latest) = latest else {
        return;
    };
    let Some(preferences) = outbound_ui_preferences(&latest.0) else {
        return;
    };
    let order = preferences.sort_order;
    let index = sort_option_index(order);
    for (strip, mut adopted, mut selected) in &mut strips {
        if adopted.0 == order {
            continue;
        }
        let Some(tab) = tabs
            .iter()
            .find(|(tab, parent)| tab.0 == index && parent.parent() == strip)
            .map(|(_, parent)| parent.parent())
        else {
            continue;
        };
        if selected.0 != Some(tab) {
            selected.0 = Some(tab);
        }
        adopted.0 = order;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The strip's four positions, in the order the strip renders them.
    const STRIP_MODES: [ProxySortMode; 4] = [
        ProxySortMode::LatencyAsc,
        ProxySortMode::LatencyDesc,
        ProxySortMode::NameAsc,
        ProxySortMode::NameDesc,
    ];

    #[test]
    fn every_strip_position_means_one_shared_order() {
        for (index, mode) in STRIP_MODES.iter().enumerate() {
            let order = proxy_sort_order(*mode);
            assert_eq!(
                sort_option_index(order),
                index,
                "the strip position and the shared order must agree"
            );
        }
    }
}
