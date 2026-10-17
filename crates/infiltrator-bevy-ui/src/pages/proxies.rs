//! The Proxies page (代理策略): proxy groups and nodes, latency testing,
//! group selection, collapsible strategy groups, and active outbound routing.
//!
//! **Update seam**: mutable nodes carry typed markers ([`ProxiesLine`],
//! [`NodeNameText`], [`NodeFlagText`], [`NodeProtoText`], [`LatencyText`],
//! [`GroupCurrentText`], [`GroupFoldText`]).
//! [`ProxiesPagePlugin`] registers [`apply_proxies_projection`] and action observers
//! once at product assembly. When [`ProxiesProjectionUpdated`] fires,
//! texts, latency inks, group expansion, and selection states restamp in place
//! without tree rebuilds.

use crate::pages::outbound_batch_probe;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
#[path = "proxies_query_access.rs"]
pub mod query_access;
use self::query_access::ProxyActionControls;
#[cfg(any(feature = "capture-tools", test))]
use infiltrator_contract::fixture_identities::{AUTO, HK_PRIMARY, JP, PROXIES, SG, STREAMING, US};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::outbound_batch_probe::{OutboundBatchProbeReadback, submit_batch};
use crate::pages::proxies_custom::{on_custom_node_action_activated, sync_custom_node_studio};
use crate::pages::proxies_filter;
use crate::pages::proxies_refresh::apply_proxies_projection;
use crate::pages::proxies_virtual::ProxiesVirtualState;
use crate::route::Route;
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::{App, Plugin};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui::prelude::{ComputedNode, Node, Val};
/// Root marker on the Proxies page scene.
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::fluid_grid::{FluidCardGrid, compute_ideal_column_layout};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::ResponsiveContext;
use infiltrator_bevy_widgets::theme::{Breakpoint, space};
use infiltrator_contract::latency_display::LatencyBand;
use infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot;
use infiltrator_contract::proxies::ProxyGroupClassification;
use lurker_core_contract::outbound::{
    details::OutboundHealthObservation,
    identity::OutboundId,
    inventory::{OutboundInventoryState, SelectionAvailability},
    preferences::{
        OutboundFavoriteState, OutboundGroupExpansion, OutboundHealthFilter, OutboundLayout,
        OutboundSortOrder, OutboundUiPreferenceIntent, OutboundUiPreferenceRequest,
        OutboundUiPreferencesSnapshot,
    },
    probe::{OutboundProbeBatchScope, OutboundSelectionRequest},
};
use std::collections::BTreeMap;
use zhuge_ui_core::text::SearchTextRun;

pub mod scene;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ProxiesPageRoot;

/// Native vertical scroll surface for all proxy groups and controls.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxiesScrollArea;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProxiesLine(pub ProxiesLineKind);

/// Different text lines on the proxies page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProxiesLineKind {
    /// Overview summary: total groups & total nodes count.
    #[default]
    Summary,
    /// Active exit node name.
    ActiveExit,
    /// Latency test status text.
    TestStatus,
}

/// Marker for a proxy node's latency display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LatencyText {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for a proxy node's name text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeNameText {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for a proxy node's country flag text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeFlagText {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for a proxy node's protocol badge text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeProtoText {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for a proxy group's current selection label text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupCurrentText(pub usize);

/// Marker for a proxy group's fold/expand toggle button text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupFoldText(pub usize);

/// Marker for a proxy group's nodes container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupNodesContainer(pub usize);

/// Marker for the "Test All Groups" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TestAllProxiesButton;

/// Marker for a proxy group speed test button ("组测速").
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct TestProxyGroupButton {
    pub group_idx: usize,
    pub collection_id: Option<OutboundId>,
}

/// Marker for a proxy group fold toggle button.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyGroupFoldButton {
    pub group_idx: usize,
    pub group_name: String,
    pub collection_id: Option<OutboundId>,
}

/// Marker and target information for a proxy node selection button.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyNodeButton {
    pub group_idx: usize,
    pub node_idx: usize,
    pub group_name: String,
    pub node_name: String,
    pub collection_id: Option<OutboundId>,
    pub outbound_id: Option<OutboundId>,
}

/// Marker for the "Add Node" button (+ 添加节点).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AddCustomNodeButton;

/// Marker for the view mode toggle button (网格/列表视图).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToggleViewModeButton;
/// Marker for the "Filter Alive" toggle switch (只看可用).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FilterAliveToggle;

