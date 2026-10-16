//! Behavior tests for scoped raw-log display projection.
use std::iter::empty;
use super::{page_from_log_records, parse_structured_log, project_log_facts};
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use infiltrator_contract::logs::{
    LogBindingSnapshot, LogFactsSnapshot, LogLevel, LogObservationSnapshot, LogSession,
    LogStreamState, RawLogRecord,
};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_contract::surface_snapshot::PageStatus;

fn session(generation: u64, token: u128) -> LogSession {
    LogSession::from_core_scope(CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("valid engine"),
            CoreInstanceId::new("projection-test").expect("valid instance"),
            None,
        ),
        generation: CoreGeneration::new(generation),
        session_token: Some(SessionToken::new(token)),
    })
    .expect("valid log session")
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

#[test]
fn severity_uses_reported_metadata_not_words_inside_the_message() {
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

#[test]
fn projection_filters_display_rows_but_preserves_raw_total_and_stream_facts() {
    let session = session(2, 12);
    let mut core = core(&session);
    let facts = LogFactsSnapshot {
        binding: LogBindingSnapshot::CoreBound(session.clone()),
        observation: LogObservationSnapshot::Observed {
            session,
            stream: LogStreamState::Live,
            records: vec![
                RawLogRecord {
                    id: 8,
                    raw: "WARN[1] retained".into(),
                },
                RawLogRecord {
                    id: 9,
                    raw: "INFO[2] filtered".into(),
                },
            ],
        },
        active_level: Some(LogLevel::Warn),
    };
    let page = project_log_facts(&core, &facts);
    assert_eq!(page.status, PageStatus::Ready);
    let data = page.data.expect("observed log records");
    assert_eq!(data.total_entries, 2);
    assert_eq!(data.entries.len(), 1);
    assert_eq!(data.entries[0].id, 8);
    assert_eq!(data.entries[0].raw.as_deref(), Some("WARN[1] retained"));
    assert_eq!(data.active_level.as_deref(), Some("WARN"));

    core.lifecycle = CoreLifecycle::Stopped;
    core.session_token = None;
    let historical = project_log_facts(
        &core,
        &LogFactsSnapshot {
            binding: LogBindingSnapshot::CoreUnbound,
            ..facts
        },
    );
    assert!(matches!(historical.status, PageStatus::Unavailable { .. }));
    assert_eq!(historical.data.unwrap().total_entries, 2);
}

#[test]
fn current_scope_rejects_foreign_observation_and_connecting_is_not_empty() {
    let current = session(3, 30);
    let foreign = session(3, 31);
    let facts = LogFactsSnapshot {
        binding: LogBindingSnapshot::CoreBound(current.clone()),
        observation: LogObservationSnapshot::Observed {
            session: foreign,
            stream: LogStreamState::Live,
            records: vec![RawLogRecord {
                id: 1,
                raw: "ERROR[1] foreign".into(),
            }],
        },
        active_level: None,
    };
    let page = project_log_facts(&core(&current), &facts);
    assert_eq!(page.status, PageStatus::Loading);
    assert!(page.data.is_none());

    let connecting = LogFactsSnapshot {
        binding: LogBindingSnapshot::CoreBound(current.clone()),
        observation: LogObservationSnapshot::Connecting,
        active_level: None,
    };
    let page = project_log_facts(&core(&current), &connecting);
    assert_eq!(page.status, PageStatus::Loading);
    assert!(page.data.is_none());

    let empty = page_from_log_records(empty::<&str>());
    assert_eq!(empty.status, PageStatus::Empty);
}
