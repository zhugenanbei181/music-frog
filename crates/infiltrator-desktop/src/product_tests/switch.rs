use super::*;


#[test]
fn post_close_switch_failure_closes_the_old_owner_and_retries_the_typed_target() {
    let previous_application = application();
    let previous_engine = previous_application
        .snapshot()
        .scope()
        .expect("Core owner scope")
        .identity
        .engine
        .clone();
    let target_engine = if previous_engine.as_str() == "mihomo" {
        EngineId::new("sing-box").expect("valid target engine")
    } else {
        EngineId::new("mihomo").expect("valid target engine")
    };
    let replacement_store = Arc::new(RuleTraceStore::default());
    let replacement_profile_store: Arc<dyn ProfileStore> = replacement_store.clone();
    let replacement_profiles = ProfileApplication::new(replacement_profile_store);
    let replacement = backend_with_profiles(application(), replacement_profiles);
    let expected_application = Arc::clone(&replacement.application);
    replacement_store.deny_read.store(true, Ordering::SeqCst);
    let factory = Arc::new(RetryAfterCloseFactory {
        retry_backend: Mutex::new(Some(replacement)),
        switch_calls: AtomicUsize::new(0),
        retry_calls: AtomicUsize::new(0),
    });
    let session = session_for_with_factory(
        previous_application,
        Arc::clone(&factory) as Arc<dyn DesktopProductBackendFactory>,
    )
    .into_shared();

    let recovered = session
        .apply(DesktopProductSessionAction::SelectProfile(
            ActiveProfileReferenceTarget::selected(
                target_engine,
                ProfileId::new("next").expect("valid profile ID"),
            ),
        ))
        .expect("closed prior owner is resumed into the target owner");

    assert!(Arc::ptr_eq(&expected_application, &recovered.application));
    let verification_executor = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create profile owner verification executor");
    assert!(
        verification_executor
            .block_on(recovered.profiles.list_profiles())
            .is_err()
    );
    assert_eq!(factory.switch_calls.load(Ordering::SeqCst), 1);
    assert_eq!(factory.retry_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        session
            .binding()
            .expect("recovered owner is available")
            .lifecycle
            .lifecycle,
        CoreLifecycle::Stopped,
    );
}