/// Marker for the sort mode pills.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProxySortMode {
    #[default]
    LatencyAsc,
    LatencyDesc,
    NameAsc,
    NameDesc,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProxySortPill(pub ProxySortMode);

/// Marker for favorite pin icon / button on proxy node cards.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
#[require(Button)]
pub struct NodePinButton {
    pub group_idx: usize,
    pub node_idx: usize,
    pub node_name: String,
    pub outbound_id: Option<OutboundId>,
}

/// Marker for latency trend / waveform icon on proxy node cards.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LatencyTrendIcon {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for capability UDP tag on proxy node cards.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeUdpTag {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for the node detail button (DUAL-04-11).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeDetailButton {
    pub group_idx: usize,
    pub node_idx: usize,
    pub node_name: String,
    pub outbound_id: Option<OutboundId>,
}

/// Marker for proxy group move-up button (DUAL-04-12).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyGroupMoveUpButton {
    pub group_idx: usize,
    pub group_name: String,
    pub collection_id: Option<OutboundId>,
}

/// Marker for proxy group move-down button (DUAL-04-12).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyGroupMoveDownButton {
    pub group_idx: usize,
    pub group_name: String,
    pub collection_id: Option<OutboundId>,
}

/// Marker for resetting custom group order (DUAL-04-12).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResetProxyGroupOrderButton;

/// Marker for latency test pulsing skeleton placeholder (DUAL-04-14).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LatencySkeletonPulse {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Canonical display name for proxy protocols (Shadowsocks, Vless, VMess, Trojan, Hysteria2).
pub fn format_protocol_chip(raw_type: &str) -> String {
    proxies_filter::format_protocol_chip(raw_type)
}

/// Resolve latency text color from tier and palette tokens.
pub fn latency_color(tier: LatencyBand, palette: &UiPalette) -> Color {
    match tier {
        LatencyBand::Fast => palette.success,
        LatencyBand::Medium => palette.warning,
        LatencyBand::Slow => palette.danger,
        LatencyBand::NotObserved | LatencyBand::UnconfirmedZero => palette.ink_dim,
    }
}

/// A single proxy node snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyNode {
    /// Opaque engine-scoped identity. `None` is reserved for isolated local UI fixtures.
    pub outbound_id: Option<OutboundId>,
    pub name: String,
    pub node_type: String,
    pub delay_ms: Option<u32>,
    pub health: OutboundHealthObservation,
    pub selected: bool,
    pub favorite: bool,
    pub features: Vec<String>,
}

/// A proxy group snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyGroup {
    /// Opaque engine-scoped collection identity. `None` is reserved for local UI fixtures.
    pub collection_id: Option<OutboundId>,
    pub name: String,
    pub group_type: String,
    pub classification: ProxyGroupClassification,
    pub current: String,
    pub outbound_selection_available: bool,
    pub expanded: bool,
    pub proxies: Vec<ProxyNode>,
}

/// Snapshot of the entire Proxies domain.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxiesProjection {
    pub name_runs: BTreeMap<String, Vec<SearchTextRun>>,
    pub search_query: String,
    pub groups: Vec<ProxyGroup>,
    pub testing: bool,
    pub filter_alive: bool,
    pub compact_view: bool,
    pub active_exit: String,
    /// DUAL-05: the shared custom-node protocol studio. Both surfaces render
    /// this single projection; Bevy keeps no second protocol fact source.
    pub custom_node: ProtocolStudioSnapshot,
}

