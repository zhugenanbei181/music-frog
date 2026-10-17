//! test-intent: behavior
use crate::command_application::CommandApplication;
use crate::command_application::handler::CommandHandler;
use crate::core_capability_test_support::{execute_core_command, with_test_capabilities};
use crate::log_application::{LOG_CAPACITY, LogApplication};
use crate::log_application::{LogBindingIntent, LogBindingOutcome, LogIngestOutcome};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::LogLevel;
use infiltrator_contract::logs::{
    LogBindingSnapshot, LogObservationSnapshot, LogSession, LogStreamState,
};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_ports::runtime_stream::RuntimeStreamEvent;

fn bind(logs: &LogApplication, session: LogSession) {
    assert_eq!(
        logs.bind(LogBindingIntent::Bind(session)),
        LogBindingOutcome::Bound
    );
}

fn unbind(logs: &LogApplication) {
    assert_eq!(
        logs.bind(LogBindingIntent::Unbind),
        LogBindingOutcome::Bound
    );
}

fn append(logs: &LogApplication, session: LogSession, raw: impl Into<String>) {
    assert_eq!(
        logs.ingest(session, RuntimeStreamEvent::Item(raw.into())),
        LogIngestOutcome::Applied
    );
}

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
fn connecting_and_observed_empty_are_distinct_typed_facts() {
    let logs = LogApplication::default();
    let scope = session(1, 11);
    bind(&logs, scope.clone());
    let facts = logs.facts();
    assert_eq!(
        facts.binding,
        LogBindingSnapshot::StandaloneBound(scope.clone())
    );
    assert_eq!(facts.observation, LogObservationSnapshot::Connecting);

    assert_eq!(
        logs.ingest(scope.clone(), RuntimeStreamEvent::Connected),
        LogIngestOutcome::Applied
    );
    let facts = logs.facts();
    assert!(matches!(
        facts.observation,
        LogObservationSnapshot::Observed {
            session,
            stream: LogStreamState::Live,
            records,
        } if session == scope && records.is_empty()
    ));
}

#[test]
fn bounded_buffer_keeps_raw_records_and_monotonic_identity() {
    let logs = LogApplication::default();
    let scope = session(2, 12);
    bind(&logs, scope.clone());
    for index in 0..LOG_CAPACITY + 2 {
        append(&logs, scope.clone(), format!("INFO[{index}] row {index}"));
    }
    let facts = logs.facts();
    let LogObservationSnapshot::Observed { records, .. } = facts.observation else {
        panic!("raw buffer should be observed")
    };
    assert_eq!(records.len(), LOG_CAPACITY);
    assert_eq!(records.first().unwrap().id, 3);
    assert_eq!(records.first().unwrap().raw, "INFO[2] row 2");
    assert_eq!(
        records.last().unwrap().raw,
        format!("INFO[{}] row {}", LOG_CAPACITY + 1, LOG_CAPACITY + 1)
    );

    logs.clear();
    let facts = logs.facts();
    assert!(
        matches!(facts.observation, LogObservationSnapshot::Observed { records, .. } if records.is_empty())
    );
    append(&logs, scope.clone(), "WARN[3] new row");
    let facts = logs.facts();
    assert!(
        matches!(facts.observation, LogObservationSnapshot::Observed { records, .. } if records[0].id == (LOG_CAPACITY + 3) as u64 && records[0].raw == "WARN[3] new row")
    );
}

