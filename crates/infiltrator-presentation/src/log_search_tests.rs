//! test-intent: behavior
use crate::log_search::LogSearchState;
use infiltrator_contract::logs::LogStreamState;
use infiltrator_contract::surface_snapshot::{LogSnapshot, LogsPageSnapshot, PageData, PageStatus};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::session::SessionToken;

fn observed_page(rows: &[(&str, &str, &str, &str)]) -> PageData<LogsPageSnapshot> {
    let entries: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(index, (level, timestamp, tag, message))| LogSnapshot {
            id: index as u64 + 1,
            raw: None,
            timestamp: (*timestamp).into(),
            level: (*level).into(),
            tag: (*tag).into(),
            message: (*message).into(),
        })
        .collect();
    let status = if entries.is_empty() {
        PageStatus::Empty
    } else {
        PageStatus::Ready
    };
    PageData {
        status,
        data: Some(LogsPageSnapshot {
            stream: LogStreamState::Live,
            total_entries: entries.len(),
            active_level: None,
            entries,
        }),
    }
}

#[test]
fn regex_alternation_case_and_original_graphemes_define_one_filter_and_highlight() {
    let mut search = LogSearchState::default();
    let page = observed_page(&[
        ("INFO", "1", "", "e\u{301} api.Example 👩‍👩‍👧‍👦"),
        ("ERROR", "2", "", "dial REFUSED"),
        ("DEBUG", "3", "", "healthy"),
    ]);
    search.observe(1, Some(SessionToken::new(1)), &page);
    search.edit_query("api\\.example|refused|e|👩");
    assert_eq!(search.source_count(), 3);
    assert_eq!(search.matched_count(), 3); // DEBUG's reported level also contains e.
    let first = search.row(1).unwrap();
    assert_eq!(
        first
            .message
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>(),
        "e\u{301} api.Example 👩‍👩‍👧‍👦"
    );
    let highlights: Vec<_> = first
        .message
        .iter()
        .filter(|run| run.highlighted)
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(highlights, ["e\u{301}", "api.Example", "👩‍👩‍👧‍👦"]);
    assert!(
        search
            .row(2)
            .unwrap()
            .message
            .iter()
            .any(|run| run.highlighted && run.text == "REFUSED")
    );
    search.edit_query("api\\.example|refused");
    assert_eq!(search.matched_count(), 2);
    assert!(!search.row(3).unwrap().visible);
}
#[test]
fn invalid_pattern_has_a_real_error_and_is_distinct_from_no_matches_or_no_observation() {
    let mut search = LogSearchState::default();
    search.observe(1, None, &observed_page(&[("INFO", "1", "", "observed")]));
    search.edit_query("(");
    assert!(search.invalid_pattern().unwrap().contains("unclosed"));
    assert_eq!(search.status_key(), "logs_search_invalid");
    assert_eq!(search.matched_count(), 0);
    assert!(!search.row(1).unwrap().visible);
    search.edit_query("absent");
    assert!(search.invalid_pattern().is_none());
    assert_eq!(search.status_key(), "logs_search_no_matches");
    search.edit_query("");
    assert_eq!(search.matched_count(), 1);
    assert_eq!(search.status_key(), "logs_search_results");
    let data = observed_page(&[("INFO", "1", "", "observed")]).data;
    search.observe(
        1,
        None,
        &PageData {
            status: PageStatus::Failed {
                failure: Failure::new(ErrorCode::Permission, "denied", false),
            },
            data,
        },
    );
    assert!(!search.source_current());
    assert_eq!(search.matched_count(), 1);
    assert_eq!(search.status_key(), "logs_search_unavailable");
}
#[test]
fn observed_empty_and_source_change_preserve_query_without_reusing_old_records() {
    let mut search = LogSearchState::default();
    search.edit_query("timeout");
    search.observe(
        1,
        Some(SessionToken::new(1)),
        &observed_page(&[("ERROR", "1", "", "timeout")]),
    );
    assert_eq!(search.matched_count(), 1);
    search.observe(1, Some(SessionToken::new(2)), &PageData::loading());
    assert!(search.rows().is_empty());
    assert_eq!(search.status_key(), "logs_search_unavailable");
    assert_eq!(search.query(), "timeout");
    search.observe(2, Some(SessionToken::new(3)), &observed_page(&[]));
    assert!(search.source_current());
    assert_eq!(search.status_key(), "logs_no_realtime_records");
    assert_eq!(search.query(), "timeout");
    search.observe(
        2,
        Some(SessionToken::new(3)),
        &observed_page(&[("WARN", "2", "", "timeOUT again")]),
    );
    assert_eq!(search.matched_count(), 1);
    assert_eq!(
        search
            .row(1)
            .unwrap()
            .message
            .iter()
            .filter(|run| run.highlighted)
            .map(|run| run.text.as_str())
            .collect::<String>(),
        "timeOUT"
    );
}
