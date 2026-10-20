//! test-intent: behavior
//! The Iced surface consumes the shared page models: it renders the logs page
//! and the connections page from them and holds no second derivation of either
//! page state.
use crate::state::AppState;
use crate::types::message::Message;
use infiltrator_contract::logs::LogLevel;
use infiltrator_domain::connection_view::{ConnectionGroupingMode, ConnectionSortKey};
use infiltrator_domain::runtime::{Connection, ConnectionMetadata, ConnectionSnapshot};
use infiltrator_presentation::log_page_model::{LogLevelFilter, LogLevelSelection};
use infiltrator_presentation::log_search::LogSearchOutcome;

fn snapshot_with(count: usize) -> ConnectionSnapshot {
    ConnectionSnapshot {
        download_total: 0,
        upload_total: 0,
        connections: (0..count)
            .map(|index| Connection {
                id: index.to_string(),
                metadata: ConnectionMetadata::default(),
                upload: 0,
                download: 0,
                start: String::new(),
                rule: String::new(),
                rule_payload: String::new(),
                chains: Vec::new(),
            })
            .collect(),
    }
}

#[test]
fn the_logs_page_state_comes_from_the_shared_page_model() {
    let (mut state, _) = AppState::new();

    // The surface owns no private copy of the query: the shared model holds it.
    let _ = state.update(Message::UpdateLogRegexFilter("error|warn".into()));
    assert_eq!(state.diag.logs_page.query(), "error|warn");
    assert_eq!(state.diag.logs_page.level_filter(), LogLevelFilter::All);

    // A level that is not a level cannot be expressed: the message carries the
    // shared level enum, and the model keeps the typed filter together with
    // the authority it came from.
    let _ = state.update(Message::SetLogLevelFilter(Some(LogLevel::Warn)));
    assert_eq!(state.diag.logs_page.active_level(), Some(LogLevel::Warn));
    assert_eq!(
        state.diag.logs_page.level_selection(),
        LogLevelSelection::Selected(LogLevel::Warn)
    );
    assert_eq!(
        state.diag.logs_page.level_filter(),
        LogLevelFilter::Level(LogLevel::Warn)
    );
}

#[test]
fn a_broken_log_pattern_is_a_typed_outcome_and_not_an_empty_result() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::LogReceived("INFO[1] connected".into()));
    assert_eq!(state.diag.logs_page.source_count(), 1);

    let _ = state.update(Message::UpdateLogRegexFilter("(".into()));
    assert!(matches!(
        state.diag.logs_page.search_outcome(),
        LogSearchOutcome::InvalidPattern { .. }
    ));
    assert_eq!(state.diag.logs_page.status_key(), "logs_search_invalid");

    // A valid query that matches nothing is a different typed outcome.
    let _ = state.update(Message::UpdateLogRegexFilter("absent".into()));
    assert_eq!(
        state.diag.logs_page.search_outcome(),
        LogSearchOutcome::NoMatches {
            matched: 0,
            source: 1
        }
    );
    assert_eq!(state.diag.logs_page.status_key(), "logs_search_no_matches");
}

#[test]
fn the_connections_page_state_comes_from_the_shared_page_model() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::ConnectionsReceived(snapshot_with(3)));
    assert_eq!(state.diag.connections_model.source_count(), 3);
    assert!(state.diag.connections_model.source_current());

    // The grouping dimension and the sort key are the shared model's, not a
    // second copy inside the surface.
    let _ = state.update(Message::SetConnectionGroupingMode(
        ConnectionGroupingMode::ByProcess,
    ));
    assert_eq!(
        state.diag.connections_model.mode(),
        ConnectionGroupingMode::ByProcess
    );

    let _ = state.update(Message::UpdateRuntimeConnectionSort(
        ConnectionSortKey::HostAsc,
    ));
    assert_eq!(
        state.diag.connections_model.sort_key(),
        ConnectionSortKey::HostAsc
    );
    // The host order is the last position of the shared sort list.
    assert_eq!(state.diag.connections_model.sort_index(), 5);
}
