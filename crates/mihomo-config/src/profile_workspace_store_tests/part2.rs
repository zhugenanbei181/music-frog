use super::*;

#[tokio::test]
async fn failed_document_publication_restores_exact_sidecar_bytes_or_absence() {
    let dir = TempDir::new().unwrap();
    let sidecar = dir.path().join("options/main.yaml");
    let previous_values: [Option<&[u8]>; 3] =
        [None, Some(b""), Some(b"# byte-exact comment\r\n\xff\x00")];
    for previous in previous_values {
        if let Some(previous) = previous {
            write_document_bytes(&sidecar, previous).await.unwrap();
        }
        let failure = publish_document_pair(&sidecar, previous, Some(b"new sidecar"), async {
            Err(PortError::PermissionDenied(
                "profile replacement denied".into(),
            ))
        })
        .await
        .unwrap_err();
        assert!(matches!(failure, PortError::PermissionDenied(_)));
        assert_eq!(
            read_optional_bytes(&sidecar).await.unwrap().as_deref(),
            previous
        );
    }
}

#[tokio::test]
async fn rollback_failure_retains_both_causes_without_claiming_clean_rejection() {
    let dir = TempDir::new().unwrap();
    let sidecar = dir.path().join("options/main.yaml");
    write_document_bytes(&sidecar, b"# old\n\xff")
        .await
        .unwrap();
    let failure = publish_document_pair(&sidecar, Some(b"# old\n\xff"), Some(b"new"), async {
        remove_file(&sidecar).await.unwrap();
        create_dir_all(&sidecar).await.unwrap();
        Err(PortError::Io("profile publication refused".into()))
    })
    .await
    .unwrap_err();
    let failure = Failure::from(failure);
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert!(!failure.retryable);
    assert!(failure.message.contains("profile publication refused"));
    assert!(failure.message.contains("sidecar rollback failed"));
    assert!(sidecar.is_dir());
}