impl ProxiesProjection {
    /// Believable demo fixture for the Proxies page.
    #[cfg(any(feature = "capture-tools", test))]
    pub fn demo() -> Self {
        Self {
            name_runs: BTreeMap::new(),
            search_query: String::new(),
            active_exit: HK_PRIMARY.to_owned(),
            testing: false,
            filter_alive: false,
            compact_view: false,
            custom_node: Default::default(),
            groups: vec![
                ProxyGroup {
                    collection_id: None,
                    name: PROXIES.to_owned(),
                    group_type: "Selector".to_owned(),
                    classification: ProxyGroupClassification::Selector,
                    current: HK_PRIMARY.to_owned(),
                    outbound_selection_available: true,
                    expanded: true,
                    proxies: vec![
                        ProxyNode {
                            outbound_id: None,
                            name: HK_PRIMARY.to_owned(),
                            node_type: "Shadowsocks".to_owned(),
                            delay_ms: Some(38),
                            health: OutboundHealthObservation::Unknown,
                            selected: true,
                            favorite: true,
                            features: vec!["UDP".to_owned(), "TFO".to_owned()],
                        },
                        ProxyNode {
                            outbound_id: None,
                            name: JP.to_owned(),
                            node_type: "Vmess".to_owned(),
                            delay_ms: Some(65),
                            health: OutboundHealthObservation::Unknown,
                            selected: false,
                            favorite: false,
                            features: vec!["Vision".to_owned()],
                        },
                        ProxyNode {
                            outbound_id: None,
                            name: SG.to_owned(),
                            node_type: "Trojan".to_owned(),
                            delay_ms: Some(72),
                            health: OutboundHealthObservation::Unknown,
                            selected: false,
                            favorite: false,
                            features: vec!["Reality".to_owned()],
                        },
                        ProxyNode {
                            outbound_id: None,
                            name: US.to_owned(),
                            node_type: "Hysteria2".to_owned(),
                            delay_ms: Some(152),
                            health: OutboundHealthObservation::Unknown,
                            selected: false,
                            favorite: false,
                            features: vec!["UDP".to_owned(), "Reality".to_owned()],
                        },
                    ],
                },
                ProxyGroup {
                    collection_id: None,
                    name: AUTO.to_owned(),
                    group_type: "URLTest".to_owned(),
                    classification: ProxyGroupClassification::UrlTest,
                    current: HK_PRIMARY.to_owned(),
                    outbound_selection_available: false,
                    expanded: true,
                    proxies: vec![
                        ProxyNode {
                            outbound_id: None,
                            name: HK_PRIMARY.to_owned(),
                            node_type: "Shadowsocks".to_owned(),
                            delay_ms: Some(38),
                            health: OutboundHealthObservation::Unknown,
                            selected: true,
                            favorite: true,
                            features: vec!["UDP".to_owned(), "TFO".to_owned()],
                        },
                        ProxyNode {
                            outbound_id: None,
                            name: JP.to_owned(),
                            node_type: "Vmess".to_owned(),
                            delay_ms: Some(65),
                            health: OutboundHealthObservation::Unknown,
                            selected: false,
                            favorite: false,
                            features: vec!["Vision".to_owned()],
                        },
                    ],
                },
                ProxyGroup {
                    collection_id: None,
                    name: STREAMING.to_owned(),
                    group_type: "Selector".to_owned(),
                    classification: ProxyGroupClassification::Selector,
                    current: SG.to_owned(),
                    outbound_selection_available: true,
                    expanded: true,
                    proxies: vec![
                        ProxyNode {
                            outbound_id: None,
                            name: SG.to_owned(),
                            node_type: "Trojan".to_owned(),
                            delay_ms: Some(72),
                            health: OutboundHealthObservation::Unknown,
                            selected: true,
                            favorite: false,
                            features: vec!["Reality".to_owned()],
                        },
                        ProxyNode {
                            outbound_id: None,
                            name: US.to_owned(),
                            node_type: "Hysteria2".to_owned(),
                            delay_ms: Some(152),
                            health: OutboundHealthObservation::Unknown,
                            selected: false,
                            favorite: false,
                            features: vec!["UDP".to_owned(), "Reality".to_owned()],
                        },
                    ],
                },
            ],
        }
    }

    /// Total count of proxy nodes across all groups.
    pub fn total_nodes(&self) -> usize {
        self.groups.iter().map(|g| g.proxies.len()).sum()
    }
}

/// The typed event dispatched when proxy data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct ProxiesProjectionUpdated(pub ProxiesProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastProxiesProjection(pub Option<ProxiesProjection>);

// ---- Scene constructors ---------------------------------------------------

// ---- Plugin assembly and native observers -----------------------------------------------

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct ProxiesPagePlugin;

impl Plugin for ProxiesPagePlugin {
    fn build(&self, app: &mut App) {
        // DUAL-05: the listeners read the last projection (e.g. the custom-node
        // save submits the shared draft it carries), so the resource must exist
        // the moment the page is mounted instead of being read as `None` forever.
        app.init_resource::<LastProxiesProjection>();
        app.init_resource::<ProxiesVirtualState>();
        app.init_resource::<OutboundBatchProbeReadback>();
        app.add_observer(apply_proxies_projection);
        app.add_observer(on_proxies_action_activated);
        app.add_observer(outbound_batch_probe::finish_batch_probe);
        app.add_observer(on_custom_node_action_activated);
        // DUAL-05: the custom-node studio is re-covered from the same event in a
        // separate observer so it never shares the page restamp query set.
        app.add_observer(sync_custom_node_studio);
    }
}

