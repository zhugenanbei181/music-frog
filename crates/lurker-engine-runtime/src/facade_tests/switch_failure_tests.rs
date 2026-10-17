use super::*;

#[test]
fn a_switch_failing_before_the_reservation_rolls_back_to_the_live_owner() {
    let mihomo = EngineId::mihomo();
    let singbox = EngineId::sing_box();
    let old = EngineFixture::new(mihomo.clone(), "old-instance");
    let foreign_calls = Arc::new(AtomicUsize::new(0));
    let foreign: Arc<dyn EngineBackendProvider<EngineBackendPorts>> = Arc::new(FakeProvider {
        declared: singbox.clone(),
        bundle_engine: mihomo.clone(),
        calls: Arc::clone(&foreign_calls),
        process: Arc::clone(&old.process),
        readiness: Arc::clone(&old.readiness),
    });
    let journal = Arc::new(MemoryEngineSwitchJournal::new(mihomo.clone()));
    let runtime = LurkerRuntimeBuilder::new()
        .register_engine(old.provider())
        .expect("old provider registers")
        .register_engine(foreign)
        .expect("foreign provider registers")
        .select_engine(mihomo.clone())
        .with_switch_journal(Arc::clone(&journal))
        .build()
        .expect("runtime builds");
    block_on(runtime.start()).expect("start succeeds");
    let calls_before = old.process.calls();
    let scope_before = runtime.scope().expect("old owner scope is present");

    let error = block_on(runtime.switch_engine(singbox))
        .expect_err("a foreign bundle must fail the switch");
    match error {
        RuntimeError::Switch(switch) => {
            assert_eq!(switch.phase, EngineSwitchFailurePhase::BeforeReservation);
            assert!(!switch.is_recovery_required());
            assert_eq!(
                switch.disposition,
                EngineSwitchDisposition::RolledBackToPrevious {
                    selection: mihomo.clone()
                }
            );
        }
        other => panic!("expected switch failure, got {other:?}"),
    }
    assert_eq!(foreign_calls.load(Ordering::SeqCst), 1);
    assert_eq!(old.process.calls(), calls_before);
    assert_eq!(runtime.scope(), Some(scope_before));
    assert!(matches!(
        runtime.state(),
        LurkerRuntimeState::Running { engine, .. } if engine == mihomo
    ));
    assert!(matches!(
        block_on(journal.snapshot())
            .expect("journal snapshot reads")
            .owner,
        EngineOwnerState::Bound { .. }
    ));
}

#[test]
fn a_switch_failing_after_the_old_terminal_reports_recovery_required() {
    let mihomo = EngineId::mihomo();
    let singbox = EngineId::sing_box();
    let old = EngineFixture::new(mihomo.clone(), "old-instance");
    let candidate_process =
        Arc::new(FakeProcess::new(singbox.clone(), "candidate-instance").failing_start());
    let candidate = EngineFixture::with_process(
        singbox.clone(),
        candidate_process,
        Arc::new(FakeReadiness::ready()),
    );
    let journal = Arc::new(MemoryEngineSwitchJournal::new(mihomo.clone()));
    let runtime = LurkerRuntimeBuilder::new()
        .register_engine(old.provider())
        .expect("old provider registers")
        .register_engine(candidate.provider())
        .expect("candidate provider registers")
        .select_engine(mihomo.clone())
        .with_switch_journal(Arc::clone(&journal))
        .build()
        .expect("runtime builds");
    block_on(runtime.start()).expect("start succeeds");
    let mut stream = runtime.state_stream();
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Running { .. })
    ));

    let error = block_on(runtime.switch_engine(singbox.clone()))
        .expect_err("candidate start failure requires recovery");
    match error {
        RuntimeError::Switch(switch) => {
            assert_eq!(switch.phase, EngineSwitchFailurePhase::OldOwnerClosed);
            assert!(switch.is_recovery_required());
            assert_eq!(
                switch.disposition,
                EngineSwitchDisposition::RecoveryRequired
            );
        }
        other => panic!("expected switch failure, got {other:?}"),
    }
    match runtime.state() {
        LurkerRuntimeState::RecoveryRequired {
            selection, phase, ..
        } => {
            assert_eq!(selection, singbox);
            assert_eq!(phase, EngineSwitchFailurePhase::OldOwnerClosed);
        }
        other => panic!("expected recovery-required state, got {other:?}"),
    }
    assert_eq!(old.process.current_lifecycle(), CoreLifecycle::Stopped);
    assert_eq!(old.process.current_pid(), None);
    assert_eq!(
        candidate.process.current_lifecycle(),
        CoreLifecycle::Stopped
    );
    let snapshot = block_on(journal.snapshot()).expect("journal snapshot reads");
    assert_eq!(snapshot.selection, singbox);
    assert!(matches!(
        snapshot.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::BindingNew { .. }
        }
    ));

    // The stream published the exact observable switch order: closing the old
    // owner, its exact terminal, the durable new-owner admission and recovery.
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Switching {
            stage: EngineSwitchStage::ClosingOld { .. },
            ..
        })
    ));
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Switching {
            stage: EngineSwitchStage::OldStopped { .. },
            ..
        })
    ));
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::Switching {
            stage: EngineSwitchStage::BindingNew { .. },
            ..
        })
    ));
    assert!(matches!(
        block_on(stream.next()),
        Some(LurkerRuntimeState::RecoveryRequired { .. })
    ));
}

