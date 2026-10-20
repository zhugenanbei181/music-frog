//! The one typed page model of the connections page, shared by every surface.
//!
//! The connections page has exactly one state derivation: from the observed
//! connection list to what the page shows. This module owns that derivation —
//! the sort key, the grouping dimension, the query, the match counts and the
//! row order — so a surface renders the model and dispatches typed intents
//! back instead of re-deriving the same state from the list a second time.

use crate::connection_grouping::{ConnectionGroupRow, ConnectionGroupingState, sort_label_key};
use crate::connection_search::ConnectionSearchRow;
use infiltrator_domain::connection_rate::ConnectionRates;
use infiltrator_domain::connection_view::{
    self, ConnectionGroupingMode, ConnectionSortKey, ConnectionView,
};

/// The ordered sort choices of the connections page.
///
/// Every surface presents the same choices in the same order, so a sort
/// selection means the same thing everywhere and a control position maps to
/// exactly one shared key.
pub const CONNECTION_SORT_OPTIONS: [ConnectionSortKey; 6] = [
    ConnectionSortKey::DownloadDesc,
    ConnectionSortKey::UploadDesc,
    ConnectionSortKey::DownloadRateDesc,
    ConnectionSortKey::UploadRateDesc,
    ConnectionSortKey::LatestDesc,
    ConnectionSortKey::HostAsc,
];

/// The position of a sort key in the page's sort control.
pub fn sort_option_index(key: ConnectionSortKey) -> usize {
    CONNECTION_SORT_OPTIONS
        .iter()
        .position(|option| *option == key)
        .unwrap_or(0)
}

/// The sort key a control position selects.
pub fn sort_option_at(index: usize) -> ConnectionSortKey {
    CONNECTION_SORT_OPTIONS
        .get(index)
        .copied()
        .unwrap_or_default()
}

/// The one page model of the connections page.
///
/// A surface renders the connections page from this model alone: the grouped
/// rows, the flat row order, the match counts and the sort selection all come
/// from here, and the surface only dispatches typed intents back.
#[derive(Clone, Debug)]
pub struct ConnectionsPageModel<C> {
    grouping: ConnectionGroupingState<C>,
    sort: ConnectionSortKey,
}

impl<C> Default for ConnectionsPageModel<C> {
    fn default() -> Self {
        Self {
            grouping: ConnectionGroupingState::default(),
            sort: ConnectionSortKey::default(),
        }
    }
}

impl<C: ConnectionView + Clone> ConnectionsPageModel<C> {
    /// Fold one observed connection list into the page model.
    pub fn observe(&mut self, source: &[C]) {
        self.grouping.observe(source);
    }

    /// Drop the observed connections, keeping the page's own selections.
    pub fn clear_observation(&mut self) {
        self.grouping.clear_observation();
    }

    /// Mark the observed connections as no longer readable.
    pub fn mark_unavailable(&mut self) {
        self.grouping.mark_unavailable();
    }

    /// Whether the observed connections are currently readable.
    pub fn source_current(&self) -> bool {
        self.grouping.source_current()
    }

    /// The number of observed connections.
    pub fn source_count(&self) -> usize {
        self.grouping.source_count()
    }

    /// The number of observed connections that survive the current query.
    pub fn matched_count(&self) -> usize {
        self.grouping.matched_count()
    }

    /// The current connection query.
    pub fn query(&self) -> &str {
        self.grouping.query()
    }

    /// Edit the connection query the page applies.
    pub fn edit_query(&mut self, query: &str) {
        self.grouping.edit_query(query);
    }

    /// The grouping dimension the page applies.
    pub fn mode(&self) -> ConnectionGroupingMode {
        self.grouping.mode()
    }

    /// Select the grouping dimension the page applies.
    pub fn select(&mut self, mode: ConnectionGroupingMode) {
        self.grouping.select(mode);
    }

    /// The grouped rows the page renders.
    pub fn rows(&self) -> &[ConnectionGroupRow] {
        self.grouping.rows()
    }

    /// The catalog key of the grouped-row summary.
    pub fn summary_key(&self) -> &'static str {
        self.grouping.summary_key()
    }

    /// The catalog key of the search summary.
    pub fn search_summary_key(&self) -> &'static str {
        self.grouping.search_summary_key()
    }

    /// Whether the query matched nothing although connections are readable.
    pub fn summary_empty(&self) -> bool {
        self.grouping.summary_empty()
    }

    /// The search decision and highlighted text of one connection.
    pub fn search_row(&self, id: &str) -> Option<&ConnectionSearchRow> {
        self.grouping.search_row(id)
    }

    /// The sort key the page applies.
    pub fn sort_key(&self) -> ConnectionSortKey {
        self.sort
    }

    /// Select the sort key the page applies.
    pub fn select_sort(&mut self, key: ConnectionSortKey) {
        self.sort = key;
    }

    /// The position of the page's sort key in the sort control.
    pub fn sort_index(&self) -> usize {
        sort_option_index(self.sort)
    }

    /// Select the sort key a control position names.
    pub fn select_sort_at(&mut self, index: usize) {
        self.sort = sort_option_at(index);
    }

    /// The catalog key of the page's sort selection.
    pub fn sort_label_key(&self) -> &'static str {
        sort_label_key(self.sort)
    }

    /// The observed connections that survive the current query, in observation
    /// order.
    pub fn visible_rows(&self) -> Vec<&C> {
        self.grouping.visible_rows()
    }

    /// Order the given rows with the page's shared sort key.
    ///
    /// The key comes from the model, so every surface orders its rows the
    /// same way; the shared reduction itself lives in the domain.
    pub fn sort_rows<R: ConnectionView>(&self, rows: &mut [R]) {
        connection_view::sort_connections(rows, self.sort);
    }

    /// The observed connections that survive the current query, in page order,
    /// with the shared instantaneous rates attached.
    ///
    /// The rate sort keys rank on real bytes-per-second, so a surface that
    /// owns a rate book asks the model for the order instead of sorting the
    /// rows on its own.
    pub fn ordered_rated_rows(
        &self,
        rates: &ConnectionRates,
    ) -> Vec<connection_view::RatedConnection<'_, C>> {
        let rows = self.grouping.visible_rows();
        let mut rated: Vec<connection_view::RatedConnection<'_, C>> = rows
            .iter()
            .map(|row| connection_view::RatedConnection::new(*row, rates.get(row.view_id())))
            .collect();
        connection_view::sort_connections(&mut rated, self.sort);
        rated
    }
}

#[cfg(test)]
#[path = "connection_page_model_tests.rs"]
mod tests;