#[test]
fn token_generation_and_engine_identity_fence_old_stream_events() {
    let logs = LogApplication::default();
    let original = session(1, 11);
    bind(&logs, original.clone());
    append(&logs, original.clone(), "INFO[1] old");
    let replacement = session(1, 12);
    bind(&logs, replacement.clone());
    assert_eq!(
        logs.ingest(
            original.clone(),
            RuntimeStreamEvent::Item("ERROR[1] stale".into())
        ),
        LogIngestOutcome::RejectedForeignSession
    );
    assert_eq!(
        logs.fail(
            original.clone(),
            Failure::new(ErrorCode::Permission, "old failure", false),
        ),
        LogIngestOutcome::RejectedForeignSession
    );
    assert_eq!(logs.facts().observation, LogObservationSnapshot::Connecting);
    append(&logs, replacement.clone(), "DEBUG[1] replacement");
    assert!(
        matches!(logs.facts().observation, LogObservationSnapshot::Observed { session, .. } if session == replacement)
    );

    let previous_engine = identity_bound_session("mihomo", "desktop-main", 4, 41);
    let current_engine = identity_bound_session("sing-box", "desktop-main", 4, 41);
    logs.observe_core(&core(&previous_engine));
    append(&logs, previous_engine.clone(), "INFO[2] previous engine");
    logs.observe_core(&core(&current_engine));
    assert_eq!(
        logs.ingest(
            previous_engine,
            RuntimeStreamEvent::Item("ERROR[3] stale engine record".into())
        ),
        LogIngestOutcome::RejectedForeignSession
    );
    let facts = logs.facts();
    assert_eq!(facts.binding, LogBindingSnapshot::CoreBound(current_engine));
    assert_eq!(facts.observation, LogObservationSnapshot::Connecting);
}

#[test]
fn core_owner_binding_uses_typed_confirmation_and_rejection() {
    let logs = LogApplication::default();
    let current = session(7, 71);
    let foreign = session(7, 72);
    logs.observe_core(&core(&current));

    assert_eq!(
        logs.bind(LogBindingIntent::Bind(current)),
        LogBindingOutcome::CoreOwnerConfirmed
    );
    assert_eq!(
        logs.bind(LogBindingIntent::Bind(foreign)),
        LogBindingOutcome::RejectedByCoreOwner
    );
}

#[test]
fn failed_and_stopped_streams_preserve_scoped_raw_observations() {
    let logs = LogApplication::default();
    let scope = session(3, 15);
    bind(&logs, scope.clone());
    append(&logs, scope.clone(), "INFO[1] retained");
    let failure = Failure::new(ErrorCode::Permission, "controller access denied", false);
    assert_eq!(
        logs.fail(scope.clone(), failure.clone()),
        LogIngestOutcome::Applied
    );
    assert!(matches!(
        logs.facts().observation,
        LogObservationSnapshot::Observed {
            stream: LogStreamState::Failed(found),
            records,
            ..
        } if found == failure && records[0].raw == "INFO[1] retained"
    ));

    unbind(&logs);
    let facts = logs.facts();
    assert_eq!(facts.binding, LogBindingSnapshot::StandaloneUnbound);
    assert!(matches!(
        facts.observation,
        LogObservationSnapshot::Observed {
            stream: LogStreamState::Idle,
            records,
            ..
        } if records[0].raw == "INFO[1] retained"
    ));
    assert_eq!(
        logs.ingest(scope.clone(), RuntimeStreamEvent::Item("late".into())),
        LogIngestOutcome::RejectedForeignSession
    );
    bind(&logs, scope);
    assert!(
        matches!(logs.facts().observation, LogObservationSnapshot::Observed { stream: LogStreamState::Connecting, records, .. } if records.len() == 1)
    );
}

#[tokio::test]
async fn filter_changes_only_display_facts_and_clear_keeps_observation_semantics() {
    let logs = LogApplication::default();
    let scope = session(1, 11);
    bind(&logs, scope.clone());
    append(&logs, scope.clone(), "WARN[1] retained");
    append(&logs, scope.clone(), "INFO[2] filtered");
    let commands = with_test_capabilities(CommandApplication::new().with_logs(logs.clone()));
    commands
        .handle(CommandIntent::SetLogLevelFilter {
            level: Some("warning".into()),
        })
        .await
        .unwrap();
    let facts = logs.facts();
    assert_eq!(facts.active_level, Some(LogLevel::Warn));
    assert!(
        matches!(facts.observation, LogObservationSnapshot::Observed { records, .. } if records.len() == 2)
    );

    let failure = commands
        .handle(CommandIntent::SetLogLevelFilter {
            level: Some("pretend".into()),
        })
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(logs.facts().active_level, Some(LogLevel::Warn));

    execute_core_command(&commands, CommandIntent::ClearLogs)
        .await
        .unwrap();
    assert!(
        matches!(logs.facts().observation, LogObservationSnapshot::Observed { records, .. } if records.is_empty())
    );
    assert_eq!(logs.facts().active_level, Some(LogLevel::Warn));
}
