//! The one typed page model of the proxies page, shared by every surface.
//!
//! The proxies page had three private filter/sort implementations: Bevy
//! matched the advertised feature tags and the `ss`/`hy2` protocol aliases,
//! Iced matched the UDP capability and the protocol display name, and Slint
//! matched plain substrings. Two surfaces therefore answered "does this node
//! match?" differently for the same node.
//!
//! This module owns that answer. It owns:
//!
//! * the typed parse of the search query (free text, latency bound, or the
//!   empty query that matches everything);
//! * the match rule itself, including the shared protocol alias table;
//! * the member and group ordering, including where an unknown latency ranks;
//! * the shared protocol chip label and the shared name highlighting.
//!
//! The owner facts stay where they are: the contract's outbound inventory,
//! details and preference snapshot are the truth, and this model is their
//! typed fold into what the page renders.

use crate::filter_query::{FilterQuery, LatencyBound};
use crate::operation_phase::OperationPhase;
use infiltrator_contract::geo_region::GeoRegion;
use infiltrator_contract::proxies::ProxyProtocolBadge;
use infiltrator_contract::proxy_filter::ProxyFilterQuery;
use infiltrator_shared::fuzzy_search::pinyin_fuzzy_match;
use lurker_core_contract::outbound::details::OutboundHealthObservation;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::preferences::{
    OutboundGroupExpansion, OutboundHealthFilter, OutboundLayout, OutboundSortOrder,
    OutboundUiPreferencesSnapshot,
};
use std::cmp::Ordering;
use zhuge_ui_core::text::{CaseSensitivity, SearchTextRun, project_text_runs};

/// The sort choices the proxies page offers, in one shared order.
///
/// A sort control position means the same thing on every surface, so a
/// position maps to exactly one shared order.
pub const PROXY_SORT_OPTIONS: [OutboundSortOrder; 4] = [
    OutboundSortOrder::LatencyAscending,
    OutboundSortOrder::LatencyDescending,
    OutboundSortOrder::NameAscending,
    OutboundSortOrder::NameDescending,
];

/// The position of a sort order in the page's sort control.
pub fn sort_option_index(order: OutboundSortOrder) -> usize {
    PROXY_SORT_OPTIONS
        .iter()
        .position(|option| *option == order)
        .unwrap_or(0)
}

/// The sort order a control position selects.
pub fn sort_option_at(index: usize) -> OutboundSortOrder {
    PROXY_SORT_OPTIONS.get(index).copied().unwrap_or_default()
}

/// The protocol aliases the shared filter understands.
///
/// The alias table is the reason `hy2` finds a Hysteria2 node on every
/// surface: before this module the alias only existed in the Bevy filter.
const PROXY_PROTOCOL_ALIASES: [(&str, &str); 3] = [
    ("ss", "shadowsocks"),
    ("hy", "hysteria2"),
    ("hy2", "hysteria2"),
];

/// The canonical display name of a proxy protocol.
///
/// The empty protocol is "Proxy" rather than an empty chip, and an unknown
/// protocol keeps its own spelling instead of degrading into a known one.
pub fn proxy_protocol_chip(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "Proxy".to_owned();
    }
    let badge = ProxyProtocolBadge::from_str_loose(trimmed);
    if badge != ProxyProtocolBadge::Other {
        badge.display_name().to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// The reduced feature tags of one node (EFFECTS-012 #41).
///
/// The engine advertises the same capability under several spellings and
/// sometimes twice; a chip row that reads "UDP, udp, UDP" is a defect, not a
/// fact. The reduction drops the empty tags and keeps the first occurrence
/// of every tag, comparing them case-insensitively, so the chip row is the
/// same on every surface.
pub fn proxy_feature_tags(tags: &[String]) -> Vec<String> {
    let mut reduced: Vec<String> = Vec::with_capacity(tags.len());
    let mut seen: Vec<String> = Vec::with_capacity(tags.len());
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_ascii_lowercase();
        if seen.iter().any(|kept| kept == &key) {
            continue;
        }
        seen.push(key);
        reduced.push(trimmed.to_owned());
    }
    reduced
}