#[test]
fn a_second_control_operation_returns_control_busy_without_mutating_state() {
    let engine_id = EngineId::mihomo();
    let (gate, gate_receiver) = oneshot::channel();
    let process = Arc::new(
        FakeProcess::new(engine_id.clone(), "test-instance").with_prepare_gate(gate_receiver),
    );
    let fixture =
        EngineFixture::with_process(engine_id.clone(), process, Arc::new(FakeReadiness::ready()));
    let runtime = LurkerRuntimeBuilder::new()
        .register_engine(fixture.provider())
        .expect("test provider registers")
        .select_engine(engine_id.clone())
        .build()
        .expect("runtime builds");

    let mut first = Box::pin(runtime.start());
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(first.as_mut().poll(&mut context).is_pending());

    assert_eq!(block_on(runtime.start()), Err(RuntimeError::ControlBusy));
    assert_eq!(
        runtime.state(),
        LurkerRuntimeState::Starting {
            engine: engine_id.clone()
        }
    );
    assert_eq!(block_on(runtime.stop()), Err(RuntimeError::ControlBusy));
    assert_eq!(
        runtime.state(),
        LurkerRuntimeState::Starting { engine: engine_id }
    );

    drop(first);
    drop(gate);
}

#[test]
fn readiness_failure_publishes_failed_and_stops_the_started_process() {
    let engine_id = EngineId::mihomo();
    let process = Arc::new(FakeProcess::new(engine_id.clone(), "test-instance"));
    let fixture = EngineFixture::with_process(
        engine_id.clone(),
        Arc::clone(&process),
        Arc::new(FakeReadiness::never_ready()),
    );
    let runtime = LurkerRuntimeBuilder::new()
        .register_engine(fixture.provider())
        .expect("test provider registers")
        .select_engine(engine_id.clone())
        .with_readiness_timeout(Duration::from_millis(20))
        .build()
        .expect("runtime builds");

    let error = block_on(runtime.start()).expect_err("readiness never becomes ready");
    assert!(matches!(error, RuntimeError::Port(_)));
    match runtime.state() {
        LurkerRuntimeState::Failed { engine, phase, .. } => {
            assert_eq!(engine, Some(engine_id));
            assert_eq!(phase, RuntimeFailurePhase::Readiness);
        }
        other => panic!("expected failed state, got {other:?}"),
    }
    assert_eq!(process.current_lifecycle(), CoreLifecycle::Stopped);
    assert_eq!(process.current_pid(), None);
    assert!(process.calls().contains(&ProcessCall::Stop));
}