#[tokio::test]
async fn active_and_inactive_publications_check_selection_inside_the_write_boundary() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: rule\n")
        .await
        .unwrap();
    select_profile(&store, "main").await;
    let other_id = ProfileId::new("other").unwrap();
    let other = store
        .engine_profile_store()
        .load_workspace(&other_id)
        .await
        .unwrap();
    let selected = match store
        .engine_profile_store()
        .read_active_profile_reference()
        .await
        .unwrap()
    {
        ActiveProfileReferenceState::Selected(reference) => reference,
        state => panic!("expected selected profile, got {state:?}"),
    };
    let error = match store
        .engine_profile_store()
        .acquire_active_workspace_write_lease(
            selected,
            other.identity(),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
    {
        Ok(_) => panic!("active workspace lease accepted an inactive workspace identity"),
        Err(error) => error,
    };
    let failure = Failure::from(error);
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(
        store
            .engine_profile_store()
            .load_workspace(&other_id)
            .await
            .unwrap(),
        other
    );

    select_profile(&store, "other").await;
    let failure = Failure::from(
        store
            .engine_profile_store()
            .compare_and_save_inactive_workspace(
                &other.identity(),
                &update(b"mode: global\n", None),
            )
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    let selected = match store
        .engine_profile_store()
        .read_active_profile_reference()
        .await
        .unwrap()
    {
        ActiveProfileReferenceState::Selected(reference) => reference,
        state => panic!("expected selected profile, got {state:?}"),
    };
    let active_lease = store
        .engine_profile_store()
        .acquire_active_workspace_write_lease(
            selected,
            other.identity(),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
        .unwrap();
    let committed = publish_or_panic(active_lease, update(b"mode: global\n", None))
        .await
        .commit();
    assert_eq!(
        store
            .engine_profile_store()
            .load_active_workspace()
            .await
            .unwrap(),
        committed
    );
    assert!(!store.config_dir().join("other.yaml.bak").exists());
    assert_eq!(store.load("main").await.unwrap(), "mode: rule\n");
}

#[tokio::test]
async fn managed_workspace_lease_holds_reference_through_restore_and_native_recovery() {
    let dir = TempDir::new().unwrap();
    let first = manager(&dir);
    let second = manager(&dir);
    first
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    first
        .save_inactive_profile("other", "mode: rule\n")
        .await
        .unwrap();
    select_profile(&first, "main").await;

    let reference = match first
        .engine_profile_store()
        .read_active_profile_reference()
        .await
        .unwrap()
    {
        ActiveProfileReferenceState::Selected(reference) => reference,
        state => panic!("expected selected profile, got {state:?}"),
    };
    let previous = first
        .engine_profile_store()
        .load_workspace(&reference.profile_id)
        .await
        .unwrap();
    let observed = first
        .engine_profile_store()
        .acquire_active_workspace_write_lease(
            reference.clone(),
            previous.identity(),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
        .unwrap();
    assert_eq!(observed.observed_workspace(), &previous);

    let blocked_cas = timeout(
        Duration::from_millis(30),
        second
            .engine_profile_store()
            .compare_and_set_active_profile_reference(
                ActiveProfileReferenceState::Selected(reference.clone()),
                ActiveProfileReferenceTarget::selected(
                    ConfigManager::engine_id(&second),
                    ProfileId::new("other").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            ),
    )
    .await;
    assert!(
        blocked_cas.is_err(),
        "selection must wait for the active transaction"
    );

    let published = publish_or_panic(observed, update(b"mode: global\n", None)).await;
    assert_eq!(
        read(first.config_dir().join("main.yaml"))
            .await
            .unwrap(),
        b"mode: global\n"
    );
    let restored = match published.restore_if_current().await.unwrap() {
        ActiveProfileWorkspaceRestoreOutcome::Restored(restored) => restored,
        ActiveProfileWorkspaceRestoreOutcome::Superseded { observed } => {
            panic!("workspace unexpectedly superseded under its lease: {observed:?}")
        }
        ActiveProfileWorkspaceRestoreOutcome::RecoveryRequired(recovery) => {
            panic!(
                "workspace restore requires recovery: {}",
                recovery.last_error()
            )
        }
    };
    assert_eq!(restored.restored_workspace(), &previous);
    assert_eq!(
        read(first.config_dir().join("main.yaml"))
            .await
            .unwrap(),
        b"mode: rule\n"
    );

    let blocked_until_native_restore = timeout(
        Duration::from_millis(30),
        second
            .engine_profile_store()
            .compare_and_set_active_profile_reference(
                ActiveProfileReferenceState::Selected(reference.clone()),
                ActiveProfileReferenceTarget::selected(
                    ConfigManager::engine_id(&second),
                    ProfileId::new("other").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            ),
    )
    .await;
    assert!(
        blocked_until_native_restore.is_err(),
        "selection remains fenced until the caller confirms native recovery"
    );
    restored.finish();

    assert!(matches!(
        second
            .engine_profile_store()
            .compare_and_set_active_profile_reference(
                ActiveProfileReferenceState::Selected(reference),
                ActiveProfileReferenceTarget::selected(
                    ConfigManager::engine_id(&second),
                    ProfileId::new("other").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
            .unwrap(),
        ActiveProfileReferenceCasOutcome::Committed { .. }
    ));
}

#[tokio::test]
async fn reset_commits_default_reference_before_cleaning_profile_files() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("default", "mode: rule\n")
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: global\n")
        .await
        .unwrap();
    let previous = store
        .engine_profile_store()
        .read_active_profile_reference()
        .await
        .unwrap();
    let default_content = "mode: direct\n";
    let expected_revision = previous.revision().next().unwrap();

    let outcome = store
        .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
            expected: previous.clone(),
            default_document: default_content.to_owned(),
        })
        .await
        .unwrap();
    let current = match outcome {
        ActiveProfileReferenceResetOutcome::Committed {
            previous: committed_previous,
            current,
        } => {
            assert_eq!(committed_previous, previous);
            current
        }
        other => panic!("reset did not complete: {other:?}"),
    };
    assert!(matches!(
        &current,
        ActiveProfileReferenceState::Selected(reference)
            if reference.engine_id == ConfigManager::engine_id(&store)
                && reference.profile_id.as_str() == "default"
                && reference.revision == expected_revision
    ));
    assert_eq!(
        store
            .engine_profile_store()
            .read_active_profile_reference()
            .await
            .unwrap(),
        current
    );
    assert_eq!(store.load("default").await.unwrap(), default_content);
    assert!(!store.config_dir().join("other.yaml").exists());

    assert!(matches!(
        store
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
                expected: previous.clone(),
                default_document: default_content.to_owned(),
            })
            .await
            .unwrap(),
        ActiveProfileReferenceResetOutcome::Committed {
            previous: retried_previous,
            current: retried_current,
        } if retried_previous == previous && retried_current == current
    ));
}

#[tokio::test]
async fn reset_of_an_already_default_profile_advances_revision_and_completes_intent() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    let default_content = "mode: direct\n";
    store
        .save_inactive_profile("default", default_content)
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: global\n")
        .await
        .unwrap();
    select_profile(&store, "default").await;
    let expected = store
        .engine_profile_store()
        .read_active_profile_reference()
        .await
        .unwrap();
    let expected_next_revision = expected.revision().next().unwrap();

    let current = match store
        .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
            expected: expected.clone(),
            default_document: default_content.to_owned(),
        })
        .await
        .unwrap()
    {
        ActiveProfileReferenceResetOutcome::Committed { previous, current } => {
            assert_eq!(previous, expected);
            current
        }
        other => panic!("already-default reset did not complete: {other:?}"),
    };

    assert!(matches!(
        &current,
        ActiveProfileReferenceState::Selected(reference)
            if reference.engine_id == ConfigManager::engine_id(&store)
                && reference.profile_id.as_str() == "default"
                && reference.revision == expected_next_revision
    ));
    assert_eq!(
        store
            .engine_profile_store()
            .read_active_profile_reference()
            .await
            .unwrap(),
        current
    );
    assert_eq!(store.load("default").await.unwrap(), default_content);
    assert!(!store.config_dir().join("other.yaml").exists());
    assert!(
        !store
            .config_dir()
            .join(".active-profile-reset-intent.json")
            .exists()
    );
    assert!(
        store
            .config_dir()
            .join(".active-profile-reset-complete.json")
            .exists()
    );
}

