use super::*;

#[tokio::test]
async fn subprocess_compare_and_set_entrypoint() {
    use std::fs;

    let Ok(home) = env::var("MUSIC_FROG_ACTIVE_REFERENCE_CAS_HOME") else {
        return;
    };
    let ready = env::var("MUSIC_FROG_ACTIVE_REFERENCE_CAS_READY").unwrap();
    let gate = env::var("MUSIC_FROG_ACTIVE_REFERENCE_CAS_GATE").unwrap();
    let result_path = env::var("MUSIC_FROG_ACTIVE_REFERENCE_CAS_RESULT").unwrap();
    let engine = env::var("MUSIC_FROG_ACTIVE_REFERENCE_CAS_ENGINE").unwrap();
    fs::write(ready, "ready").unwrap();
    wait_for_marker(Path::new(&gate)).await;
    let manager = manager_at(Path::new(&home));
    let result = manager
        .engine_profile_store()
        .compare_and_set(
            ActiveProfileReferenceRevision::new(0),
            ActiveProfileReferenceTarget::selected(
                EngineId::new(engine).unwrap(),
                ProfileId::new("default").unwrap(),
            ),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await;
    let outcome = match result {
        Ok(state) => format!("committed:{}", state.revision().value()),
        Err(PortError::Rejected(_)) => "stale".to_owned(),
        Err(error) => format!("error:{error}"),
    };
    fs::write(result_path, outcome).unwrap();
}

#[tokio::test]
async fn compare_and_set_has_one_cross_process_revision_winner() {
    use std::fs;

    let directory = TempDir::new().unwrap();
    let home = directory.path().to_string_lossy().into_owned();
    let gate = directory.path().join("cas-start");
    let ready_a = directory.path().join("cas-ready-a");
    let ready_b = directory.path().join("cas-ready-b");
    let result_a = directory.path().join("cas-result-a");
    let result_b = directory.path().join("cas-result-b");
    let ready_a_text = ready_a.to_string_lossy().into_owned();
    let ready_b_text = ready_b.to_string_lossy().into_owned();
    let gate_text = gate.to_string_lossy().into_owned();
    let result_a_text = result_a.to_string_lossy().into_owned();
    let result_b_text = result_b.to_string_lossy().into_owned();
    let child_a = spawn_test_child(
        "subprocess_compare_and_set_entrypoint",
        &[
            ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_HOME", &home),
            ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_READY", &ready_a_text),
            ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_GATE", &gate_text),
            ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_RESULT", &result_a_text),
            ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_ENGINE", "engine-a"),
        ],
    );
    let child_b = spawn_test_child(
        "subprocess_compare_and_set_entrypoint",
        &[
            ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_HOME", &home),
            ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_READY", &ready_b_text),
            ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_GATE", &gate_text),
            ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_RESULT", &result_b_text),
            ("MUSIC_FROG_ACTIVE_REFERENCE_CAS_ENGINE", "engine-b"),
        ],
    );
    wait_for_marker(&ready_a).await;
    wait_for_marker(&ready_b).await;
    fs::write(&gate, "go").unwrap();
    assert!(wait_child(child_a).await.success());
    assert!(wait_child(child_b).await.success());
    let results = [
        fs::read_to_string(result_a).unwrap(),
        fs::read_to_string(result_b).unwrap(),
    ];
    assert_eq!(
        results
            .iter()
            .filter(|result| result.starts_with("committed:1"))
            .count(),
        1
    );
    assert_eq!(
        results.iter().filter(|result| *result == "stale").count(),
        1
    );
    assert!(!results.iter().any(|result| result.starts_with("error:")));
    assert_eq!(
        manager_at(directory.path())
            .engine_profile_store()
            .read()
            .await
            .unwrap()
            .revision()
            .value(),
        1
    );
}

#[tokio::test]
async fn subprocess_owner_lock_fences_cas_and_process_exit_releases_it() {
    use std::fs;

    let directory = TempDir::new().unwrap();
    let home = directory.path().to_string_lossy().into_owned();
    let ready = directory.path().join("owner-ready");
    let release = directory.path().join("owner-release");
    let ready_text = ready.to_string_lossy().into_owned();
    let release_text = release.to_string_lossy().into_owned();
    let child = spawn_test_child(
        "subprocess_owner_fence_entrypoint",
        &[
            ("MUSIC_FROG_OWNER_FENCE_HOME", &home),
            ("MUSIC_FROG_OWNER_FENCE_READY", &ready_text),
            ("MUSIC_FROG_OWNER_FENCE_RELEASE", &release_text),
        ],
    );
    wait_for_marker(&ready).await;
    let manager = manager_at(directory.path());
    let foreign_engine = EngineId::new("engine-after-crash").unwrap();
    let owner_context = CoreProcessStartContext::managed(
        identity("engine-live", "live-owner"),
        CoreGeneration::new(2),
        SessionToken::new(42),
    );
    assert!(matches!(
        manager
            .engine_profile_store()
            .compare_and_set(
                ActiveProfileReferenceRevision::new(1),
                ActiveProfileReferenceTarget::selected(
                    foreign_engine.clone(),
                    ProfileId::new("default").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await,
        Err(PortError::Rejected(_))
    ));
    assert!(matches!(
        manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await,
        Err(PortError::Rejected(_))
    ));
    assert!(
        manager
            .engine_profile_store()
            .recover_bound_owner_after_terminal(OwnerRecoveryEvidence::ManagedChildExited {
                context: owner_context.clone(),
                terminal: stopped_snapshot(identity("engine-live", "live-owner"), 2),
            })
            .await
            .is_err()
    );
    fs::write(&release, "exit").unwrap();
    assert!(wait_child(child).await.success());

    // The kernel lock disappearing proves process exit, not that a
    // supervised Core child reached Stopped. Keep the persisted Bound
    // proof fenced until the platform supplies exact terminal evidence.
    assert!(matches!(
        manager
            .engine_profile_store()
            .compare_and_set(
                ActiveProfileReferenceRevision::new(1),
                ActiveProfileReferenceTarget::selected(
                    foreign_engine.clone(),
                    ProfileId::new("default").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await,
        Err(PortError::Rejected(_))
    ));
    let wrong_session = CoreProcessStartContext::managed(
        identity("engine-live", "live-owner"),
        CoreGeneration::new(2),
        SessionToken::new(43),
    );
    assert!(
        manager
            .engine_profile_store()
            .recover_bound_owner_after_terminal(OwnerRecoveryEvidence::ManagedChildExited {
                context: wrong_session,
                terminal: stopped_snapshot(identity("engine-live", "live-owner"), 2),
            })
            .await
            .is_err()
    );
    let wrong_generation = CoreProcessStartContext::managed(
        identity("engine-live", "live-owner"),
        CoreGeneration::new(3),
        SessionToken::new(42),
    );
    assert!(
        manager
            .engine_profile_store()
            .recover_bound_owner_after_terminal(OwnerRecoveryEvidence::ManagedChildExited {
                context: wrong_generation,
                terminal: stopped_snapshot(identity("engine-live", "live-owner"), 3),
            })
            .await
            .is_err()
    );
    let owner_terminal = stopped_snapshot(identity("engine-live", "live-owner"), 2);
    assert!(matches!(
        manager
            .engine_profile_store()
            .recover_bound_owner_after_terminal(OwnerRecoveryEvidence::ManagedChildExited {
                context: owner_context,
                terminal: owner_terminal,
            })
            .await
            .unwrap(),
        OwnerRecoveryOutcome::Recovered(_)
    ));

    let after_exit = manager
        .engine_profile_store()
        .compare_and_set(
            ActiveProfileReferenceRevision::new(1),
            ActiveProfileReferenceTarget::selected(
                foreign_engine,
                ProfileId::new("default").unwrap(),
            ),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
        .unwrap();
    assert_eq!(after_exit.revision().value(), 2);
}

#[tokio::test]
async fn subprocess_owner_fence_entrypoint() {
    use std::fs;

    let Ok(home) = env::var("MUSIC_FROG_OWNER_FENCE_HOME") else {
        return;
    };
    let ready = env::var("MUSIC_FROG_OWNER_FENCE_READY").unwrap();
    let release = env::var("MUSIC_FROG_OWNER_FENCE_RELEASE").unwrap();
    let manager = manager_at(Path::new(&home));
    let engine = EngineId::new("engine-live").unwrap();
    let selected = manager
        .engine_profile_store()
        .compare_and_set(
            ActiveProfileReferenceRevision::new(0),
            ActiveProfileReferenceTarget::selected(
                engine.clone(),
                ProfileId::new("default").unwrap(),
            ),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
        .unwrap();
    let owner = manager
        .engine_profile_store()
        .acquire_provider_selection_lease()
        .await
        .unwrap()
        .into_owner_binding(
            identity(engine.as_str(), "live-owner"),
            CoreGeneration::new(1),
        )
        .await
        .unwrap();
    let owner_context = CoreProcessStartContext::managed(
        identity(engine.as_str(), "live-owner"),
        CoreGeneration::new(2),
        SessionToken::new(42),
    );
    owner.record_start_context(&owner_context).await.unwrap();
    assert_eq!(selected.revision().value(), 1);
    fs::write(ready, "owner-bound").unwrap();
    wait_for_marker(Path::new(&release)).await;
    drop(owner);
    // No terminal release: process exit must release only the kernel
    // fence, and the next store instance must prove the stale binding.
}