/// The proxies page match rule of the shared filter query.
///
/// The query type is shared with the other pages; what a query *matches*
/// belongs to the page that owns the rows. This rule is the one answer for
/// an outbound node: a free-text query matches the display name, the engine
/// identity, the protocol (raw spelling and shared aliases), an advertised
/// feature tag, the UDP capability, or the pinyin initials of the name. A
/// latency bound keeps the nodes whose measured latency is on the requested
/// side of the bound; a node without a measurement never satisfies a bound.
impl FilterQuery {
    /// Whether the given node satisfies the query.
    pub fn matches<T: ProxyFilterTarget>(&self, target: &T) -> bool {
        match self {
            Self::MatchAll => true,
            Self::Latency(bound) => match target.proxy_latency_ms().filter(|delay| *delay > 0) {
                Some(delay) => match bound {
                    LatencyBound::FasterThan(limit) => delay < *limit,
                    LatencyBound::SlowerThan(limit) => delay > *limit,
                },
                None => false,
            },
            Self::Text(query) => {
                if query.is_empty() {
                    return true;
                }
                contains_ignore_case(target.proxy_name(), query)
                    || target
                        .proxy_identity()
                        .is_some_and(|identity| contains_ignore_case(identity, query))
                    || contains_ignore_case(target.proxy_protocol(), query)
                    || protocol_matches(target.proxy_protocol(), query)
                    || target
                        .proxy_features()
                        .iter()
                        .any(|tag| contains_ignore_case(tag, query))
                    || (query == "udp" && target.proxy_supports_udp())
                    || pinyin_fuzzy_match(target.proxy_name(), query)
                    || target
                        .proxy_identity()
                        .is_some_and(|identity| pinyin_fuzzy_match(identity, query))
            }
        }
    }
}

/// The facts one proxy node exposes to the shared filter rule.
///
/// A surface implements this for its own node view; the rule itself stays in
/// the shared model, so two surfaces can no longer disagree about what a
/// query matches.
pub trait ProxyFilterTarget {
    /// The node's display name.
    fn proxy_name(&self) -> &str;

    /// The engine-scoped identity of the node, when it has one.
    fn proxy_identity(&self) -> Option<&str>;

    /// The protocol the node speaks, raw or as a display name.
    fn proxy_protocol(&self) -> &str;

    /// The feature tags the node advertises.
    fn proxy_features(&self) -> &[String] {
        &[]
    }

    /// The last measured latency of the node.
    fn proxy_latency_ms(&self) -> Option<u32> {
        None
    }

    /// Whether the node carries UDP traffic.
    fn proxy_supports_udp(&self) -> bool {
        false
    }

    /// Whether the node is pinned as a favourite.
    fn proxy_favorite(&self) -> bool {
        false
    }
}

fn contains_ignore_case(value: &str, query: &str) -> bool {
    value.to_ascii_lowercase().contains(query)
}

fn protocol_matches(protocol: &str, query: &str) -> bool {
    let protocol = protocol.trim();
    if protocol.eq_ignore_ascii_case(query) {
        return true;
    }
    PROXY_PROTOCOL_ALIASES.iter().any(|(alias, canonical)| {
        (alias.eq_ignore_ascii_case(query) || canonical.eq_ignore_ascii_case(query))
            && protocol.eq_ignore_ascii_case(canonical)
    })
}

/// The shared name highlighting of one node.
///
/// The highlight follows the same rule as the match: a node the pinyin
/// initials reach is highlighted as a whole, so a search result never shows
/// an unhighlighted match.
pub fn proxy_name_runs(name: &str, query: &FilterQuery) -> Vec<SearchTextRun> {
    let text = match query {
        FilterQuery::MatchAll => {
            return project_text_runs(name, "", CaseSensitivity::AsciiInsensitive);
        }
        FilterQuery::Text(text) => text,
        FilterQuery::Latency(_) => {
            return project_text_runs(name, "", CaseSensitivity::AsciiInsensitive);
        }
    };
    let mut runs = project_text_runs(name, text, CaseSensitivity::AsciiInsensitive);
    if !text.is_empty() && !runs.iter().any(|run| run.highlighted) && pinyin_fuzzy_match(name, text)
    {
        for run in &mut runs {
            run.highlighted = true;
        }
    }
    runs
}

