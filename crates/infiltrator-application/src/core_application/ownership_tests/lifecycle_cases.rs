use super::*;

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
async fn start_uses_typed_host_admission_before_spawning_a_new_session() {
    let process = Arc::new(StartAdmissionTrackingProcess {
        running: AtomicBool::new(false),
        prepare_called: AtomicBool::new(false),
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
    assert!(process.prepare_called.load(Ordering::SeqCst));
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
