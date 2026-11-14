//! Each proxy group owns its own filter query.
//!
//! The reference client (Mihomo Party / Clash Party) filters every group
//! with its own search box: the query belongs to the group, not to the
//! page. The page-wide query is a different fact — the engine publishes it
//! as a preference, so the owner keeps it — while the per-group query is a
//! surface-local interaction fact and belongs to the state layer.
//!
//! The typed query is the shared [`FilterQuery`], so a per-group query
//! matches a node by exactly the rule the page-wide query uses: one answer
//! for "does this node match?" on every surface.

use infiltrator_presentation::filter_query::FilterQuery;
use std::collections::BTreeMap;

use crate::state::AppState;

/// The filter of one proxy group.
///
/// The two states are exclusive: a group either shows no filter of its own,
/// or it shows one and carries that group's own query.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GroupFilter {
    /// The group shows no filter of its own.
    #[default]
    Closed,
    /// The group shows a filter of its own and carries its query.
    Open { text: String, query: FilterQuery },
}

impl GroupFilter {
    /// The raw text the group's filter box holds.
    pub fn text(&self) -> &str {
        match self {
            Self::Closed => "",
            Self::Open { text, .. } => text,
        }
    }

    /// The typed query of the group's filter, when the group has one.
    pub fn query(&self) -> Option<&FilterQuery> {
        match self {
            Self::Closed => None,
            Self::Open { query, .. } => Some(query),
        }
    }

    /// Whether the group shows a filter of its own.
    pub fn is_open(&self) -> bool {
        matches!(self, Self::Open { .. })
    }
}

/// The per-group filters of the proxies page.
///
/// One entry per group that was given a filter; a group without an entry
/// shows no filter and keeps every member the page-wide query keeps.
#[derive(Clone, Debug, Default)]
pub struct ProxyGroupFilterState {
    filters: BTreeMap<String, GroupFilter>,
}

impl ProxyGroupFilterState {
    /// The filter of one group, when the group has one.
    pub fn filter(&self, group: &str) -> Option<&GroupFilter> {
        self.filters.get(group)
    }

    /// Whether the group shows a filter of its own.
    pub fn is_open(&self, group: &str) -> bool {
        self.filters.get(group).is_some_and(GroupFilter::is_open)
    }

    /// The raw text the group's filter box holds.
    pub fn text(&self, group: &str) -> &str {
        self.filters
            .get(group)
            .map(GroupFilter::text)
            .unwrap_or_default()
    }

    /// The typed query of the group's filter, when the group has one.
    pub fn query(&self, group: &str) -> Option<&FilterQuery> {
        self.filters.get(group).and_then(GroupFilter::query)
    }

    /// Open the filter of one group.
    ///
    /// Opening a group that already shows a filter keeps the text the user
    /// typed; only a group without a filter starts empty.
    pub fn open(&mut self, group: &str) {
        if self.is_open(group) {
            return;
        }
        self.filters.insert(
            group.to_owned(),
            GroupFilter::Open {
                text: String::new(),
                query: FilterQuery::MatchAll,
            },
        );
    }

    /// Close the filter of one group and forget its query.
    pub fn close(&mut self, group: &str) {
        self.filters.remove(group);
    }

    /// Replace the text of one group's filter.
    ///
    /// A group without an open filter ignores the edit: the text belongs to
    /// a filter that is not shown, so no query may be derived from it.
    pub fn set_text(&mut self, group: &str, text: String) {
        if !self.is_open(group) {
            return;
        }
        let query = FilterQuery::parse(&text);
        self.filters
            .insert(group.to_owned(), GroupFilter::Open { text, query });
    }
}

impl AppState {
    /// Open or close the filter one proxy group owns.
    ///
    /// The filter is a surface-local interaction fact, so it is not
    /// published to the owner: the owner's preferences keep the page-wide
    /// query and the group expansion, and this query lives here.
    pub(crate) fn toggle_outbound_group_filter(&mut self, group: &str) {
        if self.runtime.proxy_group_filters.is_open(group) {
            self.runtime.proxy_group_filters.close(group);
        } else {
            self.runtime.proxy_group_filters.open(group);
        }
        self.refresh_outbound_inventory_projection();
    }

    /// Replace the query of the filter one proxy group owns.
    ///
    /// The projection is refreshed so the group shows the members its own
    /// query keeps.
    pub(crate) fn filter_outbound_group(&mut self, group: &str, text: String) {
        self.runtime.proxy_group_filters.set_text(group, text);
        self.refresh_outbound_inventory_projection();
    }
}

#[cfg(test)]
#[path = "../../tests/gui/proxy_group_filter_tests.rs"]
mod tests;