#[test]
fn post_close_retry_restarts_a_core_that_was_running_before_the_switch() {
    let previous_application = application();
    let previous_engine = previous_application
        .snapshot()
        .scope()
        .expect("Core owner scope")
        .identity
        .engine
        .clone();
    let target_engine = if previous_engine.as_str() == "mihomo" {
        EngineId::new("sing-box").expect("valid target engine")
    } else {
        EngineId::new("mihomo").expect("valid target engine")
    };
    let replacement = backend();
    let expected_application = Arc::clone(&replacement.application);
    let factory = Arc::new(RetryAfterCloseFactory {
        retry_backend: Mutex::new(Some(replacement)),
        switch_calls: AtomicUsize::new(0),
        retry_calls: AtomicUsize::new(0),
    });
    let session = session_for_with_factory(
        previous_application,
        Arc::clone(&factory) as Arc<dyn DesktopProductBackendFactory>,
    )
    .into_shared();
    session
        .apply(DesktopProductSessionAction::StartCore)
        .expect("start the original Core owner");

    let recovered = session
        .apply(DesktopProductSessionAction::SelectProfile(
            ActiveProfileReferenceTarget::selected(
                target_engine,
                ProfileId::new("next").expect("valid profile ID"),
            ),
        ))
        .expect("retry recovers the replacement owner");

    assert!(Arc::ptr_eq(&expected_application, &recovered.application));
    assert_eq!(recovered.lifecycle.lifecycle, CoreLifecycle::Running);
    assert_eq!(factory.switch_calls.load(Ordering::SeqCst), 1);
    assert_eq!(factory.retry_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn successful_switch_replaces_application_and_surface_handles() {
    let mut session = session();
    let previous_application = session.application();
    let previous_profiles = session.binding().expect("current owner binding").profiles;
    assert!(
        session
            .executor
            .block_on(previous_profiles.list_profiles())
            .is_ok()
    );
    let target = profile_target(&session, "next");
    let resume = resume_policy(&session);
    let (host_runtime, pump) = session
        .begin_engine_switch()
        .expect("open session begins switch");
    let replacement_store = Arc::new(RuleTraceStore::default());
    let replacement_profile_store: Arc<dyn ProfileStore> = replacement_store.clone();
    let replacement_profiles = ProfileApplication::new(replacement_profile_store);
    let replacement = backend_with_profiles(application(), replacement_profiles);
    let replacement_application = Arc::clone(&replacement.application);

    session
        .complete_engine_switch(target, resume, host_runtime, pump, Ok(replacement))
        .expect("new owner becomes the open session");

    assert_eq!(
        session.availability(),
        DesktopProductSessionAvailability::Open
    );
    assert!(Arc::ptr_eq(
        &replacement_application,
        &session.application()
    ));
    assert!(!Arc::ptr_eq(&previous_application, &session.application()));
    replacement_store.deny_read.store(true, Ordering::SeqCst);
    let current_profiles = session
        .binding()
        .expect("replacement owner binding")
        .profiles;
    assert!(
        session
            .executor
            .block_on(current_profiles.list_profiles())
            .is_err()
    );
}

#[test]
fn typed_lifecycle_actions_keep_the_bound_core_owner_and_update_its_snapshot() {
    let session = session().into_shared();
    let before = session.binding().expect("open session binding");

    let started = session
        .apply(DesktopProductSessionAction::StartCore)
        .expect("start the bound Core owner");
    assert_eq!(started.lifecycle.lifecycle, CoreLifecycle::Running);
    assert!(Arc::ptr_eq(&before.application, &started.application));

    let restarted = session
        .apply(DesktopProductSessionAction::RestartCore)
        .expect("restart the bound Core owner");
    assert_eq!(restarted.lifecycle.lifecycle, CoreLifecycle::Running);
    assert!(restarted.lifecycle.generation > started.lifecycle.generation);
    assert!(Arc::ptr_eq(&started.application, &restarted.application));

    let stopped = session
        .apply(DesktopProductSessionAction::StopCore)
        .expect("stop Core without destroying its product owner");
    assert_eq!(stopped.lifecycle.lifecycle, CoreLifecycle::Stopped);
    assert!(Arc::ptr_eq(&restarted.application, &stopped.application));
    assert_eq!(
        session
            .binding()
            .expect("owner remains open")
            .lifecycle
            .lifecycle,
        CoreLifecycle::Stopped,
    );
}

#[test]
fn explicit_close_stops_the_bound_owner_and_is_idempotent() {
    let session = session().into_shared();
    let binding = session.binding().expect("open session binding");
    session
        .apply(DesktopProductSessionAction::StartCore)
        .expect("start the bound Core owner");

    session.close().expect("close the bound Core owner");

    assert_eq!(
        session.availability().expect("closed session state"),
        DesktopProductSessionAvailability::Closed
    );
    assert_eq!(
        binding.application.snapshot().lifecycle,
        CoreLifecycle::Stopped
    );
    session.close().expect("closed session close is idempotent");
}

#[test]
fn same_engine_profile_selection_uses_the_bound_scoped_core_command() {
    let (application, handler) = application_with_handler();
    let session = session_for(Arc::clone(&application)).into_shared();
    let started = session
        .apply(DesktopProductSessionAction::StartCore)
        .expect("start bound Core owner");
    let engine_id = started
        .lifecycle
        .identity
        .as_ref()
        .expect("started Core identity")
        .engine
        .clone();

    let selected = session
        .apply(DesktopProductSessionAction::SelectProfile(
            ActiveProfileReferenceTarget::selected(
                engine_id,
                ProfileId::new("main").expect("valid profile ID"),
            ),
        ))
        .expect("select profile through bound owner");

    assert!(Arc::ptr_eq(&application, &selected.application));
    assert_eq!(selected.lifecycle.generation, started.lifecycle.generation);
    assert_eq!(
        handler
            .intents
            .lock()
            .expect("recorded commands")
            .as_slice(),
        &[CommandIntent::SwitchProfile {
            profile_id: "main".to_owned(),
        }],
    );
}

#[test]
fn stale_lifecycle_scope_is_rejected_without_replacing_the_desktop_owner() {
    let session = session();
    let application = session.application();
    let stale_scope = application.snapshot().scope().expect("Core identity scope");
    session
        .executor
        .block_on(application.execute_scoped(
            CoreCommandRequest {
                scope: stale_scope.clone(),
                intent: CommandIntent::StartCore,
            },
        ))
        .result
        .into_unit()
        .expect("first scoped start succeeds");
    let before_restart = application.snapshot();

    let stale = session.executor.block_on(application.execute_scoped(
        CoreCommandRequest {
            scope: stale_scope,
            intent: CommandIntent::RestartCore,
        },
    ));

    assert!(stale.result.into_unit().is_err());
    assert_eq!(application.snapshot().generation, before_restart.generation);
    assert!(Arc::ptr_eq(&application, &session.application()));
}

#[test]
fn session_action_reports_owner_switch_phase_instead_of_falling_back_to_host_boot() {
    let session = session().into_shared();
    let (host_runtime, pump) = {
        let mut session = session.lock().expect("session lock");
        session.begin_engine_switch().expect("begin owner switch")
    };
    drop(host_runtime);
    drop(pump);

    let failure = match session.apply(DesktopProductSessionAction::RestartCore) {
        Err(failure) => failure,
        Ok(_) => panic!("a session in the switching state must be rejected"),
    };
    assert!(matches!(
        failure,
        DesktopProductSessionActionFailure::SessionUnavailable(_)
    ));
}

