//! test-intent: behavior
use crate::log_page_model::{
    LogLevelFilter, LogLevelSelection, LogPageStatus, LogsPageModel, project_log_line_sections,
    project_log_wrap_metrics,
};
use crate::log_search::LogSearchOutcome;
use infiltrator_contract::logs::{LogLevel, LogStreamState};
use infiltrator_contract::surface_snapshot::{LogSnapshot, LogsPageSnapshot};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::{PageData, PageStatus};
use lurker_core_contract::session::SessionToken;

fn entry(id: u64, level: &str, message: &str) -> LogSnapshot {
    LogSnapshot {
        id,
        raw: None,
        timestamp: "2026-10-20T00:00:00Z".into(),
        level: level.into(),
        tag: "TCP".into(),
        message: message.into(),
    }
}

fn ready_page(entries: Vec<LogSnapshot>, active_level: Option<&str>) -> PageData<LogsPageSnapshot> {
    let total = entries.len();
    PageData {
        status: if entries.is_empty() {
            PageStatus::Empty
        } else {
            PageStatus::Ready
        },
        data: Some(LogsPageSnapshot {
            stream: LogStreamState::Live,
            total_entries: total,
            active_level: active_level.map(|level| level.to_string()),
            entries,
        }),
    }
}

#[test]
fn one_observation_drives_every_fact_the_logs_page_renders() {
    let mut model = LogsPageModel::default();
    assert_eq!(model.status(), LogPageStatus::Unobserved);
    assert_eq!(model.total_entries(), 0);
    assert_eq!(model.generation(), None);
    assert!(model.rows().is_empty());

    model.observe(
        7,
        Some(SessionToken::new(3)),
        &ready_page(
            vec![entry(1, "INFO", "hello"), entry(2, "ERROR", "boom")],
            None,
        ),
    );

    assert_eq!(model.status(), LogPageStatus::Ready);
    assert_eq!(model.stream(), LogStreamState::Live);
    assert_eq!(model.total_entries(), 2);
    assert_eq!(model.generation(), Some(7));
    assert_eq!(model.session_token(), Some(SessionToken::new(3)));
    assert_eq!(model.rows().len(), 2);
    assert_eq!(model.source_count(), 2);
    assert_eq!(model.matched_count(), 2);
    assert_eq!(model.status_key(), "logs_search_results");
    assert_eq!(model.level_filter(), LogLevelFilter::All);
    assert_eq!(model.active_level(), None);
}

#[test]
fn the_level_filter_is_typed_and_keeps_the_authority_it_came_from() {
    let mut model = LogsPageModel::default();

    // A surface may select a level before an owner confirmed it.
    model.select_level(LogLevelFilter::Level(LogLevel::Warn));
    assert_eq!(
        model.level_selection(),
        LogLevelSelection::Selected(LogLevel::Warn)
    );
    assert_eq!(model.active_level(), Some(LogLevel::Warn));

    // The owner's readback replaces it with the confirmed selection.
    model.observe_level(LogLevelFilter::Level(LogLevel::Error));
    assert_eq!(
        model.level_selection(),
        LogLevelSelection::Confirmed(LogLevel::Error)
    );
    assert_eq!(model.active_level(), Some(LogLevel::Error));

    // An identifier that names no level is not a level filter at all: the
    // illegal "a level that is not a level" can no longer be expressed.
    assert_eq!(
        LogLevelFilter::from_identifier("pretend"),
        LogLevelFilter::All
    );
    assert_eq!(LogLevelFilter::from_identifier(""), LogLevelFilter::All);
    assert_eq!(LogLevelFilter::from_active_level(None), LogLevelFilter::All);
    assert_eq!(
        LogLevelFilter::from_active_level(Some("WARN")),
        LogLevelFilter::Level(LogLevel::Warn)
    );
    assert_eq!(
        LogLevelFilter::from_active_level(Some("unknown")),
        LogLevelFilter::Level(LogLevel::Unknown)
    );
    assert_eq!(
        LogLevelFilter::Level(LogLevel::Warn)
            .identifier()
            .as_deref(),
        Some("WARN")
    );
    assert_eq!(LogLevelFilter::All.identifier(), None);
}

