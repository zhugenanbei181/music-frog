use super::*;
use crate::core_application::{
    test_dispatch_at_current_scope, test_execute_result_at_current_scope,
};
use crate::core_capability_test_support::core_scope;

#[tokio::test]
async fn outbound_scope_source_only_exposes_a_ready_or_running_owner_scope() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );
    assert!(OutboundScopeSource::current_scope(&app).is_none());

    let started = test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await;
    assert!(matches!(started, CommandResult::Completed { .. }));
    assert!(OutboundScopeSource::current_scope(&app).is_some());

    let stopped = test_execute_result_at_current_scope(&app, CommandIntent::StopCore).await;
    assert!(matches!(stopped, CommandResult::Completed { .. }));
    assert!(OutboundScopeSource::current_scope(&app).is_none());
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
        _context: infiltrator_ports::core_process::CoreProcessStartContext,
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
    generation: infiltrator_contract::core_identity::CoreGeneration,
    drops: Arc<AtomicUsize>,
}

struct GatedOwnerLease {
    identity: CoreIdentity,
    generation: infiltrator_contract::core_identity::CoreGeneration,
    attempts: Arc<AtomicUsize>,
    entered: Arc<tokio::sync::Notify>,
    allow_finish: Arc<tokio::sync::Notify>,
}

