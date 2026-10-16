use infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization;
use infiltrator_contract::core_identity::CoreGeneration;
use infiltrator_contract::core_process::CoreProcessStartContext;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::snapshot::CoreLifecycleSnapshot;
use infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerLease;
use infiltrator_ports::error::PortError;
use tokio::sync::Notify;
use super::*;
use crate::core_application::{
    test_dispatch_at_current_scope, test_execute_result_at_current_scope,
};
use crate::core_capability_test_support::core_scope;

#[tokio::test]
async fn core_scope_source_only_exposes_a_ready_or_running_owner_scope() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );
    assert_eq!(
        CoreScopeSource::scope_observation(&app),
        CoreScopeObservation::Unavailable
    );

    let started = test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await;
    assert!(matches!(started, CommandResult::Completed { .. }));
    assert!(matches!(
        CoreScopeSource::scope_observation(&app),
        CoreScopeObservation::Observed(_)
    ));

    let stopped = test_execute_result_at_current_scope(&app, CommandIntent::StopCore).await;
    assert!(matches!(stopped, CommandResult::Completed { .. }));
    assert_eq!(
        CoreScopeSource::scope_observation(&app),
        CoreScopeObservation::Unavailable
    );
}

#[tokio::test]
async fn scoped_command_executes_for_the_current_core_instance() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );
    app.install_command_handler(Arc::new(FakeCommandHandler));
    let scope = app.snapshot().scope().expect("core identity scope");

    let outcome = app
        .execute_scoped(CoreCommandRequest {
            scope: scope.clone(),
            intent: CommandIntent::ClearLogs,
        })
        .await;

    assert_eq!(outcome.scope, Some(scope));
    assert!(matches!(outcome.result, CommandResult::Completed { .. }));
}

struct MissingIdentityProcess;

#[async_trait::async_trait]
impl CoreProcess for MissingIdentityProcess {
    async fn start(
        &self,
        _context: CoreProcessStartContext,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Stopped)
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

struct CountingCommandHandler(Arc<AtomicUsize>);

impl CommandHandler for CountingCommandHandler {
    fn handle(&self, _intent: CommandIntent) -> CommandFuture {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }

    fn handle_output_scoped(
        &self,
        _scope: ValidatedCoreCommandScope,
        _intent: CommandIntent,
    ) -> CommandOutputFuture {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(CommandOutput::Unit) })
    }
}

#[tokio::test]
async fn scoped_command_fails_closed_when_owner_identity_is_unavailable() {
    let app = CoreApplication::new(
        Arc::new(MissingIdentityProcess),
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".into()),
        }),
        runtime(),
    );
    let handler_calls = Arc::new(AtomicUsize::new(0));
    app.install_command_handler(Arc::new(CountingCommandHandler(handler_calls.clone())));

    let outcome = app
        .execute_scoped(CoreCommandRequest {
            scope: core_scope(),
            intent: CommandIntent::ClearLogs,
        })
        .await;

    assert!(matches!(
        outcome.result,
        CommandResult::Rejected { failure, .. } if failure.code == ErrorCode::InvalidState
    ));
    assert_eq!(outcome.scope, None);
    assert!(
        app.drain_events()
            .iter()
            .all(|event| !matches!(event, CoreEvent::CommandAccepted { .. }))
    );
    assert_eq!(handler_calls.load(Ordering::SeqCst), 0);
}

struct TestActiveOwnerLease {
    identity: CoreIdentity,
    generation: CoreGeneration,
    drops: Arc<AtomicUsize>,
}

struct GatedOwnerLease {
    identity: CoreIdentity,
    generation: CoreGeneration,
    attempts: Arc<AtomicUsize>,
    entered: Arc<Notify>,
    allow_finish: Arc<Notify>,
}

