//! test-intent: behavior
//! TEA consumes the application buffer and executes clear/filter on that same owner.
use crate::state::AppState;
use crate::test_mounts::command_harness::test_command_application_for;
use crate::test_mounts::execute_at_observed_scope;
use crate::types::message::Message;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::core_capabilities::{CoreLifecycleOperation, CoreOperation};
use infiltrator_contract::logs::{LogLevel, LogSession};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_ports::runtime_stream::RuntimeStreamEvent;
use infiltrator_presentation::log_page_model::LogLevelFilter;
use infiltrator_presentation::log_projection::project_log_facts;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use std::sync::Arc;
use tokio::runtime::Builder;

struct RunningIdentifiedProcess;

#[async_trait::async_trait]
impl CoreProcess for RunningIdentifiedProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("valid test engine"),
            CoreInstanceId::new("iced-log-owner-test").expect("valid test instance"),
            Some(CoreVersion::new("1.19.12").expect("valid test version")),
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Running)
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://127.0.0.1:9090".into())
    }
}

#[async_trait::async_trait]
impl CoreReadiness for RunningIdentifiedProcess {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("http://127.0.0.1:9090".into())
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(Some(
            CoreVersion::new("1.19.12").expect("valid test version"),
        ))
    }
}

fn composed() -> (AppState, CoreApplication) {
    let process = Arc::new(RunningIdentifiedProcess);
    let core = CoreApplication::new(
        process.clone(),
        process,
        infiltrator_composition::tokio_application_runtime().unwrap(),
    );
    let handler = test_command_application_for(
        [CoreOperation::Lifecycle(CoreLifecycleOperation::Start)],
        [Capability::Logs],
    )
    .with_logs(core.log_application());
    core.install_core_capability_application(handler.core_capability_application())
        .expect("one explicit test resolver serves owner and command facade");
    let started = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(execute_at_observed_scope(&core, CommandIntent::StartCore));
    assert!(
        started.into_unit().is_ok(),
        "identified test core starts successfully"
    );
    core.install_command_handler(Arc::new(handler));
    let (mut state, _) = AppState::new();
    state.attach_product_application(core.clone());
    (state, core)
}
fn observe(state: &mut AppState, core: &CoreApplication, revision: u64) {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "fixture", true),
    );
    snapshot.core = core.snapshot();
    let log_facts = core.log_application().facts();
    snapshot.pages.logs = project_log_facts(&snapshot.core, &log_facts);
    snapshot.revision = revision;
    assert!(state.apply_shared_surface_snapshot(snapshot));
}
fn finish(state: &mut AppState, task: Task<Message>) {
    let message = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("real command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("terminal message");
            };
            message
        });
    let _ = state.update(message);
}
#[test]
fn shared_logs_ignore_private_stream_and_clear_only_after_actual_command_execution() {
    let (mut state, core) = composed();
    let scope = LogSession::from_core_scope(core.snapshot().scope().expect("active core scope"))
        .expect("active owner log session");
    core.log_application()
        .ingest(scope, RuntimeStreamEvent::Item("WARN[1] retained".into()));
    observe(&mut state, &core, 1);
    assert_eq!(state.diag.logs.len(), 1);
    let _ = state.update(Message::LogReceived("private duplicate".into()));
    assert_eq!(state.diag.logs.len(), 1);
    let task = state.update(Message::ClearRuntimeLogs);
    assert_eq!(
        project_log_facts(&core.snapshot(), &core.log_application().facts())
            .data
            .unwrap()
            .entries
            .len(),
        1
    );
    finish(&mut state, task);
    observe(&mut state, &core, 2);
    assert!(state.diag.logs.is_empty());
    assert!(
        project_log_facts(&core.snapshot(), &core.log_application().facts())
            .data
            .unwrap()
            .entries
            .is_empty()
    );
}
#[test]
fn shared_filter_has_actual_effect_and_the_surface_cannot_ask_for_an_invalid_level() {
    let (mut state, core) = composed();
    let scope = LogSession::from_core_scope(core.snapshot().scope().expect("active core scope"))
        .expect("active owner log session");
    for raw in ["WARN[1] visible", "INFO[2] filtered"] {
        core.log_application()
            .ingest(scope.clone(), RuntimeStreamEvent::Item(raw.into()));
    }
    observe(&mut state, &core, 1);
    let task = state.update(Message::SetLogLevelFilter(Some(LogLevel::Warn)));
    finish(&mut state, task);
    observe(&mut state, &core, 2);
    assert_eq!(
        state.diag.logs.iter().collect::<Vec<_>>(),
        vec!["WARN[1] visible"]
    );
    // The owner's readback is the only authority for the level filter.
    assert_eq!(state.diag.logs_page.active_level(), Some(LogLevel::Warn));

    // The surface cannot express a level that is not a level: the message
    // carries the shared level enum and the shared page model refuses an
    // identifier that names no level.
    assert_eq!(
        LogLevelFilter::from_identifier("pretend"),
        LogLevelFilter::All
    );

    // The owner still rejects an invalid level with a typed failure.
    let rejected = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(execute_at_observed_scope(
            &core,
            CommandIntent::SetLogLevelFilter {
                level: Some("pretend".into()),
            },
        ))
        .into_unit()
        .expect_err("the owner rejects a level that names no level");
    assert_eq!(rejected.code, ErrorCode::InvalidInput);
    observe(&mut state, &core, 3);
    assert_eq!(state.diag.logs.len(), 1);
}
