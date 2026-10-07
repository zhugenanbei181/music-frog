//! One cached grouping fold for native peers; renderers only replay its rows.
use crate::byte_format::format_bytes;
use crate::connection_search::project_search_row;
use infiltrator_contract::connection_search::ConnectionSearchRow;
use infiltrator_domain::connection_view::{
    self, ConnectionGroupingMode, ConnectionSortKey, ConnectionView,
};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionGroupRow {
    pub key: String,
    pub count: usize,
    pub traffic: String,
}
#[derive(Clone, Debug)]
pub struct ConnectionGroupingState<C> {
    source: Vec<C>,
    query: String,
    mode: ConnectionGroupingMode,
    rows: Vec<ConnectionGroupRow>,
    search: HashMap<String, ConnectionSearchRow>,
    matches: usize,
    source_current: bool,
}
impl<C> Default for ConnectionGroupingState<C> {
    fn default() -> Self {
        Self {
            source: Vec::new(),
            query: String::new(),
            mode: ConnectionGroupingMode::Flat,
            rows: Vec::new(),
            search: HashMap::new(),
            matches: 0,
            source_current: false,
        }
    }
}
impl<C: ConnectionView + Clone> ConnectionGroupingState<C> {
    pub fn observe(&mut self, source: &[C]) {
        self.source_current = true;
        self.source = source.to_vec();
        self.fold();
    }
    pub fn select(&mut self, mode: ConnectionGroupingMode) {
        if self.mode != mode {
            self.mode = mode;
            self.fold();
        }
    }
    pub fn edit_query(&mut self, query: &str) {
        if self.query != query {
            self.query = query.into();
            self.fold();
        }
    }
    pub fn source_current(&self) -> bool {
        self.source_current
    }
    pub fn mark_unavailable(&mut self) {
        self.source_current = false;
    }
    pub fn clear_observation(&mut self) {
        self.source.clear();
        self.source_current = false;
        self.fold();
    }
    pub fn search_summary_key(&self) -> &'static str {
        if self.source_current {
            "conn_search_results"
        } else {
            "conn_search_unavailable"
        }
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn source_count(&self) -> usize {
        self.source.len()
    }
    pub fn matched_count(&self) -> usize {
        self.matches
    }
    pub fn search_row(&self, id: &str) -> Option<&ConnectionSearchRow> {
        self.search.get(id)
    }
    pub fn summary_empty(&self) -> bool {
        self.source_current && !self.query.trim().is_empty() && self.matches == 0
    }
    pub fn mode(&self) -> ConnectionGroupingMode {
        self.mode
    }
    pub fn rows(&self) -> &[ConnectionGroupRow] {
        &self.rows
    }
    pub fn summary_key(&self) -> &'static str {
        if self.rows.is_empty() {
            "conn_aggregate_empty"
        } else {
            match self.mode {
                ConnectionGroupingMode::Flat => "conn_group_summary_flat",
                ConnectionGroupingMode::ByProcess => "conn_group_summary_process",
                ConnectionGroupingMode::ByHost => "conn_group_summary_host",
            }
        }
    }
    fn fold(&mut self) {
        self.search = self
            .source
            .iter()
            .map(|row| (row.view_id().into(), project_search_row(row, &self.query)))
            .collect();
        self.matches = self.search.values().filter(|row| row.visible).count();
        let filtered: Vec<C> = self
            .source
            .iter()
            .filter(|row| {
                self.search
                    .get(row.view_id())
                    .is_some_and(|row| row.visible)
            })
            .cloned()
            .collect();
        self.rows = connection_view::aggregate_connections(&filtered, self.mode)
            .into_iter()
            .map(|row| ConnectionGroupRow {
                key: row.key,
                count: row.count,
                traffic: format!(
                    "↑ {} / ↓ {}",
                    format_bytes(row.upload_total),
                    format_bytes(row.download_total)
                ),
            })
            .collect();
    }
}
pub fn grouping_label_key(mode: ConnectionGroupingMode) -> &'static str {
    match mode {
        ConnectionGroupingMode::Flat => "conn_group_flat",
        ConnectionGroupingMode::ByProcess => "conn_group_process",
        ConnectionGroupingMode::ByHost => "conn_group_host",
    }
}
pub fn sort_label_key(key: ConnectionSortKey) -> &'static str {
    match key {
        ConnectionSortKey::DownloadDesc => "runtime_conn_sort_download_desc",
        ConnectionSortKey::UploadDesc => "runtime_conn_sort_upload_desc",
        ConnectionSortKey::DownloadRateDesc => "runtime_conn_sort_download_rate",
        ConnectionSortKey::UploadRateDesc => "runtime_conn_sort_upload_rate",
        ConnectionSortKey::LatestDesc => "runtime_conn_sort_latest_desc",
        ConnectionSortKey::HostAsc => "runtime_conn_sort_host_asc",
    }
}

#[cfg(test)]
mod tests;