#[tokio::test]
async fn reset_resumes_already_default_cleanup_from_its_durable_intent() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    let default_content = "mode: direct\n";
    store
        .save_inactive_profile("default", default_content)
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: global\n")
        .await
        .unwrap();
    select_profile(&store, "default").await;
    let expected = store
        .engine_profile_store()
        .read_active_profile_reference()
        .await
        .unwrap();
    select_profile(&store, "default").await;
    let current = store
        .engine_profile_store()
        .read_active_profile_reference()
        .await
        .unwrap();
    assert_eq!(current.revision(), expected.revision().next().unwrap());

    let digest = sha256_hex(default_content.as_bytes());
    let intent = serde_json::json!({
        "schema_version": 1,
        "expected_reference": expected,
        "target_reference": current,
        "default_document_sha256": digest,
    });
    let intent_path = store.config_dir().join(".active-profile-reset-intent.json");
    write(&intent_path, serde_json::to_vec(&intent).unwrap())
        .await
        .unwrap();

    let outcome = store
        .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
            expected: expected.clone(),
            default_document: default_content.to_owned(),
        })
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        ActiveProfileReferenceResetOutcome::Committed {
            previous,
            current: committed,
        } if previous == expected && committed == current
    ));
    assert!(!store.config_dir().join("other.yaml").exists());
    assert!(!intent_path.exists());
    assert!(
        store
            .config_dir()
            .join(".active-profile-reset-complete.json")
            .exists()
    );
}

#[tokio::test]
async fn stale_reset_leaves_profile_tree_untouched() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("default", "mode: rule\n")
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: global\n")
        .await
        .unwrap();
    let stale = store
        .engine_profile_store()
        .read_active_profile_reference()
        .await
        .unwrap();
    let default_before = read(store.config_dir().join("default.yaml"))
        .await
        .unwrap();
    let other_before = read(store.config_dir().join("other.yaml"))
        .await
        .unwrap();
    select_profile(&store, "other").await;

    assert!(matches!(
        store
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
                expected: stale.clone(),
                default_document: "mode: direct\n".to_owned(),
            })
            .await
            .unwrap(),
        ActiveProfileReferenceResetOutcome::Stale { expected, .. }
            if expected == stale
    ));
    assert_eq!(
        read(store.config_dir().join("default.yaml"))
            .await
            .unwrap(),
        default_before
    );
    assert_eq!(
        read(store.config_dir().join("other.yaml"))
            .await
            .unwrap(),
        other_before
    );
}