/// The one page model of the proxies page.
///
/// A surface renders the proxies page from this model alone: the search
/// query, the sort order, the health filter, the layout and every visible
/// member come from here, and the surface only dispatches typed intents back.
#[derive(Clone, Debug, Default)]
pub struct ProxiesPageModel {
    query: FilterQuery,
    advanced_filter: ProxyFilterQuery,
    sort_order: OutboundSortOrder,
    health_filter: OutboundHealthFilter,
    layout: OutboundLayout,
    collapsed_groups: Vec<OutboundId>,
    group_order: Vec<OutboundId>,
    favorites: Vec<OutboundId>,
    preference_write: OperationPhase,
}

impl ProxiesPageModel {
    /// Fold the owner's committed preferences into the page model.
    ///
    /// The preference snapshot is the truth for what the owner confirmed; the
    /// surface's own edits stay local until the owner publishes them back.
    pub fn observe_preferences(&mut self, preferences: &OutboundUiPreferencesSnapshot) {
        self.query = FilterQuery::parse(&preferences.search_query);
        self.sort_order = preferences.sort_order;
        self.health_filter = preferences.health_filter;
        self.layout = preferences.layout;
        self.collapsed_groups = preferences.collapsed_group_ids.clone();
        self.group_order = preferences.group_order.clone();
        self.favorites = preferences.favorite_outbound_ids.clone();
    }

    /// Edit the search query the page applies.
    pub fn edit_query(&mut self, raw: &str) {
        self.query = FilterQuery::parse(raw);
    }

    /// The typed search query of the page.
    pub fn query(&self) -> &FilterQuery {
        &self.query
    }

    /// The free-text form of the current search query.
    pub fn search_text(&self) -> &str {
        self.query.text()
    }

    /// The sort order the page applies.
    pub fn sort_order(&self) -> OutboundSortOrder {
        self.sort_order
    }

    /// Select the sort order the page applies.
    pub fn select_sort(&mut self, order: OutboundSortOrder) {
        self.sort_order = order;
    }

    /// The position of the page's sort order in the sort control.
    pub fn sort_index(&self) -> usize {
        sort_option_index(self.sort_order)
    }

    /// Select the sort order a control position names.
    pub fn select_sort_at(&mut self, index: usize) {
        self.sort_order = sort_option_at(index);
    }

    /// The health filter the page applies.
    pub fn health_filter(&self) -> OutboundHealthFilter {
        self.health_filter
    }

    /// Select the health filter the page applies.
    pub fn select_health_filter(&mut self, filter: OutboundHealthFilter) {
        self.health_filter = filter;
    }

    /// The layout the page applies.
    pub fn layout(&self) -> OutboundLayout {
        self.layout
    }

    /// Select the layout the page applies.
    pub fn select_layout(&mut self, layout: OutboundLayout) {
        self.layout = layout;
    }

    /// Whether the page shows only the healthy outbounds.
    pub fn healthy_only(&self) -> bool {
        self.health_filter == OutboundHealthFilter::HealthyOnly
    }

    /// Whether the given health satisfies the page's health filter.
    ///
    /// An unknown or unsupported health never satisfies `HealthyOnly`: the
    /// owner did not report the node as usable.
    pub fn keeps_health(&self, health: OutboundHealthObservation) -> bool {
        match self.health_filter {
            OutboundHealthFilter::All => true,
            OutboundHealthFilter::HealthyOnly => health == OutboundHealthObservation::Healthy,
        }
    }

    /// The expansion state of one group.
    pub fn group_expansion(&self, group_id: &OutboundId) -> OutboundGroupExpansion {
        if self.collapsed_groups.contains(group_id) {
            OutboundGroupExpansion::Collapsed
        } else {
            OutboundGroupExpansion::Expanded
        }
    }

    /// Whether one group is expanded.
    pub fn group_expanded(&self, group_id: &OutboundId) -> bool {
        self.group_expansion(group_id) == OutboundGroupExpansion::Expanded
    }

