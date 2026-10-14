//! One match and text presentation fold for live and projected connection facts.
use infiltrator_contract::connection_search::ConnectionSearchRow;
pub use infiltrator_domain::connection_view::display_endpoint;
use infiltrator_domain::connection_view::{self, ConnectionView};
use zhuge_ui_core::text::{CaseSensitivity, project_text_runs};
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
        .map(|term| project_text_runs(term, query, CaseSensitivity::UnicodeInsensitive))
        .unwrap_or_default();
    let endpoint = project_text_runs(&endpoint, query, CaseSensitivity::UnicodeInsensitive);
    let process_rule = project_text_runs(&process_rule, query, CaseSensitivity::UnicodeInsensitive);
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
        process_name: project_text_runs(&process, query, CaseSensitivity::UnicodeInsensitive),
        process_rule,
        matched_term,
    }
}
