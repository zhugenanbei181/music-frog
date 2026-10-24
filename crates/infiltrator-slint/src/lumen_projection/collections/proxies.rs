//! The proxies page projection of the Slint surface.
//!
//! The proxies page has exactly one state derivation and it is shared: the
//! search query, the match rule, the group and member visibility fold, the
//! member order, the health filter and the group order live in the shared
//! page model. This module renders that model; it never folds the published
//! outbound inventory a second time.
//!
//! The widget's sort menu speaks its own vocabulary, so the adapter in
//! [`crate::widget_vocabulary`] is the one place that maps a widget
//! dimension onto a shared sort order.

use super::super::flag_assets;
use super::super::snapshot_language;
use crate::widget_vocabulary;
use crate::{AppWindow, ObservationRow, ProxyGroup, refresh_model};
use infiltrator_contract::active_exit::ActiveExitStatus;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::proxies_page_model::{
    ProxiesPageModel, ProxyFilterTarget, proxy_protocol_chip,
};
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::outbound::details::{
    OutboundCollectionKind, OutboundDetail, OutboundDetailsState, OutboundFeature,
    OutboundHealthObservation, OutboundObservation, OutboundUdpSupport,
};
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::inventory::{
    OutboundInventoryState, OutboundSelectionSet, OutboundSelectionState,
};
use std::collections::HashMap;

/// One outbound the surface renders in the proxies page.
///
/// The view carries the owner facts the shared model reads; the shared model
/// owns the rule that filters and orders them, so the surface never decides
/// on its own what a query matches.
#[derive(Clone, Debug)]
struct ProxyNodeView {
    id: String,
    name: String,
    protocol: String,
    features: Vec<String>,
    latency_ms: Option<u32>,
    supports_udp: bool,
    favorite: bool,
    health: OutboundHealthObservation,
    active: bool,
    country: String,
}

impl ProxyFilterTarget for ProxyNodeView {
    fn proxy_name(&self) -> &str {
        &self.name
    }

    fn proxy_identity(&self) -> Option<&str> {
        Some(&self.id)
    }

    fn proxy_protocol(&self) -> &str {
        &self.protocol
    }

    fn proxy_features(&self) -> &[String] {
        &self.features
    }

    fn proxy_latency_ms(&self) -> Option<u32> {
        self.latency_ms
    }

    fn proxy_supports_udp(&self) -> bool {
        self.supports_udp
    }

    fn proxy_favorite(&self) -> bool {
        self.favorite
    }
}