/// Reflow proxy node cards to the shared tier column count (1 / 2 / 3 / 4).
/// When container dimensions are measured via [`ComputedNode`], the exact card
/// width is computed via [`compute_ideal_column_layout`] to fill 100% of the
/// container width symmetrically. Otherwise, it falls back to the authoritative
/// percentage basis.
pub fn sync_proxies_node_columns(
    ctx: Option<Res<ResponsiveContext>>,
    last: Option<Res<LastProxiesProjection>>,
    containers: Query<(Option<&ComputedNode>, &Children), With<GroupNodesContainer>>,
    mut nodes: Query<&mut Node, With<ProxyNodeButton>>,
) {
    let Some(ctx) = ctx else {
        return;
    };
    let columns = if last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .is_some_and(|projection| projection.compact_view)
    {
        1
    } else {
        match ctx.breakpoint {
            Breakpoint::Compact => 1,
            Breakpoint::Medium => 2,
            Breakpoint::Expanded => 3,
            Breakpoint::Ultra => 4,
        }
    };
    let fallback_width = Val::Percent(FluidCardGrid::wrapped_item_percent(columns));

    if containers.is_empty() {
        for mut node in &mut nodes {
            if node.width != fallback_width {
                node.width = fallback_width;
            }
        }
    } else {
        for (computed, children) in &containers {
            let target_width = if let Some(computed) = computed {
                let measured_w = computed.size().x * computed.inverse_scale_factor();
                if measured_w > 100.0 {
                    let layout = compute_ideal_column_layout(measured_w, 220.0, space::S8, columns);
                    Val::Px(layout.item_width_px)
                } else {
                    fallback_width
                }
            } else {
                fallback_width
            };

            for child in children.iter() {
                if let Ok(mut node) = nodes.get_mut(*child)
                    && node.width != target_width
                {
                    node.width = target_width;
                }
            }
        }
    }
}

pub(crate) fn on_proxies_action_activated(
    activate: On<Activate>,
    handle: Option<Res<CommandSinkHandle>>,
    latest: Option<Res<LatestSurfaceSnapshot>>,
    mut batch_readback: Option<ResMut<OutboundBatchProbeReadback>>,
    targets: ProxyActionControls,
) {
    let ProxyActionControls {
        test_all_buttons,
        test_group_buttons,
        fold_buttons,
        node_buttons,
        filter_alive_toggles,
        sort_pills,
        pin_buttons,
        toggle_view_buttons,
    } = targets;

    let Some(handle) = handle else {
        return;
    };
    let Some(snapshot) = latest.as_deref().map(|latest| &latest.0) else {
        return;
    };
    if test_all_buttons.contains(activate.entity) {
        let Some(readback) = batch_readback.as_deref_mut() else {
            return;
        };
        submit_batch(
            snapshot,
            OutboundProbeBatchScope::All,
            Route::Proxies,
            &handle,
            readback,
        );
    } else if let Ok(btn) = test_group_buttons.get(activate.entity) {
        let Some(collection_id) = btn.collection_id.clone() else {
            return;
        };
        let Some(readback) = batch_readback.as_deref_mut() else {
            return;
        };
        submit_batch(
            snapshot,
            OutboundProbeBatchScope::Collection(collection_id),
            Route::Proxies,
            &handle,
            readback,
        );
    } else if let Ok(btn) = fold_buttons.get(activate.entity) {
        let Some(collection_id) = btn.collection_id.as_ref() else {
            return;
        };
        let Some(preferences) = outbound_ui_preferences(snapshot) else {
            return;
        };
        let expansion = if preferences.collapsed_group_ids.contains(collection_id) {
            OutboundGroupExpansion::Expanded
        } else {
            OutboundGroupExpansion::Collapsed
        };
        if let Some(request) = outbound_ui_preference_request(
            snapshot,
            OutboundUiPreferenceIntent::SetGroupExpansion {
                group_id: collection_id.clone(),
                expansion,
            },
        ) {
            handle.submit(UiCommand::ApplyOutboundUiPreference { request });
        }
    } else if let Ok(btn) = node_buttons.get(activate.entity) {
        let (Some(collection_id), Some(outbound_id)) =
            (btn.collection_id.as_ref(), btn.outbound_id.as_ref())
        else {
            return;
        };
        let Some(request) = outbound_selection_request(snapshot, collection_id, outbound_id) else {
            return;
        };
        handle.submit(UiCommand::SelectOutbound { request });
    } else if filter_alive_toggles.contains(activate.entity) {
        if let Some(preferences) = outbound_ui_preferences(snapshot) {
            let filter = match preferences.health_filter {
                OutboundHealthFilter::All => OutboundHealthFilter::HealthyOnly,
                OutboundHealthFilter::HealthyOnly => OutboundHealthFilter::All,
            };
            if let Some(request) = outbound_ui_preference_request(
                snapshot,
                OutboundUiPreferenceIntent::SetHealthFilter(filter),
            ) {
                handle.submit(UiCommand::ApplyOutboundUiPreference { request });
            }
        }
    } else if let Ok(pill) = sort_pills.get(activate.entity) {
        let order = match pill.0 {
            ProxySortMode::LatencyAsc => OutboundSortOrder::LatencyAscending,
            ProxySortMode::LatencyDesc => OutboundSortOrder::LatencyDescending,
            ProxySortMode::NameAsc => OutboundSortOrder::NameAscending,
            ProxySortMode::NameDesc => OutboundSortOrder::NameDescending,
        };
        if let Some(request) = outbound_ui_preference_request(
            snapshot,
            OutboundUiPreferenceIntent::SetSortOrder(order),
        ) {
            handle.submit(UiCommand::ApplyOutboundUiPreference { request });
        }
    } else if let Ok(pin) = pin_buttons.get(activate.entity) {
        let Some(outbound_id) = pin.outbound_id.as_ref() else {
            return;
        };
        let Some(preferences) = outbound_ui_preferences(snapshot) else {
            return;
        };
        let state = if preferences.favorite_outbound_ids.contains(outbound_id) {
            OutboundFavoriteState::Unpinned
        } else {
            OutboundFavoriteState::Pinned
        };
        if let Some(request) = outbound_ui_preference_request(
            snapshot,
            OutboundUiPreferenceIntent::SetFavorite {
                outbound_id: outbound_id.clone(),
                state,
            },
        ) {
            handle.submit(UiCommand::ApplyOutboundUiPreference { request });
        }
    } else if toggle_view_buttons.contains(activate.entity)
        && let Some(preferences) = outbound_ui_preferences(snapshot)
    {
        let layout = match preferences.layout {
            OutboundLayout::CompactList => OutboundLayout::Grid,
            OutboundLayout::Grid => OutboundLayout::CompactList,
        };
        if let Some(request) =
            outbound_ui_preference_request(snapshot, OutboundUiPreferenceIntent::SetLayout(layout))
        {
            handle.submit(UiCommand::ApplyOutboundUiPreference { request });
        }
    }
}

