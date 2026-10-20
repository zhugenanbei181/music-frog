//! The chaos matrix of the neutral runtime (PUB-006.7).
//!
//! Every scenario runs headless and is deterministic: the fault fires a fixed
//! number of times and the assertion is about the typed failure and the typed
//! recovery, never about a wall-clock measurement.

use lurker_core_contract::identity::{CoreGeneration, EngineId};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_ports::engine_switch::EngineSwitchJournal;
use lurker_core_ports::error::PortError;
use lurker_engine_runtime::facade::{LurkerRuntimeState, RuntimeError, RuntimeFailurePhase};
use lurker_engine_runtime::switch::memory_journal::MemoryEngineSwitchJournal;
use lurker_sandbox::fault::Fault;
use lurker_sandbox::ports::{ChaosCodec, ChaosPhase, ChaosProvider};
use lurker_sandbox::runtime_chaos::{sandbox_identity, Sandbox};
use lurker_sandbox::store_chaos::{observe_corrupted_document, observe_fence_contention};

#[tokio::test]
async fn a_network_flap_fails_closed_and_the_runtime_heals_itself() {
    let sandbox = Sandbox::new(EngineId::mihomo());
    sandbox.inject(Fault::NetworkFlap, u32::MAX);
    let runtime = sandbox.runtime().expect("the sandbox runtime builds");

    let error = runtime
        .start()
        .await
        .expect_err("the flapping controller never answers");
    match error {
        RuntimeError::Port(PortError::Network(_)) => {}
        other => panic!("the network flap must fail closed with a network error, got {other:?}"),
    }
    match runtime.state() {
        LurkerRuntimeState::Failed {
            phase: RuntimeFailurePhase::Readiness,
            ..
        } => {}
        other => panic!("the runtime must publish the readiness failure, got {other:?}"),
    }

    // The failed start stopped the process again: no orphan is left behind.
    assert_eq!(sandbox.process().phase(), ChaosPhase::Stopped);
    assert_eq!(sandbox.process().starts(), sandbox.process().stops());

    // The runtime refuses to run again from the failed state.
    assert!(matches!(
        runtime.start().await,
        Err(RuntimeError::InvalidState { .. })
    ));

    // The host observes the typed failure and builds a fresh runtime.
    sandbox.clear_faults();
    let healed = sandbox.runtime().expect("the healed sandbox runtime builds");
    healed.start().await.expect("the healed runtime starts");
    assert!(matches!(healed.state(), LurkerRuntimeState::Running { .. }));
}

#[tokio::test]
async fn a_dead_kernel_fails_closed_and_leaves_no_running_claim() {
    let sandbox = Sandbox::new(EngineId::mihomo());
    sandbox.inject(Fault::KernelProcessDeath, 1);
    let runtime = sandbox.runtime().expect("the sandbox runtime builds");

    let error = runtime
        .start()
        .await
        .expect_err("the dead kernel never starts");
    match error {
        RuntimeError::Port(PortError::Failed(_)) => {}
        other => panic!("the dead kernel must fail closed, got {other:?}"),
    }
    match runtime.state() {
        LurkerRuntimeState::Failed {
            phase: RuntimeFailurePhase::StartProcess,
            ..
        } => {}
        other => panic!("the runtime must publish the start failure, got {other:?}"),
    }
    assert_eq!(sandbox.process().phase(), ChaosPhase::Died);

    // The process never started, so nothing is left behind.
    assert_eq!(sandbox.process().starts(), 0);
    assert_eq!(sandbox.process().stops(), 0);

    // The host replaces the dead kernel and reaches the running state again.
    let healed = Sandbox::new(EngineId::mihomo());
    let runtime = healed.runtime().expect("the healed sandbox runtime builds");
    runtime.start().await.expect("the fresh kernel starts");
    assert!(matches!(runtime.state(), LurkerRuntimeState::Running { .. }));
}

#[test]
fn a_contended_write_fence_fails_closed_and_is_reclaimed() {
    let directory = tempfile::tempdir().expect("the temporary directory is usable");
    let lock_path = directory.path().join("instance.lock");

    let contention = observe_fence_contention(&lock_path);
    assert!(
        contention.acquired,
        "the first owner must acquire the write fence"
    );
    assert!(
        contention.contended,
        "the second owner must be refused while the fence is held"
    );
    assert!(
        contention.reclaimed,
        "the write fence must be acquirable again after its holder is gone"
    );
}