/// Projects the proxies page from the shared page model.
///
/// The model owns the query, the match rule, the member order, the health
/// filter and the group order; the surface only renders the rows the model
/// reports and writes the owner's search and sort readback back into the
/// widget.
pub(super) fn project_proxies(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    let language = snapshot_language(snapshot);
    let lang = Lang(&language);
    let mut model = ProxiesPageModel::default();
    if let Some(preferences) = snapshot.outbound_ui_preferences.data.as_ref() {
        model.observe_preferences(preferences);
        window.set_proxy_search(preferences.search_query.clone().into());
    }
    window.set_proxy_sort_mode(widget_vocabulary::proxy_sort_value(model.sort_order()).into());
    let Some(inventory) = snapshot.outbound_inventory.data.as_ref() else {
        clear_proxies(window);
        return;
    };
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        clear_proxies(window);
        return;
    };
    let names = entries
        .iter()
        .map(|entry| {
            (
                entry.id.value().to_owned(),
                entry
                    .display_name
                    .clone()
                    .unwrap_or_else(|| entry.id.value().to_owned()),
            )
        })
        .collect::<HashMap<_, _>>();
    let details = snapshot
        .outbound_details
        .data
        .as_ref()
        .and_then(|details| match details.state() {
            OutboundDetailsState::Ready { details } => Some(details.as_slice()),
            OutboundDetailsState::Unknown
            | OutboundDetailsState::Empty
            | OutboundDetailsState::Unsupported
            | OutboundDetailsState::Failed { .. } => None,
        })
        .unwrap_or_default();
    let mut groups = collections
        .iter()
        .map(|collection| {
            let selected_id = match &collection.selection {
                OutboundSelectionState::Ready { selected } => Some(selected.value()),
                OutboundSelectionState::Unknown
                | OutboundSelectionState::Empty
                | OutboundSelectionState::Unsupported
                | OutboundSelectionState::Failed { .. } => None,
            };
            let current = selected_id
                .and_then(|id| names.get(id))
                .map(|name| name.clone())
                .unwrap_or_else(|| "".to_owned());
            let kind = details
                .iter()
                .find(|detail| detail.id == collection.id)
                .and_then(|detail| match &detail.collection_kind {
                    OutboundObservation::Observed(kind) => {
                        Some(collection_kind_label(*kind, &language))
                    }
                    OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
                })
                .unwrap_or_else(|| lang.tr("策略组").into_owned());
            let member_names = collection
                .member_ids
                .iter()
                .filter_map(|id| names.get(id.value()))
                .cloned()
                .collect::<Vec<_>>();
            let active_country = if active_exit_matches(snapshot, &collection.id, selected_id) {
                snapshot
                    .active_exit
                    .country_code
                    .as_deref()
                    .unwrap_or_default()
            } else {
                ""
            };
            (
                collection.id.clone(),
                ProxyGroup {
                    name: collection.id.value().to_owned().into(),
                    icon_emoji: "".into(),
                    kind: kind.into(),
                    current: current.clone().into(),
                    current_flag_code: active_country.to_ascii_lowercase().into(),
                    current_flag_index: flag_assets::index_for_code(active_country).unwrap_or(-1),
                    current_accessible: current.clone().into(),
                    nodes: member_names.join("\n").into(),
                },
            )
        })
        .collect::<Vec<_>>();
    // The group order is the owner's order; a group the owner did not order
    // keeps the engine order behind the ordered ones.
    groups.sort_by_key(|(id, _)| model.group_rank(id));
    let groups = groups
        .into_iter()
        .map(|(_, group)| group)
        .collect::<Vec<_>>();
    let requested_group = window.get_selected_group().to_string();
    let visible_group = if !requested_group.is_empty()
        && collections
            .iter()
            .any(|group| group.id.value() == requested_group)
    {
        Some(requested_group.as_str())
    } else {
        if !requested_group.is_empty() {
            window.set_selected_group("".into());
        }
        selected_group_from_snapshot(snapshot, collections)
    };
    let nodes = visible_group
        .and_then(|group_name| {
            collections
                .iter()
                .find(|group| group.id.value() == group_name)
        })
        .map(|group| {
            let selected_id = match &group.selection {
                OutboundSelectionState::Ready { selected } => Some(selected.value()),
                _ => None,
            };
            let members = group
                .member_ids
                .iter()
                .map(|id| {
                    let detail = details.iter().find(|detail| detail.id == *id);
                    let name = names
                        .get(id.value())
                        .cloned()
                        .unwrap_or_else(|| id.value().to_owned());
                    let delay = detail.and_then(|detail| match &detail.current_latency_ms {
                        OutboundObservation::Observed(delay) => Some(*delay),
                        OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
                    });
                    let country = if active_exit_matches(snapshot, &group.id, Some(id.value())) {
                        snapshot
                            .active_exit
                            .country_code
                            .as_deref()
                            .unwrap_or_default()
                    } else {
                        ""
                    };
                    ProxyNodeView {
                        id: id.value().to_owned(),
                        name,
                        protocol: detail
                            .and_then(|detail| detail.protocol_display_name.clone())
                            .unwrap_or_default(),
                        features: match detail {
                            Some(detail) => feature_tags(detail),
                            None => Vec::new(),
                        },
                        latency_ms: delay,
                        supports_udp: detail
                            .is_some_and(|detail| detail.udp == OutboundUdpSupport::Supported),
                        favorite: model.is_favorite(id),
                        health: detail
                            .map_or(OutboundHealthObservation::Unknown, |detail| detail.health),
                        active: selected_id == Some(id.value()),
                        country: country.to_owned(),
                    }
                })
                .filter(|member| model.keeps_health(member.health))
                .collect::<Vec<_>>();
            // The group fold and the member order are the shared model's.
            let mut visible = model
                .visible_members(group.id.value(), &members)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            model.sort_members(&mut visible);
            visible.into_iter().map(node_row).collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let has_nodes = !nodes.is_empty();
    window.set_proxy_groups(refresh_model(&window.get_proxy_groups(), groups));
    window.set_proxy_nodes(refresh_model(&window.get_proxy_nodes(), nodes));
    window.set_proxy_current_node_available(has_nodes);
}

/// Clears the proxies page when the owner published no inventory.
fn clear_proxies(window: &AppWindow) {
    window.set_proxy_groups(refresh_model(&window.get_proxy_groups(), Vec::new()));
    window.set_proxy_nodes(refresh_model(&window.get_proxy_nodes(), Vec::new()));
    window.set_proxy_current_node_available(false);
    window.set_selected_group("".into());
}

/// The node row the surface renders.
fn node_row(node: ProxyNodeView) -> ObservationRow {
    let ProxyNodeView {
        id,
        name,
        protocol,
        latency_ms,
        active,
        country,
        ..
    } = node;
    ObservationRow {
        id: id.into(),
        title: name.clone().into(),
        accessible_title: name.into(),
        flag_code: country.to_ascii_lowercase().into(),
        flag_index: flag_assets::index_for_code(&country).unwrap_or(-1),
        delay_ms: latency_ms
            .map(|value| value.min(i32::MAX as u32) as i32)
            .unwrap_or(-1),
        subtitle: proxy_protocol_chip(&protocol).into(),
        trailing: latency_ms
            .map(|value| format!("{value} ms"))
            .unwrap_or_else(|| "—".to_owned())
            .into(),
        active,
    }
}

/// The feature tags one outbound advertises.
fn feature_tags(detail: &OutboundDetail) -> Vec<String> {
    match &detail.features {
        OutboundObservation::Observed(features) => features
            .iter()
            .map(|feature| match feature {
                OutboundFeature::Udp => "UDP".to_owned(),
            })
            .collect(),
        OutboundObservation::Unknown | OutboundObservation::Unsupported => Vec::new(),
    }
}

fn active_exit_matches(
    snapshot: &SurfaceSnapshot,
    group: &OutboundId,
    member_id: Option<&str>,
) -> bool {
    let Some(member_id) = member_id else {
        return false;
    };
    matches!(
        &snapshot.active_exit.status,
        ActiveExitStatus::Ready {
            group_id,
            outbound_id,
        } if group_id == group && outbound_id.value() == member_id
    )
}

/// The group the owner marked as active, or the first group the engine knows.
fn selected_group_from_snapshot<'a>(
    snapshot: &'a SurfaceSnapshot,
    collections: &'a [OutboundSelectionSet],
) -> Option<&'a str> {
    snapshot
        .active_exit
        .group
        .as_deref()
        .or_else(|| collections.first().map(|group| group.id.value()))
}

/// The localized label of the role one collection plays.
fn collection_kind_label(kind: OutboundCollectionKind, language: &str) -> String {
    let lang = Lang(language);
    match kind {
        OutboundCollectionKind::ManualSelection => lang.tr("手动选择").into_owned(),
        OutboundCollectionKind::AutomaticSelection => lang.tr("自动选择").into_owned(),
        OutboundCollectionKind::Failover => lang.tr("故障转移").into_owned(),
        OutboundCollectionKind::LoadBalancing => lang.tr("负载均衡").into_owned(),
        OutboundCollectionKind::Relay => lang.tr("链式中继").into_owned(),
    }
}
