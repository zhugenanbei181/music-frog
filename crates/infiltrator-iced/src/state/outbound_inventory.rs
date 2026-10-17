//! Iced's private rendering projection of the engine-neutral outbound model.
//!
//! Product and demo rows are built from typed inventory, details and
//! preference observations.

use infiltrator_contract::surface_snapshot::{PageData, PageStatus};
use infiltrator_shared::fuzzy_search::pinyin_fuzzy_match;
use lurker_core_contract::error::Failure;
use lurker_core_contract::outbound::{
    details::{
        OutboundCollectionKind, OutboundDetail, OutboundDetailsSnapshot, OutboundEndpoint,
        OutboundFeature, OutboundHealthObservation, OutboundLatencySample, OutboundObservation,
        OutboundProbeAvailability, OutboundUdpSupport,
    },
    identity::OutboundId,
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionState,
        SelectionAvailability,
    },
    preferences::{OutboundHealthFilter, OutboundSortOrder, OutboundUiPreferencesSnapshot},
};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use zhuge_ui_core::text::{CaseSensitivity, SearchTextRun, project_text_runs};

pub(crate) type FilteredOutboundGroups = (
    Vec<(String, Vec<String>)>,
    BTreeMap<String, Vec<SearchTextRun>>,
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum OutboundSelectionViewState {
    Unknown,
    Selected(String),
    Empty,
    Unsupported,
    Failed(Failure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OutboundMemberView {
    /// Product IDs are engine-scoped. Demo rows have no engine identity.
    pub id: Option<OutboundId>,
    pub name: String,
    pub display_name: Option<String>,
    pub protocol_display_name: Option<String>,
    pub features: OutboundObservation<Vec<OutboundFeature>>,
    pub endpoint: OutboundObservation<OutboundEndpoint>,
    pub udp: OutboundUdpSupport,
    pub health: OutboundHealthObservation,
    pub current_latency_ms: OutboundObservation<u32>,
    pub latency_history: OutboundObservation<Vec<OutboundLatencySample>>,
    pub probe_availability: OutboundProbeAvailability,
    pub favorite: bool,
}

impl OutboundMemberView {
    pub fn latency_ms(&self) -> Option<u32> {
        match &self.current_latency_ms {
            OutboundObservation::Observed(delay) => Some(*delay),
            OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
        }
    }

    pub fn search_name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.name)
    }

    pub fn supports_udp(&self) -> bool {
        self.udp == OutboundUdpSupport::Supported
            || matches!(
                &self.features,
                OutboundObservation::Observed(features)
                    if features.contains(&OutboundFeature::Udp)
            )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OutboundGroupView {
    pub id: Option<OutboundId>,
    pub name: String,
    pub collection_kind: OutboundObservation<OutboundCollectionKind>,
    pub availability: SelectionAvailability,
    pub selection: OutboundSelectionViewState,
    pub expanded: bool,
    pub proxies: Vec<OutboundMemberView>,
}

impl OutboundGroupView {
    pub fn current(&self) -> Option<&str> {
        match &self.selection {
            OutboundSelectionViewState::Selected(selected) => Some(selected),
            OutboundSelectionViewState::Unknown
            | OutboundSelectionViewState::Empty
            | OutboundSelectionViewState::Unsupported
            | OutboundSelectionViewState::Failed(_) => None,
        }
    }
}

pub(super) fn project_groups(
    inventory_page: &PageData<OutboundInventorySnapshot>,
    details_page: &PageData<OutboundDetailsSnapshot>,
    preferences_page: &PageData<OutboundUiPreferencesSnapshot>,
    previous: &[OutboundGroupView],
) -> Vec<OutboundGroupView> {
    if !matches!(&inventory_page.status, PageStatus::Ready) {
        return Vec::new();
    }
    let Some(inventory) = inventory_page.data.as_ref() else {
        return Vec::new();
    };
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        return Vec::new();
    };

    let details = details_page
        .data
        .as_ref()
        .filter(|snapshot| snapshot.scope() == inventory.scope())
        .filter(|_| matches!(&details_page.status, PageStatus::Ready));
    let preferences = preferences_page
        .data
        .as_ref()
        .filter(|snapshot| snapshot.scope() == inventory.scope())
        .filter(|_| matches!(&preferences_page.status, PageStatus::Ready));

    let mut groups = collections
        .iter()
        .map(|collection| {
            let collection_detail = details.and_then(|snapshot| snapshot.detail(&collection.id));
            let collection_kind = collection_detail
                .map(|detail| detail.collection_kind.clone())
                .unwrap_or(OutboundObservation::Unknown);
            let mut proxies = collection
                .member_ids
                .iter()
                .filter_map(|member_id| {
                    let entry = entries.iter().find(|entry| &entry.id == member_id)?;
                    let detail = details.and_then(|snapshot| snapshot.detail(member_id));
                    let favorite = preferences
                        .is_some_and(|prefs| prefs.favorite_outbound_ids.contains(member_id));
                    let member = member_view(
                        member_id.clone(),
                        entry.display_name.clone(),
                        detail,
                        favorite,
                    );
                    if preferences.is_some_and(|prefs| {
                        prefs.health_filter == OutboundHealthFilter::HealthyOnly
                    }) && member.health != OutboundHealthObservation::Healthy
                    {
                        // Unknown and unsupported health cannot satisfy HealthyOnly.
                        return None;
                    }
                    Some(member)
                })
                .collect::<Vec<_>>();
            let sort_order =
                preferences.map_or(OutboundSortOrder::default(), |prefs| prefs.sort_order);
            sort_members(&mut proxies, sort_order);
            let expanded = preferences.map_or_else(
                || {
                    previous
                        .iter()
                        .find(|group| group.id.as_ref() == Some(&collection.id))
                        .is_none_or(|group| group.expanded)
                },
                |prefs| !prefs.collapsed_group_ids.contains(&collection.id),
            );
            OutboundGroupView {
                id: Some(collection.id.clone()),
                name: collection.id.value().to_owned(),
                collection_kind,
                availability: collection.availability,
                selection: selection_view(&collection.selection),
                expanded,
                proxies,
            }
        })
        .collect::<Vec<_>>();

    if let Some(preferences) = preferences.filter(|preferences| !preferences.group_order.is_empty())
    {
        groups.sort_by(|left, right| {
            group_order_rank(left, &preferences.group_order)
                .cmp(&group_order_rank(right, &preferences.group_order))
                .then_with(|| left.name.cmp(&right.name))
        });
    }
    groups
}

fn member_view(
    id: OutboundId,
    display_name: Option<String>,
    detail: Option<&OutboundDetail>,
    favorite: bool,
) -> OutboundMemberView {
    let (
        protocol_display_name,
        features,
        endpoint,
        udp,
        health,
        current_latency_ms,
        latency_history,
        probe_availability,
    ) = match detail {
        Some(detail) => (
            detail.protocol_display_name.clone(),
            detail.features.clone(),
            detail.endpoint.clone(),
            detail.udp,
            detail.health,
            detail.current_latency_ms.clone(),
            detail.latency_history.clone(),
            detail.probe_availability,
        ),
        None => (
            None,
            OutboundObservation::Unknown,
            OutboundObservation::Unknown,
            OutboundUdpSupport::Unknown,
            OutboundHealthObservation::Unknown,
            OutboundObservation::Unknown,
            OutboundObservation::Unknown,
            OutboundProbeAvailability::Unknown,
        ),
    };
    OutboundMemberView {
        name: id.value().to_owned(),
        id: Some(id),
        display_name,
        protocol_display_name,
        features,
        endpoint,
        udp,
        health,
        current_latency_ms,
        latency_history,
        probe_availability,
        favorite,
    }
}

pub(crate) fn filtered_groups(groups: &[OutboundGroupView], query: &str) -> FilteredOutboundGroups {
    let query = query.trim();
    let mut visible = Vec::new();
    let mut name_runs = BTreeMap::new();

    for group in groups {
        let group_matches = fuzzy_match(&group.name, query);
        let mut members = Vec::new();
        for node in &group.proxies {
            let matches = group_matches || member_matches(node, query);
            if matches {
                members.push(node.name.clone());
            }
            name_runs.insert(node.name.clone(), highlight_runs(node.search_name(), query));
        }
        if group_matches || !members.is_empty() {
            visible.push((group.name.clone(), members));
        }
    }

    (visible, name_runs)
}

fn member_matches(member: &OutboundMemberView, query: &str) -> bool {
    if query.trim().is_empty()
        || fuzzy_match(&member.name, query)
        || fuzzy_match(member.search_name(), query)
    {
        return true;
    }
    let query = query.trim().to_ascii_lowercase();
    if member
        .protocol_display_name
        .as_deref()
        .is_some_and(|name| name.to_ascii_lowercase().contains(&query))
    {
        return true;
    }
    if query == "udp" && member.supports_udp() {
        return true;
    }
    if let Some(delay) = member.latency_ms().filter(|delay| *delay > 0)
        && let Some((operator, bound)) = query.split_at_checked(1)
        && matches!(operator, "<" | ">")
        && let Ok(bound) = bound.trim().parse::<u32>()
    {
        return if operator == "<" {
            delay < bound
        } else {
            delay > bound
        };
    }
    false
}

fn fuzzy_match(value: &str, query: &str) -> bool {
    query.trim().is_empty() || pinyin_fuzzy_match(value, query)
}

fn highlight_runs(value: &str, query: &str) -> Vec<SearchTextRun> {
    let mut runs = project_text_runs(value, query, CaseSensitivity::AsciiInsensitive);
    if !query.trim().is_empty()
        && !runs.iter().any(|run| run.highlighted)
        && pinyin_fuzzy_match(value, &query.trim().to_ascii_lowercase())
    {
        for run in &mut runs {
            run.highlighted = true;
        }
    }
    runs
}

fn selection_view(selection: &OutboundSelectionState) -> OutboundSelectionViewState {
    match selection {
        OutboundSelectionState::Unknown => OutboundSelectionViewState::Unknown,
        OutboundSelectionState::Ready { selected } => {
            OutboundSelectionViewState::Selected(selected.value().to_owned())
        }
        OutboundSelectionState::Empty => OutboundSelectionViewState::Empty,
        OutboundSelectionState::Unsupported => OutboundSelectionViewState::Unsupported,
        OutboundSelectionState::Failed { failure } => {
            OutboundSelectionViewState::Failed(failure.clone())
        }
    }
}

fn group_order_rank(group: &OutboundGroupView, order: &[OutboundId]) -> usize {
    group
        .id
        .as_ref()
        .and_then(|id| order.iter().position(|ordered| ordered == id))
        .unwrap_or(usize::MAX)
}

fn sort_members(members: &mut [OutboundMemberView], sort_order: OutboundSortOrder) {
    members.sort_by(|left, right| {
        if left.favorite != right.favorite {
            return if left.favorite {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        match sort_order {
            OutboundSortOrder::NameAscending => name_order(left, right),
            OutboundSortOrder::NameDescending => name_order(right, left),
            OutboundSortOrder::LatencyAscending => latency_order(left, right, false),
            OutboundSortOrder::LatencyDescending => latency_order(left, right, true),
        }
    });
}

fn name_order(left: &OutboundMemberView, right: &OutboundMemberView) -> Ordering {
    left.search_name()
        .to_lowercase()
        .cmp(&right.search_name().to_lowercase())
}

fn latency_order(
    left: &OutboundMemberView,
    right: &OutboundMemberView,
    descending: bool,
) -> Ordering {
    match (
        left.latency_ms().filter(|delay| *delay > 0),
        right.latency_ms().filter(|delay| *delay > 0),
    ) {
        (Some(left_delay), Some(right_delay)) => if descending {
            right_delay.cmp(&left_delay)
        } else {
            left_delay.cmp(&right_delay)
        }
        .then_with(|| name_order(left, right)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => name_order(left, right),
    }
}