#[tokio::test]
async fn reset_rejects_a_bound_owner_and_preserves_profile_files() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("default", "mode: rule\n")
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: global\n")
        .await
        .unwrap();
    select_profile(&store, "default").await;
    let expected = store
        .engine_profile_store()
        .read_active_profile_reference()
        .await
        .unwrap();
    let previous_default = read(store.config_dir().join("default.yaml"))
        .await
        .unwrap();
    let previous_other = read(store.config_dir().join("other.yaml"))
        .await
        .unwrap();

    let profile_store = store.engine_profile_store();
    let selection = ActiveProfileReferenceStore::acquire_selection_lease(profile_store)
        .await
        .unwrap();
    let identity = CoreIdentity::new(
        ConfigManager::engine_id(&store),
        CoreInstanceId::new("reset-owner").unwrap(),
        None,
    );
    let owner = selection
        .into_owner_binding(
            identity.clone(),
            CoreGeneration::new(0),
        )
        .await
        .unwrap();

    let failure = Failure::from(
        store
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
                expected,
                default_document: "mode: direct\n".to_owned(),
            })
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(
        read(store.config_dir().join("default.yaml"))
            .await
            .unwrap(),
        previous_default
    );
    assert_eq!(
        read(store.config_dir().join("other.yaml"))
            .await
            .unwrap(),
        previous_other
    );

    owner
        .release_after_terminal(&CoreLifecycleSnapshot {
            identity: Some(identity),
            lifecycle: CoreLifecycle::Stopped,
            generation: 0,
            session_token: None,
            revision: 0,
            failure: None,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn exact_recovery_rejects_forged_or_later_documents_and_restores_raw_bytes() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    let previous_document = b"# preserve document bytes\r\nmode: rule\r\n";
    store
        .save_inactive_profile("main", str::from_utf8(previous_document).unwrap())
        .await
        .unwrap();
    let path = options_path(store.config_dir(), "main");
    create_dir_all(path.parent().unwrap()).await.unwrap();
    let previous_sidecar = b"# retain exact annotation\r\n\xff\x00";
    write(&path, previous_sidecar).await.unwrap();
    let previous = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    let committed = store
        .engine_profile_store()
        .compare_and_save_inactive_workspace(
            &previous.identity(),
            &update(b"mode: global\n", Some(b"new-sidecar")),
        )
        .await
        .unwrap();
    let mut forged = previous.clone();
    forged.document.raw_bytes = b"mode: direct\n".to_vec();
    assert_eq!(
        Failure::from(
            store
                .engine_profile_store()
                .restore_inactive_workspace(&committed.identity(), &forged)
                .await
                .unwrap_err()
        )
        .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        store
            .engine_profile_store()
            .load_workspace(&profile_id())
            .await
            .unwrap(),
        committed
    );

    let recovered = store
        .engine_profile_store()
        .restore_inactive_workspace(&committed.identity(), &previous)
        .await
        .unwrap();
    assert_eq!(recovered.document.raw_bytes, previous_document);
    assert_eq!(
        recovered.sidecar_bytes.as_deref(),
        Some(previous_sidecar.as_slice())
    );
    assert_eq!(read(&path).await.unwrap(), previous_sidecar);

    let later = store
        .engine_profile_store()
        .compare_and_save_inactive_workspace(
            &recovered.identity(),
            &update(b"mode: direct\n", Some(previous_sidecar)),
        )
        .await
        .unwrap();
    assert_eq!(
        Failure::from(
            store
                .engine_profile_store()
                .restore_inactive_workspace(&committed.identity(), &previous)
                .await
                .unwrap_err()
        )
        .code,
        ErrorCode::NotReady
    );
    assert_eq!(
        store
            .engine_profile_store()
            .load_workspace(&profile_id())
            .await
            .unwrap(),
        later
    );
}

#[tokio::test]
async fn deleting_profile_removes_its_sidecar() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    store
        .save_inactive_profile("other", "mode: rule\n")
        .await
        .unwrap();
    let sidecar = options_path(store.config_dir(), "other");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"# keep no orphan\n").await.unwrap();

    store.delete_profile("other").await.unwrap();

    assert!(!store.config_dir().join("other.yaml").exists());
    assert!(!sidecar.exists());
}
