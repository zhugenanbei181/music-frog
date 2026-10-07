//! test-intent: behavior
//! Application session changes retire legacy facts; delayed/unscoped reads cannot reintroduce them.
use crate::state::AppState;
use crate::types::message::Message;
use infiltrator_application::connection_grouping_fixtures::grouping_snapshot;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::{LogCaptureProcess, populate_logs};
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_domain::proxy::Proxy;
use infiltrator_ports::surface::SurfaceReader;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Builder;

#[test]
fn composed_snapshot_retires_demo_rows_and_rejects_reads_from_the_previous_core_session() {
    let runtime = tokio_application_runtime().unwrap();
    let process = Arc::new(LogCaptureProcess::default());
    let core = CoreApplication::new(process.clone(), process, runtime.clone());
    let producing = core.clone();
    runtime.block_on(Box::pin(async move {
        populate_logs(&producing).await.unwrap();
    }));
    let reader = ApplicationSurfaceReader::new(
        Arc::new(core.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    );
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::ConnectionsReceived(grouping_snapshot()));
    assert_eq!(state.diag.connection_groups.source_count(), 9);
    state.commands = Some(core.clone());
    state
        .runtime
        .proxies
        .insert("old-demo".into(), Proxy::Unknown);
    state
        .runtime
        .filtered_groups
        .push(("old-demo-group".into(), vec!["old-demo".into()]));
    let publish = |state: &mut AppState| {
        let snapshot = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(reader.read())
            .unwrap();
        assert!(state.apply_shared_surface_snapshot(snapshot));
    };
    publish(&mut state);
    assert!(state.runtime.proxies.is_empty());
    assert!(state.diag.connections.is_none());
    assert_eq!(state.diag.connection_groups.source_count(), 0);
    assert!(state.runtime.filtered_groups.is_empty());
    let first = core.snapshot();
    let _ = state.update(Message::ProxiesLoadedForSession {
        generation: first.generation,
        session_token: first.session_token,
        result: Ok(HashMap::from([("actual-current".into(), Proxy::Unknown)])),
    });
    assert!(state.runtime.proxies.contains_key("actual-current"));
    publish(&mut state);
    assert!(
        state.runtime.proxies.contains_key("actual-current"),
        "same-session missing reader facts cannot erase a real observation"
    );
    let restarting = core.clone();
    runtime.block_on(Box::pin(async move {
        restarting
            .execute(CommandIntent::RestartCore)
            .await
            .into_unit()
            .unwrap();
    }));
    publish(&mut state);
    assert!(state.runtime.proxies.is_empty());
    let _ = state.update(Message::ProxiesLoadedForSession {
        generation: first.generation,
        session_token: first.session_token,
        result: Ok(HashMap::from([("late-old".into(), Proxy::Unknown)])),
    });
    let _ = state.update(Message::ProxiesLoaded(Ok(HashMap::from([(
        "unscoped".into(),
        Proxy::Unknown,
    )]))));
    assert!(state.runtime.proxies.is_empty());
    let current = core.snapshot();
    let _ = state.update(Message::ProxiesLoadedForSession {
        generation: current.generation,
        session_token: current.session_token,
        result: Ok(HashMap::from([("new-current".into(), Proxy::Unknown)])),
    });
    assert!(state.runtime.proxies.contains_key("new-current"));
    let closing = core.clone();
    runtime.block_on(Box::pin(async move {
        closing.close().await.unwrap();
    }));
}
