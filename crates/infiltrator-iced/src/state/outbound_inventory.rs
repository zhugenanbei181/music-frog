//! Iced's private rendering projection of the engine-neutral outbound model.
//!
//! Product and demo rows are built from typed inventory, details and
//! preference observations.

use infiltrator_presentation::proxies_page_model::{
    ProxiesPageModel, ProxyFilterTarget, proxy_name_runs,
};
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
};
use lurker_core_contract::page::{PageData, PageStatus};
use std::collections::BTreeMap;
use std::sync::OnceLock;
use zhuge_ui_core::text::SearchTextRun;

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

impl ProxyFilterTarget for OutboundMemberView {
    fn proxy_name(&self) -> &str {
        self.search_name()
    }

    fn proxy_identity(&self) -> Option<&str> {
        self.id.as_ref().map(|id| id.value())
    }

    fn proxy_protocol(&self) -> &str {
        self.protocol_display_name.as_deref().unwrap_or_default()
    }

    fn proxy_features(&self) -> &[String] {
        match &self.features {
            OutboundObservation::Observed(features) if features.contains(&OutboundFeature::Udp) => {
                udp_feature_tags()
            }
            _ => &[],
        }
    }

    fn proxy_latency_ms(&self) -> Option<u32> {
        self.latency_ms()
    }

    fn proxy_supports_udp(&self) -> bool {
        self.supports_udp()
    }

    fn proxy_favorite(&self) -> bool {
        self.favorite
    }
}

/// The one advertised feature tag the contract knows today.
fn udp_feature_tags() -> &'static [String] {
    static TAGS: OnceLock<Vec<String>> = OnceLock::new();
    TAGS.get_or_init(|| vec!["udp".to_owned()])
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
    model: &ProxiesPageModel,
    inventory_page: &PageData<OutboundInventorySnapshot>,
    details_page: &PageData<OutboundDetailsSnapshot>,
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
                    let favorite = model.is_favorite(member_id);
                    let member = member_view(
                        member_id.clone(),
                        entry.display_name.clone(),
                        detail,
                        favorite,
                    );
                    if !model.keeps_health(member.health) {
                        // Unknown and unsupported health cannot satisfy HealthyOnly.
                        return None;
                    }
                    Some(member)
                })
                .collect::<Vec<_>>();
            model.sort_members(&mut proxies);
            let expanded = model.group_expanded(&collection.id);
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

    // The owner's group order only ranks the groups it ordered; the rest
    // keep the order the inventory published.
    let ordered = groups
        .iter()
        .filter_map(|group| group.id.as_ref())
        .map(|id| model.group_rank(id))
        .min()
        .is_some_and(|rank| rank != usize::MAX);
    if ordered {
        let rank = |group: &OutboundGroupView| {
            group
                .id
                .as_ref()
                .map(|id| model.group_rank(id))
                .unwrap_or(usize::MAX)
        };
        groups.sort_by(|left, right| {
            rank(left)
                .cmp(&rank(right))
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

pub(crate) fn filtered_groups(
    model: &ProxiesPageModel,
    groups: &[OutboundGroupView],
) -> FilteredOutboundGroups {
    let query = model.query();
    let mut visible = Vec::new();
    let mut name_runs = BTreeMap::new();

    for group in groups {
        let group_matches = model.group_matches(&group.name);
        let mut members = Vec::new();
        for node in &group.proxies {
            let matches = group_matches || model.matches(node);
            if matches {
                members.push(node.name.clone());
            }
            name_runs.insert(
                node.name.clone(),
                proxy_name_runs(node.search_name(), query),
            );
        }
        if group_matches || !members.is_empty() {
            visible.push((group.name.clone(), members));
        }
    }

    (visible, name_runs)
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