pub(crate) fn outbound_selection_request(
    snapshot: &SurfaceSnapshot,
    collection_id: &OutboundId,
    outbound_id: &OutboundId,
) -> Option<OutboundSelectionRequest> {
    use lurker_core_contract::page::PageStatus;

    if !matches!(
        snapshot.outbound_inventory.status,
        PageStatus::Ready | PageStatus::Empty
    ) {
        return None;
    }
    let inventory = snapshot.outbound_inventory.data.as_ref()?;
    if snapshot.core.scope().as_ref() != Some(inventory.scope()) {
        return None;
    }
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        return None;
    };
    let collection = collections
        .iter()
        .find(|collection| &collection.id == collection_id)?;
    if collection.availability != SelectionAvailability::Selectable
        || !entries.iter().any(|entry| &entry.id == outbound_id)
        || !collection.member_ids.contains(outbound_id)
    {
        return None;
    }
    OutboundSelectionRequest::new(
        collection.id.clone(),
        outbound_id.clone(),
        inventory.revision(),
    )
    .ok()
}

pub(crate) fn outbound_ui_preferences(
    snapshot: &SurfaceSnapshot,
) -> Option<&OutboundUiPreferencesSnapshot> {
    use lurker_core_contract::page::PageStatus;

    if !matches!(
        snapshot.outbound_ui_preferences.status,
        PageStatus::Ready | PageStatus::Empty
    ) {
        return None;
    }
    let preferences = snapshot.outbound_ui_preferences.data.as_ref()?;
    (snapshot.core.scope().as_ref() == Some(preferences.scope())).then_some(preferences)
}