#[async_trait::async_trait]
impl infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerLease
    for GatedOwnerLease
{
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> infiltrator_contract::core_identity::CoreGeneration {
        self.generation
    }

    async fn release_after_terminal(
        &self,
        terminal: &infiltrator_contract::snapshot::CoreLifecycleSnapshot,
    ) -> Result<(), infiltrator_ports::error::PortError> {
        if !terminal
            .identity
            .as_ref()
            .is_some_and(|identity| self.identity.same_instance(identity))
            || terminal.generation < self.generation.value()
            || terminal.lifecycle != CoreLifecycle::Stopped
            || terminal.session_token.is_some()
        {
            return Err(infiltrator_ports::error::PortError::Failed(
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
impl infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerLease
    for TestActiveOwnerLease
{
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> infiltrator_contract::core_identity::CoreGeneration {
        self.generation
    }

    async fn release_after_terminal(
        &self,
        terminal: &infiltrator_contract::snapshot::CoreLifecycleSnapshot,
    ) -> Result<(), infiltrator_ports::error::PortError> {
        if !terminal
            .identity
            .as_ref()
            .is_some_and(|identity| self.identity.same_instance(identity))
            || terminal.generation < self.generation.value()
            || terminal.lifecycle != infiltrator_contract::snapshot::CoreLifecycle::Stopped
            || terminal.session_token.is_some()
        {
            return Err(infiltrator_ports::error::PortError::Failed(
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
        generation: infiltrator_contract::core_identity::CoreGeneration::new(0),
        drops: Arc::clone(&released),
    }))
    .unwrap();
    assert_eq!(
        app.active_profile_reference_write_authorization().unwrap(),
        infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization::BoundOwner {
            identity: identity.clone(),
            generation: infiltrator_contract::core_identity::CoreGeneration::new(0),
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
        infiltrator_contract::active_profile_reference::ActiveProfileReferenceWriteAuthorization::BoundOwner {
            identity,
            generation: infiltrator_contract::core_identity::CoreGeneration::new(0),
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
            generation: infiltrator_contract::core_identity::CoreGeneration::new(0),
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
    let entered = Arc::new(tokio::sync::Notify::new());
    let allow_finish = Arc::new(tokio::sync::Notify::new());
    app.install_active_profile_owner_binding(Box::new(GatedOwnerLease {
        identity,
        generation: infiltrator_contract::core_identity::CoreGeneration::new(0),
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

#[tokio::test]
async fn lifecycle_commands_publish_only_contract_values() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );

    let started = test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await;
    assert_eq!(
        started,
        CommandResult::Completed {
            request_id: RequestId(1)
        }
    );
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
    assert_eq!(app.snapshot().generation, 1);
    assert_eq!(
        app.snapshot()
            .identity
            .as_ref()
            .and_then(|identity| identity.version.as_ref())
            .map(CoreVersion::raw),
        Some("1.19.12")
    );
    let lifecycle = app.lifecycle_snapshot();
    assert_eq!(lifecycle.lifecycle, CoreLifecycle::Running);
    assert_eq!(lifecycle.generation, 1);
    assert_eq!(lifecycle.session_token, app.snapshot().session_token);
    assert_eq!(lifecycle.revision, app.snapshot().revision);
    let first_session = app
        .snapshot()
        .session_token
        .expect("running core must expose a session token");
    assert!(first_session.is_valid());
    assert_eq!(app.snapshot().revision, 2);

    let events = app.drain_events();
    assert!(matches!(
        events.first(),
        Some(CoreEvent::CommandAccepted { .. })
    ));
    assert!(matches!(events.get(1), Some(CoreEvent::SnapshotUpdated(_))));
    assert!(matches!(events.get(2), Some(CoreEvent::SnapshotUpdated(_))));
    assert!(matches!(
        events.get(3),
        Some(CoreEvent::CommandCompleted { .. })
    ));

    let stopped = test_execute_result_at_current_scope(&app, CommandIntent::StopCore).await;
    assert_eq!(
        stopped,
        CommandResult::Completed {
            request_id: RequestId(2)
        }
    );
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Stopped);
    assert_eq!(app.snapshot().generation, 1);
    assert!(app.snapshot().session_token.is_none());

    let restarted = test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await;
    assert!(matches!(restarted, CommandResult::Completed { .. }));
    let second_session = app
        .snapshot()
        .session_token
        .expect("restarted core must expose a session token");
    assert_ne!(first_session, second_session);
    assert_eq!(app.snapshot().generation, 2);
}

#[tokio::test]
async fn adapter_failures_become_typed_contract_failures() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: true,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );

    let result = test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await;
    let CommandResult::Rejected { failure, .. } = result else {
        panic!("expected rejected command");
    };
    assert_eq!(failure.code, ErrorCode::Internal);
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Failed);
    assert_eq!(app.snapshot().generation, 1);

    let events = app.drain_events();
    assert!(events.iter().any(|event| matches!(
        event,
        CoreEvent::CommandFailed { failure, .. } if failure.code == ErrorCode::Internal
    )));
}

#[tokio::test]
async fn readiness_failures_never_report_running() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Err(PortError::Network("controller unavailable".to_string())),
    );

    let result = test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await;
    let CommandResult::Rejected { failure, .. } = result else {
        panic!("expected rejected command");
    };
    assert_eq!(failure.code, ErrorCode::Network);
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Failed);
    assert!(
        !app.drain_events()
            .iter()
            .any(|event| matches!(event, CoreEvent::CommandCompleted { .. }))
    );
}

#[tokio::test]
async fn readiness_failure_stops_the_spawned_process_before_reporting_failed() {
    let process_state = Arc::new(Mutex::new(CoreLifecycle::Stopped));
    let stop_calls = Arc::new(AtomicUsize::new(0));
    let app = readiness_rollback_application(process_state.clone(), stop_calls.clone());

    let result = test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await;
    assert!(matches!(result, CommandResult::Rejected { .. }));
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Failed);
    assert_eq!(
        *process_state.lock().expect("process state lock"),
        CoreLifecycle::Stopped
    );
    assert_eq!(stop_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn adoption_readiness_failure_stops_the_unverified_process() {
    let process_state = Arc::new(Mutex::new(CoreLifecycle::Running));
    let stop_calls = Arc::new(AtomicUsize::new(0));
    let app = readiness_rollback_application(process_state.clone(), stop_calls.clone());

    assert!(app.adopt_if_running().await.is_err());
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Failed);
    assert_eq!(
        *process_state.lock().expect("process state lock"),
        CoreLifecycle::Stopped
    );
    assert_eq!(stop_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unknown_running_version_is_not_invented_and_restart_remains_version_independent() {
    let application = CoreApplication::new_with_policy(
        Arc::new(FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        }),
        Arc::new(VersionUnavailableReadiness),
        ReadinessPolicy {
            timeout: time::Duration::from_millis(100),
            poll_interval: time::Duration::from_millis(1),
        },
        runtime(),
    );
    install_lifecycle_capabilities(&application);

    assert!(matches!(
        test_execute_result_at_current_scope(&application, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    assert!(application.snapshot().identity.unwrap().version.is_none());
    let running = application.snapshot();

    let restart =
        test_execute_result_at_current_scope(&application, CommandIntent::RestartCore).await;
    assert!(matches!(restart, CommandResult::Completed { .. }));
    assert_eq!(application.snapshot().lifecycle, CoreLifecycle::Running);
    assert!(application.snapshot().generation > running.generation);
    assert_ne!(application.snapshot().session_token, running.session_token);
    assert!(application.snapshot().identity.unwrap().version.is_none());

    assert!(matches!(
        test_execute_result_at_current_scope(&application, CommandIntent::StopCore).await,
        CommandResult::Completed { .. }
    ));
    assert_eq!(application.snapshot().lifecycle, CoreLifecycle::Stopped);
}

#[tokio::test]
async fn dispatch_returns_id_and_completes_through_event_queue() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );

    let (request_id, terminal) = test_dispatch_at_current_scope(&app, CommandIntent::StartCore);
    assert_eq!(request_id, RequestId(1));

    let deadline = time::Instant::now() + time::Duration::from_secs(5);
    loop {
        for event in app.drain_events() {
            match event {
                CoreEvent::CommandCompleted {
                    request_id: completed,
                    ..
                } if completed == request_id => {
                    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
                    assert_eq!(app.snapshot().generation, 1);
                    assert!(app.snapshot().session_token.is_some());
                    assert!(matches!(
                        terminal.recv().unwrap().result,
                        CommandResult::Completed { request_id: completed } if completed == request_id
                    ));
                    return;
                }
                CoreEvent::CommandFailed {
                    request_id: failed,
                    failure,
                    ..
                } if failed == request_id => {
                    panic!("dispatched start command failed: {failure:?}");
                }
                _ => {}
            }
        }
        assert!(
            time::Instant::now() < deadline,
            "dispatched start command has no correlated terminal event"
        );
        sleep(time::Duration::from_millis(1)).await;
    }
}

#[tokio::test]
async fn adopt_checks_host_status_without_starting_the_process() {
    let process = FakeProcess {
        running: AtomicBool::new(true),
        fail_start: false,
        fail_stop: false,
    };
    let app = application(process, Ok("http://127.0.0.1:9090".to_string()));

    assert!(app.adopt_if_running().await.expect("adopt succeeds"));
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
    assert_eq!(app.snapshot().generation, 1);
    assert!(app.snapshot().session_token.is_some());
}

#[tokio::test]
async fn non_lifecycle_commands_use_the_installed_handler() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );
    app.install_command_handler(Arc::new(FakeCommandHandler));

    let result = test_execute_result_at_current_scope(&app, CommandIntent::ClearLogs).await;
    assert_eq!(
        result,
        CommandResult::Completed {
            request_id: RequestId(1)
        }
    );
}

#[tokio::test]
async fn routing_intents_fail_closed_without_a_current_owner_scope() {
    let app = CoreApplication::new(
        Arc::new(MissingIdentityProcess),
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".into()),
        }),
        runtime(),
    );
    let handler_calls = Arc::new(AtomicUsize::new(0));
    app.install_command_handler(Arc::new(CountingCommandHandler(handler_calls.clone())));

    let engine = EngineId::new("mihomo").expect("valid engine ID");
    let collection = OutboundId::new(engine.clone(), "selector").expect("collection ID");
    let selected = OutboundId::new(engine, "proxy-a").expect("outbound ID");
    let intents = [
        CommandIntent::SetRoutingIntent {
            intent: RoutingIntent::FollowRules,
        },
        CommandIntent::ReadOutboundInventory,
        CommandIntent::SelectOutbound {
            request: OutboundSelectionRequest::new(collection, selected, SnapshotRevision::new(1))
                .expect("valid selection request"),
        },
    ];

    for intent in intents {
        let request = CoreCommandRequest {
            scope: core_scope(),
            intent: intent.clone(),
        };
        let outcome = app.execute_scoped(request.clone()).await;
        assert!(matches!(
            outcome.result,
            CommandResult::Rejected { failure, .. } if failure.code == ErrorCode::InvalidState
        ));
        assert_eq!(outcome.scope, None);

        let (request_id, result) = app.dispatch_scoped_tracked(request);
        let outcome = result
            .recv_timeout(time::Duration::from_secs(1))
            .expect("missing owner scope receives a terminal rejection");
        assert!(matches!(
            outcome.result,
            CommandResult::Rejected { request_id: received, failure }
                if received == request_id && failure.code == ErrorCode::InvalidState
        ));
        assert_eq!(outcome.scope, None);
    }

    assert!(
        app.drain_events()
            .iter()
            .all(|event| !matches!(event, CoreEvent::CommandAccepted { .. }))
    );
    assert_eq!(handler_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn stale_session_tokens_are_rejected_after_stop_and_restart() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );

    test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await;
    let old_token = app.session_token().expect("session token after start");
    assert!(app.check_session(old_token).is_ok());

    test_execute_result_at_current_scope(&app, CommandIntent::StopCore).await;
    assert!(app.check_session(old_token).is_err());

    test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await;
    let new_token = app.session_token().expect("session token after restart");
    assert_ne!(old_token, new_token);
    assert!(app.check_session(old_token).is_err());
    assert!(app.check_session(new_token).is_ok());
}

#[tokio::test]
async fn start_reconciles_orphan_state_before_spawning_a_new_session() {
    let process = Arc::new(CleanupTrackingProcess {
        running: AtomicBool::new(false),
        cleanup_called: AtomicBool::new(false),
    });
    let app = CoreApplication::new_with_policy(
        process.clone(),
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".to_string()),
        }),
        ReadinessPolicy {
            timeout: time::Duration::from_millis(100),
            poll_interval: time::Duration::from_millis(1),
        },
        runtime(),
    );
    install_lifecycle_capabilities(&app);

    let result = test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await;
    assert!(matches!(result, CommandResult::Completed { .. }));
    assert!(process.cleanup_called.load(Ordering::SeqCst));
    assert_eq!(app.snapshot().generation, 1);
    assert!(app.snapshot().session_token.is_some());
}