#[tokio::test]
async fn a_corrupted_configuration_is_rejected_and_recovered() {
    let directory = tempfile::tempdir().expect("the temporary directory is usable");
    let document_path = directory.path().join("primary.yaml");
    let codec = ChaosCodec::new(
        ChaosProvider::codec_scope(&EngineId::mihomo()),
        ChaosProvider::valid_document(),
    );

    let observation = observe_corrupted_document(&codec, &document_path).await;
    assert!(
        observation.intact_accepted,
        "the intact document must be accepted"
    );
    assert!(
        observation.corrupted_rejected,
        "the corrupted document must be rejected"
    );
    assert_eq!(
        observation.diagnostics, 1,
        "the rejection must carry typed diagnostics"
    );
    assert!(
        observation.recovered_exactly,
        "the recovery must be read back exactly"
    );
}

#[tokio::test]
async fn a_released_owner_lease_is_reclaimed_and_the_journal_rebinds() {
    let engine = EngineId::mihomo();
    let journal = MemoryEngineSwitchJournal::new(engine.clone());
    let identity = sandbox_identity(&engine);

    let snapshot = journal.snapshot().await.expect("the journal reads back");
    let (bound, lease) = journal
        .bind_owner(
            snapshot.revision,
            engine.clone(),
            identity.clone(),
            CoreGeneration::new(1),
        )
        .await
        .expect("the first owner binds");
    assert!(bound.bound_owner().is_some(), "the owner must be bound");

    // A terminal that does not match the owner fails closed and keeps the
    // binding in place.
    let foreign = CoreLifecycleSnapshot {
        identity: None,
        lifecycle: CoreLifecycle::Stopped,
        generation: 1,
        session_token: None,
        revision: 1,
        failure: None,
    };
    assert!(
        lease.release_after_terminal(&foreign).await.is_err(),
        "a foreign terminal must be refused"
    );
    assert!(
        journal
            .snapshot()
            .await
            .expect("the journal reads back")
            .bound_owner()
            .is_some(),
        "the binding must survive a refused release"
    );

    // The exact terminal releases the lease.
    let terminal = CoreLifecycleSnapshot {
        identity: Some(identity.clone()),
        lifecycle: CoreLifecycle::Stopped,
        generation: 1,
        session_token: None,
        revision: 2,
        failure: None,
    };
    lease
        .release_after_terminal(&terminal)
        .await
        .expect("the exact terminal releases the lease");
    assert!(
        journal
            .snapshot()
            .await
            .expect("the journal reads back")
            .bound_owner()
            .is_none(),
        "the owner must be released"
    );

    // The reclaimed journal binds a new owner.
    let revision = journal.snapshot().await.expect("the journal reads back").revision;
    let (rebound, _lease) = journal
        .bind_owner(revision, engine, identity, CoreGeneration::new(2))
        .await
        .expect("the reclaimed journal binds a new owner");
    assert!(rebound.bound_owner().is_some());
}

#[tokio::test]
async fn every_resource_the_sandbox_takes_is_released_again() {
    let sandbox = Sandbox::new(EngineId::mihomo());
    let runtime = sandbox.runtime().expect("the sandbox runtime builds");

    runtime.start().await.expect("the runtime starts");
    runtime.stop().await.expect("the runtime stops");

    assert_eq!(sandbox.process().starts(), 1);
    assert_eq!(sandbox.process().stops(), 1);
    assert_eq!(sandbox.process().phase(), ChaosPhase::Stopped);
    match runtime.state() {
        LurkerRuntimeState::Stopped { .. } => {}
        other => panic!("the runtime must publish the stopped terminal, got {other:?}"),
    }
}

/// The scenario that covers one fault of the model.
///
/// The match is exhaustive: a new fault must name its scenario here, so the
/// matrix can never grow a fault without a scenario.
fn scenario_of(fault: Fault) -> &'static str {
    match fault {
        Fault::NetworkFlap => "a_network_flap_fails_closed_and_the_runtime_heals_itself",
        Fault::KernelProcessDeath => "a_dead_kernel_fails_closed_and_leaves_no_running_claim",
        Fault::LockContention => "a_contended_write_fence_fails_closed_and_is_reclaimed",
        Fault::CorruptedConfiguration => {
            "a_corrupted_configuration_is_rejected_and_recovered"
        }
    }
}

#[test]
fn every_fault_of_the_model_names_its_scenario() {
    for fault in [
        Fault::NetworkFlap,
        Fault::KernelProcessDeath,
        Fault::LockContention,
        Fault::CorruptedConfiguration,
    ] {
        assert!(
            !scenario_of(fault).is_empty(),
            "the fault {fault:?} must name its scenario"
        );
    }
}