    /// The rank of one group in the owner's group order.
    ///
    /// A group the owner did not order ranks last.
    pub fn group_rank(&self, group_id: &OutboundId) -> usize {
        self.group_order
            .iter()
            .position(|ordered| ordered == group_id)
            .unwrap_or(usize::MAX)
    }

    /// Whether one outbound is pinned as a favourite.
    pub fn is_favorite(&self, outbound_id: &OutboundId) -> bool {
        self.favorites.contains(outbound_id)
    }

    /// The active advanced multi-dimensional filter query.
    pub fn advanced_filter(&self) -> &ProxyFilterQuery {
        &self.advanced_filter
    }

    /// Set or update the active multi-dimensional filter query.
    pub fn set_advanced_filter(&mut self, filter: ProxyFilterQuery) {
        self.advanced_filter = filter;
    }

    /// Reset the active multi-dimensional filter query to match all.
    pub fn clear_advanced_filter(&mut self) {
        self.advanced_filter.clear();
    }

    /// Whether any basic or advanced filter is actively restricting nodes.
    pub fn has_active_filter(&self) -> bool {
        !self.query.matches_all() || self.advanced_filter.is_active()
    }

    /// Whether the given node satisfies the current search query.
    pub fn matches<T: ProxyFilterTarget>(&self, target: &T) -> bool {
        self.query.matches(target)
    }

    /// Whether the group itself satisfies the current search query.
    ///
    /// A latency bound never matches a group: a group has no latency.
    pub fn group_matches(&self, group_name: &str) -> bool {
        match &self.query {
            FilterQuery::MatchAll => true,
            FilterQuery::Latency(_) => false,
            FilterQuery::Text(query) => {
                contains_ignore_case(group_name, query) || pinyin_fuzzy_match(group_name, query)
            }
        }
    }

    /// The members of one group the page shows.
    ///
    /// A group the query matches keeps all of its members; a group the query
    /// does not match keeps only the members the query matches.
    pub fn visible_members<'a, T: ProxyFilterTarget>(
        &self,
        group_name: &str,
        members: &'a [T],
    ) -> Vec<&'a T> {
        let group_matched = self.group_matches(group_name);
        members
            .iter()
            .filter(|member| {
                let basic_match = group_matched || self.matches(*member);
                basic_match && matches_advanced_filter(&self.advanced_filter, *member)
            })
            .collect()
    }

    /// The shared member order of the page.
    ///
    /// Favourites come first on every surface, then the selected order, then
    /// the name. A node without a measured latency always ranks last, in the
    /// ascending order and in the descending order alike.
    pub fn sort_members<T: ProxyFilterTarget>(&self, members: &mut [T]) {
        members.sort_by(|left, right| {
            right
                .proxy_favorite()
                .cmp(&left.proxy_favorite())
                .then_with(|| match self.sort_order {
                    OutboundSortOrder::NameAscending => name_order(left, right),
                    OutboundSortOrder::NameDescending => name_order(right, left),
                    OutboundSortOrder::LatencyAscending => latency_order(left, right, false),
                    OutboundSortOrder::LatencyDescending => latency_order(left, right, true),
                })
                .then_with(|| name_order(left, right))
        });
    }

    /// The shared member order of the page, as an owned list.
    pub fn ordered_members<T: ProxyFilterTarget + Clone>(&self, members: &[T]) -> Vec<T> {
        let mut ordered = members.to_vec();
        self.sort_members(&mut ordered);
        ordered
    }

    /// The phase of the submitted preference write.
    ///
    /// The owner confirms a preference write with its own revision-fenced
    /// readback; this phase only records that the page submitted a write and
    /// has not seen the answer yet.
    pub fn preference_write_phase(&self) -> &OperationPhase {
        &self.preference_write
    }

    /// The phase of the submitted preference write, for the fold that changes it.
    pub fn preference_write_phase_mut(&mut self) -> &mut OperationPhase {
        &mut self.preference_write
    }
}

