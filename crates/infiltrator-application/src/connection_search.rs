//! One match and text presentation fold for live and projected connection facts.
use crate::search_text::project_text_runs;
use infiltrator_contract::connection_search::ConnectionSearchRow;
use infiltrator_domain::connection_view::{self, ConnectionView};

pub fn display_endpoint(host: &str, port: &str) -> String {
    if port.is_empty() {
        host.into()
    } else if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}
pub fn project_search_row<C: ConnectionView>(row: &C, query: &str) -> ConnectionSearchRow {
    let query = query.trim();
    let endpoint = display_endpoint(
        connection_view::connection_host(row),
        row.view_destination_port(),
    );
    let process = connection_view::process_display_name(row.view_process_path());
    let process_rule = if row.view_rule().is_empty() {
        process.clone()
    } else {
        format!("{process} · {}", row.view_rule())
    };
    let matched = connection_view::matching_term(row, query);
    let visible = query.is_empty() || matched.is_some();
    let matched_term = matched
        .map(|term| project_text_runs(term, query))
        .unwrap_or_default();
    let endpoint = project_text_runs(&endpoint, query);
    let process_rule = project_text_runs(&process_rule, query);
    let matched_term = if endpoint
        .iter()
        .chain(&process_rule)
        .any(|run| run.highlighted)
    {
        Vec::new()
    } else {
        matched_term
    };
    ConnectionSearchRow {
        visible,
        endpoint,
        process_name: project_text_runs(&process, query),
        process_rule,
        matched_term,
    }
}

#[cfg(test)]
mod tests;