#[async_trait::async_trait]
impl ActiveProfileOwnerLease
    for GatedOwnerLease
{
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> CoreGeneration {
        self.generation
    }

    async fn record_start_context(
        &self,
        _context: &CoreProcessStartContext,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn release_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<(), PortError> {
        if !terminal
            .identity
            .as_ref()
            .is_some_and(|identity| self.identity.same_instance(identity))
            || terminal.generation < self.generation.value()
            || terminal.lifecycle != CoreLifecycle::Stopped
            || terminal.session_token.is_some()
        {
            return Err(PortError::Failed(
                "owner lease was not released by a matching terminal snapshot".into(),
            ));
        }
        self.attempts.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        self.allow_finish.notified().await;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ActiveProfileOwnerLease
    for TestActiveOwnerLease
{
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> CoreGeneration {
        self.generation
    }

    async fn record_start_context(
        &self,
        _context: &CoreProcessStartContext,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn release_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<(), PortError> {
        if !terminal
            .identity
            .as_ref()
            .is_some_and(|identity| self.identity.same_instance(identity))
            || terminal.generation < self.generation.value()
            || terminal.lifecycle != CoreLifecycle::Stopped
            || terminal.session_token.is_some()
        {
            return Err(PortError::Failed(
                "owner lease was not released by a matching terminal snapshot".into(),
            ));
        }
        self.drops.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn active_profile_owner_binding_releases_only_after_stopped_readback() {
    let released = Arc::new(AtomicUsize::new(0));
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );
    let identity = app.snapshot().identity.unwrap();
    app.install_active_profile_owner_binding(Box::new(TestActiveOwnerLease {
        identity: identity.clone(),
        generation: CoreGeneration::new(0),
        drops: Arc::clone(&released),
    }))
    .unwrap();
    assert_eq!(
        app.active_profile_reference_write_authorization().unwrap(),
        ActiveProfileReferenceWriteAuthorization::BoundOwner {
            identity: identity.clone(),
            generation: CoreGeneration::new(0),
        }
    );
    assert!(matches!(
        test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    assert!(app.generation() > 0);
    assert_eq!(
        app.snapshot()
            .identity
            .as_ref()
            .and_then(|identity| identity.version.as_ref())
            .map(|version| version.raw()),
        Some("1.19.12")
    );
    assert_eq!(
        app.active_profile_reference_write_authorization().unwrap(),
        ActiveProfileReferenceWriteAuthorization::BoundOwner {
            identity,
            generation: CoreGeneration::new(0),
        }
    );

    app.close().await.unwrap();
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert!(app.active_profile_reference_write_authorization().is_err());
    assert!(matches!(
        *app.inner.active_profile_owner_binding.lock().unwrap(),
        super::ActiveProfileOwnerBindingState::Released
    ));

    let retained = Arc::new(AtomicUsize::new(0));
    let failed_close = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: true,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );
    let identity = failed_close.snapshot().identity.unwrap();
    assert!(matches!(
        test_execute_result_at_current_scope(&failed_close, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    failed_close
        .install_active_profile_owner_binding(Box::new(TestActiveOwnerLease {
            identity,
            generation: CoreGeneration::new(0),
            drops: Arc::clone(&retained),
        }))
        .unwrap();

    assert!(failed_close.close().await.is_err());
    assert_eq!(retained.load(Ordering::SeqCst), 0);
    assert!(matches!(
        *failed_close
            .inner
            .active_profile_owner_binding
            .lock()
            .unwrap(),
        super::ActiveProfileOwnerBindingState::Bound(_)
    ));
    drop(failed_close);
    assert_eq!(retained.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn canceling_close_restores_owner_binding_for_a_later_retry() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".into()),
    );
    let identity = app.snapshot().identity.expect("owner identity");
    let attempts = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Notify::new());
    let allow_finish = Arc::new(Notify::new());
    app.install_active_profile_owner_binding(Box::new(GatedOwnerLease {
        identity,
        generation: CoreGeneration::new(0),
        attempts: Arc::clone(&attempts),
        entered: Arc::clone(&entered),
        allow_finish: Arc::clone(&allow_finish),
    }))
    .unwrap();

    let first_app = app.clone();
    let first_close = tokio::spawn(async move { first_app.close().await });
    entered.notified().await;
    assert!(matches!(
        *app.inner.active_profile_owner_binding.lock().unwrap(),
        super::ActiveProfileOwnerBindingState::Releasing
    ));
    first_close.abort();
    assert!(first_close.await.unwrap_err().is_cancelled());
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    assert!(matches!(
        *app.inner.active_profile_owner_binding.lock().unwrap(),
        super::ActiveProfileOwnerBindingState::Bound(_)
    ));

    let retry_app = app.clone();
    let retry = tokio::spawn(async move { retry_app.close().await });
    entered.notified().await;
    allow_finish.notify_one();
    retry.await.unwrap().unwrap();
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert!(matches!(
        *app.inner.active_profile_owner_binding.lock().unwrap(),
        super::ActiveProfileOwnerBindingState::Released
    ));
}

#[tokio::test]
async fn stale_core_generation_is_rejected_before_the_handler_runs() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );
    app.install_command_handler(Arc::new(FakeCommandHandler));
    let observed_scope = app.snapshot().scope().expect("core identity scope");
    assert!(matches!(
        test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    app.drain_events();

    let outcome = app
        .execute_scoped(CoreCommandRequest {
            scope: observed_scope,
            intent: CommandIntent::ClearLogs,
        })
        .await;

    assert!(matches!(outcome.result, CommandResult::Rejected { .. }));
    assert_eq!(outcome.scope, app.snapshot().scope());
    let events = app.drain_events();
    assert!(events.iter().all(|event| !matches!(
        event,
        CoreEvent::CommandAccepted {
            kind: CommandKind::Runtime,
            ..
        }
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        CoreEvent::CommandFailed { scope: Some(scope), .. }
            if Some(scope.clone()) == outcome.scope
    )));
}

#[tokio::test]
async fn expired_owner_scope_is_rejected_before_backend_capability_dispatch() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_owned()),
    );
    let handler_calls = Arc::new(AtomicUsize::new(0));
    app.install_command_handler(Arc::new(CountingCommandHandler(handler_calls.clone())));
    let expired_owner_scope = app.snapshot().scope().expect("owner Core scope");

    assert!(matches!(
        test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    app.drain_events();
    let outcome = app
        .execute_scoped(CoreCommandRequest {
            scope: expired_owner_scope,
            intent: CommandIntent::GetRoutingIntent,
        })
        .await;

    assert!(matches!(
        outcome.result,
        CommandResult::Rejected { failure, .. } if failure.code == ErrorCode::InvalidState
    ));
    assert_eq!(handler_calls.load(Ordering::SeqCst), 0);
    assert!(
        app.drain_events()
            .iter()
            .all(|event| !matches!(event, CoreEvent::CommandAccepted { .. }))
    );
}

#[tokio::test]
async fn tracked_scoped_dispatch_rejects_old_scope_and_completes_current_scope() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );
    app.install_command_handler(Arc::new(FakeCommandHandler));
    let observed_scope = app.snapshot().scope().expect("core identity scope");
    assert!(matches!(
        test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    app.drain_events();

    let (_, stale_reply) = app.dispatch_scoped_tracked(CoreCommandRequest {
        scope: observed_scope,
        intent: CommandIntent::ClearLogs,
    });
    let stale = stale_reply
        .recv_timeout(time::Duration::from_secs(1))
        .expect("stale scope receives a terminal acknowledgment");
    assert!(matches!(stale.result, CommandResult::Rejected { .. }));
    let events = app.drain_events();
    assert!(events.iter().all(|event| !matches!(
        event,
        CoreEvent::CommandAccepted {
            kind: CommandKind::Runtime,
            ..
        }
    )));

    let current_scope = app.snapshot().scope().expect("current core scope");
    let (_, current_reply) = app.dispatch_scoped_tracked(CoreCommandRequest {
        scope: current_scope.clone(),
        intent: CommandIntent::ClearLogs,
    });
    let current = current_reply
        .recv_timeout(time::Duration::from_secs(1))
        .expect("current scope receives a terminal acknowledgment");
    assert_eq!(current.scope, Some(current_scope));
    assert!(matches!(current.result, CommandResult::Completed { .. }));
}


#[path = "ownership_tests/lifecycle_cases.rs"]
mod lifecycle_cases;