fn name_order<T: ProxyFilterTarget>(left: &T, right: &T) -> Ordering {
    left.proxy_name()
        .to_lowercase()
        .cmp(&right.proxy_name().to_lowercase())
}

fn latency_order<T: ProxyFilterTarget>(left: &T, right: &T, descending: bool) -> Ordering {
    match (
        left.proxy_latency_ms().filter(|delay| *delay > 0),
        right.proxy_latency_ms().filter(|delay| *delay > 0),
    ) {
        (Some(left_delay), Some(right_delay)) => {
            if descending {
                right_delay.cmp(&left_delay)
            } else {
                left_delay.cmp(&right_delay)
            }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Whether a node target matches an advanced multi-dimensional proxy filter query.
pub fn matches_advanced_filter<T: ProxyFilterTarget>(query: &ProxyFilterQuery, target: &T) -> bool {
    if !query.is_active() {
        return true;
    }

    // 1. Favorite only
    if query.favorite_only && !target.proxy_favorite() {
        return false;
    }

    // 2. Protocols (if specified)
    if !query.protocols.is_empty() {
        let node_badge = ProxyProtocolBadge::from_str_loose(target.proxy_protocol());
        if !query.protocols.contains(&node_badge) {
            return false;
        }
    }

    // 3. Regions (if specified)
    if !query.regions.is_empty() {
        let node_region = GeoRegion::from_name_heuristic(target.proxy_name());
        if !query.regions.contains(&node_region) {
            return false;
        }
    }

    // 4. Latency bounds
    if query.min_latency_ms.is_some() || query.max_latency_ms.is_some() {
        match target.proxy_latency_ms().filter(|d| *d > 0) {
            Some(delay) => {
                if let Some(min) = query.min_latency_ms
                    && delay < min
                {
                    return false;
                }
                if let Some(max) = query.max_latency_ms
                    && delay > max
                {
                    return false;
                }
            }
            None => return false,
        }
    }

    // 5. Exclude negative keywords
    for exclude in &query.exclude_keywords {
        if exclude.is_empty() {
            continue;
        }
        if contains_ignore_case(target.proxy_name(), exclude)
            || target
                .proxy_identity()
                .is_some_and(|id| contains_ignore_case(id, exclude))
            || contains_ignore_case(target.proxy_protocol(), exclude)
        {
            return false;
        }
    }

    // 6. Positive keyword search
    if let Some(keyword) = &query.search_keyword {
        let trimmed = keyword.trim();
        if !trimmed.is_empty() {
            let matches_kw = contains_ignore_case(target.proxy_name(), trimmed)
                || target
                    .proxy_identity()
                    .is_some_and(|id| contains_ignore_case(id, trimmed))
                || contains_ignore_case(target.proxy_protocol(), trimmed)
                || protocol_matches(target.proxy_protocol(), trimmed)
                || target
                    .proxy_features()
                    .iter()
                    .any(|tag| contains_ignore_case(tag, trimmed))
                || (trimmed == "udp" && target.proxy_supports_udp())
                || pinyin_fuzzy_match(target.proxy_name(), trimmed)
                || target
                    .proxy_identity()
                    .is_some_and(|id| pinyin_fuzzy_match(id, trimmed));
            if !matches_kw {
                return false;
            }
        }
    }

    true
}

/// Maps a proxy node display name to its standard geographic region.
pub fn proxy_node_region(name: &str) -> GeoRegion {
    GeoRegion::from_name_heuristic(name)
}

/// Maps a proxy node display name to its flag asset token (e.g. "flag-hk", "flag-globe").
pub fn proxy_flag_icon_token(name: &str) -> &'static str {
    proxy_node_region(name).flag_icon_token()
}

/// Maps a proxy node display name to its localized Chinese region name.
pub fn proxy_region_label_zh(name: &str) -> &'static str {
    proxy_node_region(name).label_zh()
}

/// Maps a proxy node display name to its localized English region name.
pub fn proxy_region_label_en(name: &str) -> &'static str {
    proxy_node_region(name).label_en()
}

#[cfg(test)]
#[path = "proxies_page_model_tests.rs"]
mod tests;
