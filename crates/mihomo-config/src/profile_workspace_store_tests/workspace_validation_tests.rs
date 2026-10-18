use super::*;

#[tokio::test]
async fn changed_subscription_source_rejects_stale_workspace_before_writing() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"# retain current sidecar\r\n")
        .await
        .unwrap();
    let settings = store.config_dir().join("profile-state.toml");
    write(
        &settings,
        "[profiles.main]\nsubscription_url = \"https://one.example/sub\"\n",
    )
    .await
    .unwrap();
    let original = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    assert_eq!(
        original.document.identity.source.kind,
        ProfileDocumentSourceKind::Subscription
    );
    write(
        &settings,
        "[profiles.main]\nsubscription_url = \"https://two.example/sub\"\n",
    )
    .await
    .unwrap();
    let profile_path = store.config_dir().join("main.yaml");
    let before = read(&profile_path).await.unwrap();

    let failure = Failure::from(
        store
            .engine_profile_store()
            .compare_and_save_inactive_workspace(
                &original.identity(),
                &update(b"mode: global\n", None),
            )
            .await
            .unwrap_err(),
    );

    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(read(&profile_path).await.unwrap(), before);
    assert_eq!(
        read(&sidecar).await.unwrap(),
        b"# retain current sidecar\r\n"
    );
}

#[tokio::test]
async fn foreign_engine_workspace_is_rejected_before_profile_path_access() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    let configs = dir.path().join("configs");
    write(&configs, "sentinel").await.unwrap();
    let profile = profile_id();
    let expected = foreign_identity(&profile);
    let previous = ProfileWorkspace {
        document: ProfileDocument::new(expected.document.clone(), b"mode: rule\n".to_vec()),
        sidecar_revision: ProfileSidecarRevision::Absent,
        write_protection: ProfileWriteProtection::Editable,
        sidecar_bytes: None,
    };

    let error = store
        .engine_profile_store()
        .compare_and_save_inactive_workspace(&expected, &update(b"mode: global\n", None))
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        PortError::Unsupported {
            capability: Capability::Profiles,
            ..
        }
    ));

    let error = store
        .engine_profile_store()
        .restore_inactive_workspace(&expected, &previous)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        PortError::Unsupported {
            capability: Capability::Profiles,
            ..
        }
    ));
    assert_eq!(read(&configs).await.unwrap(), b"sentinel");
    assert!(!store.config_dir().join("profile-state.toml").exists());
}

#[tokio::test]
async fn unsupported_mihomo_format_and_schema_reject_before_profile_path_access() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    let configs = store.config_dir().to_path_buf();
    create_dir_all(configs.parent().unwrap()).await.unwrap();
    write(&configs, b"sentinel").await.unwrap();

    for (format_id, schema_version) in [("sing-box-json", 1), (PROFILE_FORMAT, 2)] {
        let expected = mihomo_identity_with_format(&profile_id(), format_id, schema_version);
        let previous = ProfileWorkspace {
            document: ProfileDocument::new(expected.document.clone(), b"mode: rule\n".to_vec()),
            sidecar_revision: ProfileSidecarRevision::Absent,
            write_protection: ProfileWriteProtection::Editable,
            sidecar_bytes: None,
        };

        let compare_error = store
            .engine_profile_store()
            .compare_and_save_inactive_workspace(&expected, &update(b"mode: global\n", None))
            .await
            .unwrap_err();
        assert!(matches!(
            compare_error,
            PortError::Unsupported {
                capability: Capability::Profiles,
                ..
            }
        ));

        let restore_error = store
            .engine_profile_store()
            .restore_inactive_workspace(&expected, &previous)
            .await
            .unwrap_err();
        assert!(matches!(
            restore_error,
            PortError::Unsupported {
                capability: Capability::Profiles,
                ..
            }
        ));
        assert_eq!(read(&configs).await.unwrap(), b"sentinel");
        assert!(!store.config_dir().join("profile-state.toml").exists());
    }
}

#[tokio::test]
async fn document_and_sidecar_revision_must_match_the_exact_raw_bytes() {
    let dir = TempDir::new().unwrap();
    let store = manager(&dir);
    store
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    let current = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    let profile_path = store.config_dir().join("main.yaml");
    let before = read(&profile_path).await.unwrap();
    let mut forged = current.clone();
    forged.document.raw_bytes = b"mode: direct\n".to_vec();

    let failure = Failure::from(
        store
            .engine_profile_store()
            .restore_inactive_workspace(&current.identity(), &forged)
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(read(&profile_path).await.unwrap(), before);

    let mut forged = current.clone();
    forged.sidecar_bytes = Some(b"# bytes without matching revision\n".to_vec());
    let failure = Failure::from(
        store
            .engine_profile_store()
            .restore_inactive_workspace(&current.identity(), &forged)
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(read(&profile_path).await.unwrap(), before);

    let sidecar = options_path(store.config_dir(), "main");
    create_dir_all(sidecar.parent().unwrap()).await.unwrap();
    write(&sidecar, b"\xff\x00#opaque\n").await.unwrap();
    let current = store
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap();
    assert_eq!(
        current.sidecar_bytes.as_deref(),
        Some(b"\xff\x00#opaque\n".as_slice())
    );
    assert_eq!(
        current.sidecar_revision,
        ProfileSidecarRevision::Present(document_revision(b"\xff\x00#opaque\n"))
    );
}

#[tokio::test]
async fn separate_managers_share_the_cas_boundary_and_only_one_proposal_commits() {
    let dir = TempDir::new().unwrap();
    let first = Arc::new(manager(&dir));
    first
        .save_inactive_profile("main", "mode: rule\n")
        .await
        .unwrap();
    let second = Arc::new(manager(&dir));
    let guard = first.lock_profile_writes().await.unwrap();
    assert!(
        timeout(
            Duration::from_millis(20),
            second.save_inactive_profile("main", "mode: direct\n")
        )
        .await
        .is_err()
    );
    drop(guard);
    assert_eq!(first.load("main").await.unwrap(), "mode: rule\n");

    let source = first
        .engine_profile_store()
        .load_workspace(&profile_id())
        .await
        .unwrap()
        .identity();
    let first_update = update(b"mode: global\n", Some(b"global-sidecar"));
    let second_update = update(b"mode: direct\n", Some(b"direct-sidecar"));
    let (left, right) = tokio::join!(
        first
            .engine_profile_store()
            .compare_and_save_inactive_workspace(&source, &first_update),
        second
            .engine_profile_store()
            .compare_and_save_inactive_workspace(&source, &second_update),
    );
    let (winner, rejected) = match (left, right) {
        (Ok(winner), Err(rejected)) | (Err(rejected), Ok(winner)) => (winner, rejected),
        results => panic!("exactly one same-source proposal may publish: {results:?}"),
    };
    assert_eq!(Failure::from(rejected).code, ErrorCode::NotReady);
    assert_eq!(
        first
            .engine_profile_store()
            .load_workspace(&profile_id())
            .await
            .unwrap(),
        winner
    );
    assert_eq!(
        second
            .engine_profile_store()
            .load_workspace(&profile_id())
            .await
            .unwrap(),
        winner
    );
    assert!(
        winner.sidecar_bytes.as_deref() == Some(b"global-sidecar".as_slice())
            || winner.sidecar_bytes.as_deref() == Some(b"direct-sidecar".as_slice())
    );
}