#[tokio::test]
async fn hot_reload_fences_to_the_current_session_without_bumping_generation() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );
    test_execute_result_at_current_scope(&app, CommandIntent::StartCore).await;
    let before = app.snapshot();
    let token = CoreLifecyclePort::begin_reload(&app).expect("begin reload");
    assert_eq!(Some(token), before.session_token);
    assert_eq!(app.snapshot().generation, before.generation);
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Ready);

    CoreLifecyclePort::complete_reload(&app, token).expect("complete reload");
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
    assert_eq!(app.snapshot().generation, before.generation);

    let token = CoreLifecyclePort::begin_reload(&app).expect("begin second reload");
    CoreLifecyclePort::fail_reload(&app, token, "invalid config".to_string())
        .expect("record reload failure");
    assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
    assert_eq!(app.snapshot().session_token, Some(token));
}

#[test]
fn dispatch_does_not_require_a_caller_owned_runtime() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".to_string()),
    );

    let (request_id, terminal) = test_dispatch_at_current_scope(&app, CommandIntent::StartCore);
    assert_eq!(request_id, RequestId(1));
    for _ in 0..100 {
        if app
            .drain_events()
            .iter()
            .any(|event| matches!(event, CoreEvent::CommandCompleted { .. }))
        {
            assert_eq!(app.snapshot().lifecycle, CoreLifecycle::Running);
            assert!(matches!(
                terminal.recv().unwrap().result,
                CommandResult::Completed { request_id: completed } if completed == request_id
            ));
            return;
        }
        thread::sleep(time::Duration::from_millis(1));
    }
    panic!("dispatched command did not complete without a caller runtime");
}

#[test]
fn tracked_completion_survives_lifecycle_event_ring_eviction() {
    let app = application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".into()),
    );
    app.install_command_handler(Arc::new(FakeCommandHandler));
    let (id, first) = test_dispatch_at_current_scope(&app, CommandIntent::ClearLogs);
    for _ in 0..300 {
        let (next_id, reply) = test_dispatch_at_current_scope(&app, CommandIntent::ClearLogs);
        assert_eq!(
            reply
                .recv_timeout(time::Duration::from_secs(5))
                .unwrap()
                .result,
            CommandResult::Completed {
                request_id: next_id
            }
        );
    }
    // The transient lifecycle queue has lost the first event, but its private acknowledgment survives.
    assert!(!app.drain_events().iter().any(
        |event| matches!(event, CoreEvent::CommandCompleted { request_id, .. } if *request_id == id)
    ));
    assert_eq!(
        first
            .recv_timeout(time::Duration::from_secs(5))
            .unwrap()
            .result,
        CommandResult::Completed { request_id: id }
    );
}
