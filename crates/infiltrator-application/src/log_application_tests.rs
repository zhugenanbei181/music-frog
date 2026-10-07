//! test-intent: behavior
use crate::command_application::{CommandApplication, CommandHandler};
use crate::log_application::{LOG_CAPACITY, LogApplication};
use crate::log_projection::parse_structured_log;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::{LogLevel, LogSession, LogStreamState};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;

fn core(session: LogSession) -> CoreSnapshot {
    CoreSnapshot {
        lifecycle: CoreLifecycle::Running,
        generation: session.generation,
        session_token: Some(session.token),
        revision: 0,
        proxy_mode: None,
        core_version: None,
        sampled_at_epoch_ms: None,
        failure: None,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
        watchdog: Default::default(),
    }
}
fn session(generation: u64, token: u128) -> LogSession {
    LogSession {
        generation,
        token: SessionToken::new(token),
    }
}
#[test]
fn connected_empty_is_observed_while_connecting_has_no_fabricated_facts() {
    let logs = LogApplication::default();
    let scope = session(1, 11);
    logs.bind(Some(scope));
    let snapshot = core(scope);
    assert_eq!(logs.page(&snapshot).status, PageStatus::Loading);
    assert!(logs.page(&snapshot).data.is_none());
    assert!(logs.ingest(scope, RuntimeStreamEvent::Connected));
    let page = logs.page(&snapshot);
    assert_eq!(page.status, PageStatus::Empty);
    let data = page.data.expect("observed empty buffer");
    assert_eq!(data.total_entries, 0);
    assert_eq!(data.stream, LogStreamState::Live);
}
#[test]
fn bounded_buffer_retains_original_records_and_monotonic_row_identity() {
    let logs = LogApplication::default();
    let scope = session(2, 12);
    logs.bind(Some(scope));
    for index in 0..LOG_CAPACITY + 2 {
        assert!(logs.ingest(
            scope,
            RuntimeStreamEvent::Item(format!("INFO[{index}] row {index}"))
        ));
    }
    let data = logs.page(&core(scope)).data.unwrap();
    assert_eq!(data.total_entries, LOG_CAPACITY);
    assert_eq!(data.entries.first().unwrap().id, 3);
    assert_eq!(
        data.entries.first().unwrap().raw.as_deref(),
        Some("INFO[2] row 2")
    );
    assert_eq!(
        data.entries.last().unwrap().message,
        format!("row {}", LOG_CAPACITY + 1)
    );
    logs.clear();
    assert_eq!(logs.page(&core(scope)).status, PageStatus::Empty);
    assert!(logs.ingest(scope, RuntimeStreamEvent::Item("WARN[3] new row".into())));
    let data = logs.page(&core(scope)).data.unwrap();
    assert_eq!(data.entries[0].id, (LOG_CAPACITY + 3) as u64);
    assert_eq!(data.entries[0].level, "WARN");
}
#[test]
fn token_and_generation_both_fence_old_events_and_failures() {
    let logs = LogApplication::default();
    let original = session(1, 11);
    logs.bind(Some(original));
    logs.ingest(original, RuntimeStreamEvent::Item("INFO[1] old".into()));
    let replacement = session(1, 12);
    logs.bind(Some(replacement));
    assert!(!logs.ingest(original, RuntimeStreamEvent::Item("ERROR[1] stale".into())));
    logs.fail(
        original,
        Failure::new(ErrorCode::Permission, "old failure", false),
    );
    assert_eq!(logs.page(&core(replacement)).status, PageStatus::Loading);
    assert!(logs.page(&core(replacement)).data.is_none());
    logs.ingest(
        replacement,
        RuntimeStreamEvent::Item("DEBUG[1] replacement".into()),
    );
    assert!(logs.page(&core(original)).data.is_none());
    let next = session(2, 12);
    logs.bind(Some(next));
    assert!(!logs.ingest(replacement, RuntimeStreamEvent::Connected));
    assert!(logs.page(&core(next)).data.is_none());
}
#[test]
fn same_generation_failure_and_stop_preserve_observed_records() {
    let logs = LogApplication::default();
    let scope = session(3, 15);
    logs.bind(Some(scope));
    logs.ingest(scope, RuntimeStreamEvent::Item("INFO[1] retained".into()));
    let failure = Failure::new(ErrorCode::Permission, "controller access denied", false);
    logs.fail(scope, failure.clone());
    let failed = logs.page(&core(scope));
    assert_eq!(
        failed.status,
        PageStatus::Failed {
            failure: failure.clone()
        }
    );
    assert_eq!(failed.data.as_ref().unwrap().entries[0].message, "retained");
    assert_eq!(failed.data.unwrap().stream, LogStreamState::Failed(failure));
    logs.bind(None);
    let mut stopped = core(scope);
    stopped.lifecycle = CoreLifecycle::Stopped;
    stopped.session_token = None;
    let stopped_page = logs.page(&stopped);
    assert!(matches!(
        stopped_page.status,
        PageStatus::Unavailable { .. }
    ));
    assert_eq!(stopped_page.data.unwrap().entries[0].message, "retained");
    assert!(!logs.ingest(scope, RuntimeStreamEvent::Item("late".into())));
    logs.bind(Some(scope));
    assert_eq!(logs.page(&core(scope)).data.unwrap().entries.len(), 1);
}
#[tokio::test]
async fn commands_mutate_the_reader_owner_and_invalid_filters_have_no_effect() {
    let logs = LogApplication::default();
    let scope = session(1, 11);
    logs.bind(Some(scope));
    logs.ingest(scope, RuntimeStreamEvent::Item("WARN[1] retained".into()));
    logs.ingest(scope, RuntimeStreamEvent::Item("INFO[2] filtered".into()));
    let commands = CommandApplication::new().with_logs(logs.clone());
    commands
        .handle(CommandIntent::SetLogLevelFilter {
            level: Some("warning".into()),
        })
        .await
        .unwrap();
    assert_eq!(
        logs.page(&core(scope))
            .data
            .unwrap()
            .active_level
            .as_deref(),
        Some("WARN")
    );
    let filtered = logs.page(&core(scope)).data.unwrap();
    assert_eq!(filtered.total_entries, 2);
    assert_eq!(filtered.entries.len(), 1);
    assert_eq!(filtered.entries[0].message, "retained");
    let failure = commands
        .handle(CommandIntent::SetLogLevelFilter {
            level: Some("pretend".into()),
        })
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(
        logs.page(&core(scope))
            .data
            .unwrap()
            .active_level
            .as_deref(),
        Some("WARN")
    );
    commands.handle(CommandIntent::ClearLogs).await.unwrap();
    assert_eq!(logs.page(&core(scope)).status, PageStatus::Empty);
    assert_eq!(
        logs.page(&core(scope))
            .data
            .unwrap()
            .active_level
            .as_deref(),
        Some("WARN")
    );
}
#[test]
fn severity_comes_from_reported_metadata_not_words_inside_the_message() {
    assert_eq!(
        parse_structured_log("information about error handling").level,
        LogLevel::Unknown
    );
    assert_eq!(
        parse_structured_log("a WARN word and ERROR sample").level,
        LogLevel::Unknown
    );
    assert_eq!(
        parse_structured_log(r#"{"type":"unknown","payload":"ERROR[1] quoted"}"#).level,
        LogLevel::Unknown
    );
    assert_eq!(
        parse_structured_log(r#"{"type":"debug","payload":"[TCP] warning in message"}"#).level,
        LogLevel::Debug
    );
    assert_eq!(
        parse_structured_log("[WRN] native severity").level,
        LogLevel::Warn
    );
}
