//! test-intent: behavior
use crate::command_application::CommandApplication;
use crate::command_application::handler::CommandHandler;
use crate::core_capability_test_support::{execute_core_command, with_test_capabilities};
use crate::log_application::{LOG_CAPACITY, LogApplication};
use crate::log_projection::parse_structured_log;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::{LogLevel, LogSession, LogStreamState};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_ports::runtime_stream::RuntimeStreamEvent;

fn core(session: &LogSession) -> CoreSnapshot {
    CoreSnapshot {
        identity: session.identity().cloned(),
        lifecycle: CoreLifecycle::Running,
        generation: session.generation.value(),
        session_token: Some(session.token),
        revision: 0,
        routing_intent: Default::default(),
        core_version: None,
        sampled_at_epoch_ms: None,
        failure: None,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        watchdog: Default::default(),
    }
}
fn session(generation: u64, token: u128) -> LogSession {
    identity_bound_session("mihomo", "test-instance", generation, token)
}

fn identity_bound_session(
    engine: &str,
    instance: &str,
    generation: u64,
    token: u128,
) -> LogSession {
    LogSession::from_core_scope(CoreScope {
        identity: CoreIdentity::new(
            EngineId::new(engine).expect("valid test engine"),
            CoreInstanceId::new(instance).expect("valid test instance"),
            Some(CoreVersion::new("test-version").expect("valid test version")),
        ),
        generation: CoreGeneration::new(generation),
        session_token: Some(SessionToken::new(token)),
    })
    .expect("valid identity-bound test log session")
}
#[test]
fn connected_empty_is_observed_while_connecting_has_no_fabricated_facts() {
    let logs = LogApplication::default();
    let scope = session(1, 11);
    logs.bind(Some(scope.clone()));
    let snapshot = core(&scope);
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
    logs.bind(Some(scope.clone()));
    for index in 0..LOG_CAPACITY + 2 {
        assert!(logs.ingest(
            scope.clone(),
            RuntimeStreamEvent::Item(format!("INFO[{index}] row {index}"))
        ));
    }
    let data = logs.page(&core(&scope)).data.unwrap();
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
    assert_eq!(logs.page(&core(&scope)).status, PageStatus::Empty);
    assert!(logs.ingest(
        scope.clone(),
        RuntimeStreamEvent::Item("WARN[3] new row".into())
    ));
    let data = logs.page(&core(&scope)).data.unwrap();
    assert_eq!(data.entries[0].id, (LOG_CAPACITY + 3) as u64);
    assert_eq!(data.entries[0].level, "WARN");
}
#[test]
fn token_and_generation_both_fence_old_events_and_failures() {
    let logs = LogApplication::default();
    let original = session(1, 11);
    logs.bind(Some(original.clone()));
    logs.ingest(
        original.clone(),
        RuntimeStreamEvent::Item("INFO[1] old".into()),
    );
    let replacement = session(1, 12);
    logs.bind(Some(replacement.clone()));
    assert!(!logs.ingest(
        original.clone(),
        RuntimeStreamEvent::Item("ERROR[1] stale".into())
    ));
    logs.fail(
        original.clone(),
        Failure::new(ErrorCode::Permission, "old failure", false),
    );
    assert_eq!(logs.page(&core(&replacement)).status, PageStatus::Loading);
    assert!(logs.page(&core(&replacement)).data.is_none());
    logs.ingest(
        replacement.clone(),
        RuntimeStreamEvent::Item("DEBUG[1] replacement".into()),
    );
    assert!(logs.page(&core(&original)).data.is_none());
    let next = session(2, 12);
    logs.bind(Some(next.clone()));
    assert!(!logs.ingest(replacement, RuntimeStreamEvent::Connected));
    assert!(logs.page(&core(&next)).data.is_none());
}

#[test]
fn engine_identity_fences_delayed_logs_even_when_generation_and_token_match() {
    let logs = LogApplication::default();
    let previous = identity_bound_session("mihomo", "desktop-main", 4, 41);
    let current = identity_bound_session("sing-box", "desktop-main", 4, 41);
    logs.observe_core(&core(&previous));
    assert!(logs.ingest(
        previous.clone(),
        RuntimeStreamEvent::Item("INFO[1] previous engine".into())
    ));

    logs.observe_core(&core(&current));

    assert!(!logs.ingest(
        previous,
        RuntimeStreamEvent::Item("ERROR[2] stale engine record".into())
    ));
    let page = logs.page(&core(&current));
    assert_eq!(page.status, PageStatus::Loading);
    assert!(page.data.is_none());
}
#[test]
fn same_generation_failure_and_stop_preserve_observed_records() {
    let logs = LogApplication::default();
    let scope = session(3, 15);
    logs.bind(Some(scope.clone()));
    logs.ingest(
        scope.clone(),
        RuntimeStreamEvent::Item("INFO[1] retained".into()),
    );
    let failure = Failure::new(ErrorCode::Permission, "controller access denied", false);
    logs.fail(scope.clone(), failure.clone());
    let failed = logs.page(&core(&scope));
    assert_eq!(
        failed.status,
        PageStatus::Failed {
            failure: failure.clone()
        }
    );
    assert_eq!(failed.data.as_ref().unwrap().entries[0].message, "retained");
    assert_eq!(failed.data.unwrap().stream, LogStreamState::Failed(failure));
    logs.bind(None);
    let mut stopped = core(&scope);
    stopped.lifecycle = CoreLifecycle::Stopped;
    stopped.session_token = None;
    let stopped_page = logs.page(&stopped);
    assert!(matches!(
        stopped_page.status,
        PageStatus::Unavailable { .. }
    ));
    assert_eq!(stopped_page.data.unwrap().entries[0].message, "retained");
    assert!(!logs.ingest(scope.clone(), RuntimeStreamEvent::Item("late".into())));
    logs.bind(Some(scope.clone()));
    assert_eq!(logs.page(&core(&scope)).data.unwrap().entries.len(), 1);
}
#[tokio::test]
async fn commands_mutate_the_reader_owner_and_invalid_filters_have_no_effect() {
    let logs = LogApplication::default();
    let scope = session(1, 11);
    logs.bind(Some(scope.clone()));
    logs.ingest(
        scope.clone(),
        RuntimeStreamEvent::Item("WARN[1] retained".into()),
    );
    logs.ingest(
        scope.clone(),
        RuntimeStreamEvent::Item("INFO[2] filtered".into()),
    );
    let commands = with_test_capabilities(CommandApplication::new().with_logs(logs.clone()));
    commands
        .handle(CommandIntent::SetLogLevelFilter {
            level: Some("warning".into()),
        })
        .await
        .unwrap();
    assert_eq!(
        logs.page(&core(&scope))
            .data
            .unwrap()
            .active_level
            .as_deref(),
        Some("WARN")
    );
    let filtered = logs.page(&core(&scope)).data.unwrap();
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
        logs.page(&core(&scope))
            .data
            .unwrap()
            .active_level
            .as_deref(),
        Some("WARN")
    );
    execute_core_command(&commands, CommandIntent::ClearLogs)
        .await
        .unwrap();
    assert_eq!(logs.page(&core(&scope)).status, PageStatus::Empty);
    assert_eq!(
        logs.page(&core(&scope))
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