#[test]
fn a_record_observation_never_drops_the_chosen_level_filter() {
    let mut model = LogsPageModel::default();
    model.select_level(LogLevelFilter::Level(LogLevel::Warn));

    // A standalone log stream observes records without an owner; that must
    // not silently reset the level the user chose.
    model.observe(0, None, &ready_page(vec![entry(1, "WARN", "x")], None));

    assert_eq!(model.active_level(), Some(LogLevel::Warn));
    assert_eq!(
        model.level_selection(),
        LogLevelSelection::Selected(LogLevel::Warn)
    );
}

#[test]
fn the_search_outcome_separates_a_broken_pattern_from_an_empty_result() {
    let mut model = LogsPageModel::default();
    model.observe(1, None, &ready_page(vec![entry(1, "INFO", "hello")], None));

    model.edit_query("(");
    assert!(matches!(
        model.search_outcome(),
        LogSearchOutcome::InvalidPattern { .. }
    ));
    assert_eq!(model.status_key(), "logs_search_invalid");
    assert!(model.invalid_pattern().is_some());

    model.edit_query("absent");
    assert_eq!(
        model.search_outcome(),
        LogSearchOutcome::NoMatches {
            matched: 0,
            source: 1
        }
    );
    assert_eq!(model.status_key(), "logs_search_no_matches");

    model.edit_query("");
    assert_eq!(
        model.search_outcome(),
        LogSearchOutcome::Results {
            matched: 1,
            source: 1
        }
    );
    assert_eq!(model.status_key(), "logs_search_results");
}

#[test]
fn an_unavailable_page_keeps_its_reason_and_is_not_an_empty_result() {
    let mut model = LogsPageModel::default();
    let failure = Failure::new(ErrorCode::Permission, "denied", false);
    model.observe(
        1,
        None,
        &PageData {
            status: PageStatus::Failed {
                failure: failure.clone(),
            },
            data: None,
        },
    );

    assert_eq!(
        model.status(),
        LogPageStatus::Failed {
            failure: failure.clone()
        }
    );
    assert_eq!(model.status().failure(), Some(&failure));
    assert!(!model.status().is_readable());
    assert_eq!(model.search_outcome(), LogSearchOutcome::SourceUnavailable);
    assert_eq!(model.status_key(), "logs_search_unavailable");
}

#[test]
fn an_empty_page_reports_no_records_rather_than_no_matches() {
    let mut model = LogsPageModel::default();
    model.observe(1, None, &ready_page(Vec::new(), None));

    assert_eq!(model.status(), LogPageStatus::Empty);
    assert!(model.status().is_readable());
    assert_eq!(model.search_outcome(), LogSearchOutcome::NoRecords);
    assert_eq!(model.status_key(), "logs_no_realtime_records");
}

#[test]
fn a_log_line_is_projected_into_its_four_sections() {
    let mut model = LogsPageModel::default();
    model.observe(1, None, &ready_page(vec![entry(1, "info", "hello")], None));
    let row = model.row(1).expect("the record must be projected");
    let sections = project_log_line_sections(row);
    assert_eq!(sections.timestamp, "2026-10-20T00:00:00Z");
    assert_eq!(sections.level, "[INFO]");
    assert_eq!(sections.source, "[TCP]");
    assert_eq!(sections.message, "hello");
}

#[test]
fn a_long_record_wraps_into_more_than_one_line() {
    let short = project_log_wrap_metrics("abc", 10.0);
    assert_eq!(short.lines, 1);
    assert_eq!(short.extra_lines, 0);

    // A half-width character is half an em wide, so a record of 21 of them
    // is 10.5 em and needs two lines on a ten em line.
    let long = project_log_wrap_metrics("abcdefghijklmnopqrstu", 10.0);
    assert_eq!(long.lines, 2);
    assert_eq!(long.extra_lines, 1);

    // A full-width character counts twice as much as a half-width one.
    let wide = project_log_wrap_metrics("中中", 2.0);
    assert_eq!(wide.lines, 1);
    let wider = project_log_wrap_metrics("中中中", 2.0);
    assert_eq!(wider.lines, 2);

    // A line without a width keeps the record on one line.
    let narrow = project_log_wrap_metrics("abc", 0.0);
    assert_eq!(narrow.lines, 1);
}