pub(crate) fn outbound_ui_preference_request(
    snapshot: &SurfaceSnapshot,
    intent: OutboundUiPreferenceIntent,
) -> Option<OutboundUiPreferenceRequest> {
    let preferences = outbound_ui_preferences(snapshot)?;
    let request = OutboundUiPreferenceRequest::new(preferences.revision(), intent).ok()?;
    request.validate_scope(preferences.scope()).ok()?;
    Some(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
    use infiltrator_presentation::latency_projection::project_proxy_latency;
    use infiltrator_shared::country_flags::node_flag_emoji;

    fn native_caption(delay: Option<u32>) -> (String, LatencyBand) {
        let caption = project_proxy_latency(delay);
        (
            LocalizedText::new(caption.key, caption.params).render(&UiLocale::new("zh-CN")),
            caption.band,
        )
    }

    #[test]
    fn format_latency_tiers() {
        let (s1, t1) = native_caption(Some(35));
        assert_eq!(s1, "35 ms");
        assert_eq!(t1, LatencyBand::Fast);

        let (s2, t2) = native_caption(Some(220));
        assert_eq!(s2, "220 ms");
        assert_eq!(t2, LatencyBand::Medium);

        let (s3, t3) = native_caption(Some(850));
        assert_eq!(s3, "850 ms");
        assert_eq!(t3, LatencyBand::Slow);

        let (s4, t4) = native_caption(Some(0));
        assert_eq!(s4, "0 ms（结果未区分）");
        assert_eq!(t4, LatencyBand::UnconfirmedZero);

        let (s5, t5) = native_caption(None);
        assert_eq!(s5, "尚未测速");
        assert_eq!(t5, LatencyBand::NotObserved);
    }

    #[test]
    fn demo_fixture_counts() {
        let proj = ProxiesProjection::demo();
        assert_eq!(proj.groups.len(), 3);
        assert_eq!(proj.total_nodes(), 8);
        assert_eq!(proj.active_exit, "HK-01");
        assert_eq!(proj.groups[0].name, "PROXIES");
        assert_eq!(proj.groups[0].group_type, "Selector");
        assert_eq!(proj.groups[0].proxies.len(), 4);
        assert_eq!(proj.groups[0].proxies[0].name, "HK-01");
        assert_eq!(proj.groups[0].proxies[0].delay_ms, Some(38));
        assert!(proj.groups[0].expanded);
    }

    #[test]
    fn test_node_flag_extraction() {
        assert_eq!(node_flag_emoji("🇭🇰 香港 01 · BGP 专线"), "🇭🇰");
        assert_eq!(node_flag_emoji("HK-IEPL-01"), "🇭🇰");
        assert_eq!(node_flag_emoji("🇯🇵 日本东京 02 · 极速"), "🇯🇵");
        assert_eq!(node_flag_emoji("JP-Tokyo-01"), "🇯🇵");
        assert_eq!(node_flag_emoji("🇸🇬 新加坡 01 · Anycast"), "🇸🇬");
        assert_eq!(node_flag_emoji("SG-01"), "🇸🇬");
        assert_eq!(node_flag_emoji("🇺🇸 美国硅谷 01 · 4K"), "🇺🇸");
        assert_eq!(node_flag_emoji("US-Silicon-Valley"), "🇺🇸");
        assert_eq!(node_flag_emoji("Taiwan Premium"), "🇹🇼");
        assert_eq!(node_flag_emoji("Korea Seoul 01"), "🇰🇷");
        assert_eq!(node_flag_emoji("Unknown Node"), "🌐");
    }

    #[test]
    fn test_matches_proxy_filter() {
        use crate::pages::proxies_filter::matches_proxy_filter;
        let node = ProxyNode {
            outbound_id: None,
            name: "🇭🇰 香港 01 · BGP 专线".to_owned(),
            node_type: "Shadowsocks".to_owned(),
            delay_ms: Some(38),
            health: OutboundHealthObservation::Unknown,
            selected: true,
            favorite: true,
            features: vec!["UDP".to_owned(), "TFO".to_owned()],
        };

        assert!(matches_proxy_filter(&node, ""));
        assert!(matches_proxy_filter(&node, "香港"));
        assert!(matches_proxy_filter(&node, "hk"));
        assert!(matches_proxy_filter(&node, "xg"));
        assert!(matches_proxy_filter(&node, "Shadowsocks"));
        assert!(matches_proxy_filter(&node, "ss"));
        assert!(matches_proxy_filter(&node, "<100"));
        assert!(!matches_proxy_filter(&node, "<30"));
        assert!(matches_proxy_filter(&node, ">20"));
        assert!(!matches_proxy_filter(&node, ">50"));
        assert!(!matches_proxy_filter(&node, "日本"));
        assert!(!matches_proxy_filter(&node, "jp"));
    }
}
