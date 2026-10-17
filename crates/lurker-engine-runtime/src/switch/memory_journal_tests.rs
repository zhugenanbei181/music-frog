use super::*;
use futures_executor::block_on;
use lurker_core_contract::identity::CoreInstanceId;

fn instance(value: &str) -> CoreInstanceId {
    CoreInstanceId::new(value).expect("test instance ID is valid")
}

fn owner_identity(engine: &EngineId, instance_id: &str) -> CoreIdentity {
    CoreIdentity::new(engine.clone(), instance(instance_id), None)
}

fn terminal(identity: &CoreIdentity, generation: u64) -> CoreLifecycleSnapshot {
    CoreLifecycleSnapshot {
        identity: Some(identity.clone()),
        lifecycle: CoreLifecycle::Stopped,
        generation,
        session_token: None,
        revision: 1,
        failure: None,
    }
}

#[test]
fn a_live_lease_fences_a_second_bind_until_exact_terminal_release() {
    let engine = EngineId::mihomo();
    let journal = MemoryEngineSwitchJournal::new(engine.clone());
    let owner = owner_identity(&engine, "owner");
    let initial = block_on(journal.snapshot()).expect("initial snapshot reads");
    let (bound, lease) = block_on(journal.bind_owner(
        initial.revision,
        engine.clone(),
        owner.clone(),
        CoreGeneration::new(1),
    ))
    .expect("first owner binds");
    assert!(matches!(bound.owner, EngineOwnerState::Bound { .. }));

    let second = block_on(journal.bind_owner(
        bound.revision,
        engine.clone(),
        owner.clone(),
        CoreGeneration::new(1),
    ));
    assert!(second.is_err(), "a live lease must keep the journal bound");
    let live = block_on(journal.snapshot()).expect("live snapshot reads");
    assert!(matches!(live.owner, EngineOwnerState::Bound { .. }));
    assert_eq!(live.revision, bound.revision);

    let mut wrong_identity = terminal(&owner, 1);
    wrong_identity.identity = Some(owner_identity(&engine, "another-owner"));
    assert!(
        block_on(lease.release_after_terminal(&wrong_identity)).is_err(),
        "a terminal for another owner must fail closed"
    );
    let mut running = terminal(&owner, 1);
    running.lifecycle = CoreLifecycle::Running;
    assert!(
        block_on(lease.release_after_terminal(&running)).is_err(),
        "a non-terminal readback must fail closed"
    );
    assert!(
        block_on(lease.release_after_terminal(&terminal(&owner, 0))).is_err(),
        "a terminal older than the bound generation must fail closed"
    );
    let after_rejections = block_on(journal.snapshot()).expect("rejected snapshot reads");
    assert!(matches!(
        after_rejections.owner,
        EngineOwnerState::Bound { .. }
    ));
    assert_eq!(after_rejections.revision, bound.revision);

    block_on(lease.release_after_terminal(&terminal(&owner, 1)))
        .expect("the exact terminal releases the bound owner");
    let released = block_on(journal.snapshot()).expect("released snapshot reads");
    assert_eq!(released.owner, EngineOwnerState::Unowned);
    assert!(released.revision > bound.revision);

    let rebound =
        block_on(journal.bind_owner(released.revision, engine, owner, CoreGeneration::new(1)));
    assert!(rebound.is_ok(), "a released journal accepts a new owner");
}
