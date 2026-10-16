use super::*;

#[test]
    fn v1_bound_owner_proof_migrates_as_legacy_unverified() {

        let directory = TempDir::new().unwrap();
        let proof_path = directory.path().join(ACTIVE_CORE_OWNER_STATE_FILE);
        let legacy_identity = identity("mihomo", "legacy-owner");
        let legacy = serde_json::json!({
            "schema_version": LEGACY_ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION,
            "state": {
                "state": "bound",
                "owner": {
                    "identity": legacy_identity,
                    "generation": 1,
                    "token": "legacy-token"
                }
            }
        });
        use std::fs;
        fs::write(&proof_path, serde_json::to_vec(&legacy).unwrap()).unwrap();

        let migrated = sync_read_active_core_owner_proof(directory.path()).unwrap();

        assert_eq!(
            migrated.schema_version,
            ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION
        );
        assert!(matches!(migrated.state, ActiveCoreOwnerProof::Bound { .. }));
        assert_eq!(
            migrated.launch,
            ActiveCoreOwnerLaunchProof::LegacyUnverified
        );
    }

    #[tokio::test]
    async fn managed_start_context_is_persisted_and_advances_by_generation() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        manager.migrate_legacy_mihomo_selection().await.unwrap();
        let owner_identity = identity(crate::engine_id().as_str(), "managed-session-owner");
        let owner = manager
            .engine_profile_store()
            .acquire_provider_selection_lease()
            .await
            .unwrap()
            .into_owner_binding(owner_identity.clone(), CoreGeneration::new(1))
            .await
            .unwrap();
        let first = CoreProcessStartContext::managed(
            owner_identity.clone(),
            CoreGeneration::new(2),
            SessionToken::new(42),
        );
        owner.record_start_context(&first).await.unwrap();

        let second_same_generation = CoreProcessStartContext::managed(
            owner_identity.clone(),
            CoreGeneration::new(2),
            SessionToken::new(43),
        );
        assert!(
            owner
                .record_start_context(&second_same_generation)
                .await
                .is_err()
        );
        let next = CoreProcessStartContext::managed(
            owner_identity,
            CoreGeneration::new(3),
            SessionToken::new(43),
        );
        owner.record_start_context(&next).await.unwrap();

        let restarted_store = manager_at(directory.path());
        assert_eq!(
            sync_read_active_core_owner_proof(restarted_store.config_root())
                .unwrap()
                .launch,
            ActiveCoreOwnerLaunchProof::Managed(next)
        );
    }

    fn manager(directory: &TempDir) -> ConfigManager<TestStore> {
        manager_at(directory.path())
    }

    fn manager_at(home: &Path) -> ConfigManager<TestStore> {
        ConfigManager::with_home_and_store(home.to_path_buf(), TestStore)
            .expect("isolated config manager")
    }

    #[tokio::test]
    async fn pending_reset_in_another_engine_blocks_reference_and_sidecar_mutations() {

        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        let engines_root = manager.config_root().join("engines");
        let other_engine_dir = engines_root.join("sing-box");
        use std::fs;
        fs::create_dir_all(&other_engine_dir).unwrap();
        let source = ActiveProfileReferenceState::Unselected {
            revision: ActiveProfileReferenceRevision::new(0),
        };
        let target = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            EngineId::new("sing-box").unwrap(),
            ProfileId::new("default").unwrap(),
            ActiveProfileReferenceRevision::new(1),
        ));
        let intent = ActiveProfileResetIntentRecord::new(source, target, b"mode: direct\n");
        let intent_path = other_engine_dir.join(ACTIVE_PROFILE_RESET_INTENT_FILE);
        fs::write(&intent_path, serde_json::to_vec(&intent).unwrap()).unwrap();

        assert!(matches!(
            manager
                .engine_profile_store()
                .compare_and_set(
                    ActiveProfileReferenceRevision::new(0),
                    ActiveProfileReferenceTarget::selected(
                        manager.engine_id(),
                        ProfileId::new("default").unwrap(),
                    ),
                    ActiveProfileReferenceWriteAuthorization::Unowned,
                )
                .await,
            Err(PortError::Rejected(Failure {
                code: ErrorCode::NotReady,
                retryable: true,
                ..
            }))
        ));
        assert!(matches!(
            manager
                .engine_profile_store()
                .acquire_selection_lease()
                .await,
            Err(PortError::Rejected(Failure {
                code: ErrorCode::NotReady,
                retryable: true,
                ..
            }))
        ));
        assert!(matches!(
            write_options_bytes(
                manager.config_dir(),
                "default",
                Some(b"{}"),
            )
            .await,
            Err(PortError::Rejected(Failure {
                code: ErrorCode::NotReady,
                retryable: true,
                ..
            }))
        ));
        assert!(
            !options_path(manager.config_dir(), "default")
                .exists()
        );
    }

    #[tokio::test]
    async fn reset_resume_pending_v2_needs_no_new_default_or_port_discovery() {
        let directory = TempDir::new().unwrap();
        let initial_manager = manager(&directory);
        let expected = initial_manager
            .migrate_legacy_mihomo_selection()
            .await
            .unwrap();
        let current = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            initial_manager.engine_id(),
            ProfileId::new("default").unwrap(),
            expected.revision().next().unwrap(),
        ));
        let default_path = initial_manager
            .existing_profile_document_path("default")
            .await
            .unwrap();
        let previous_default = fs::read(&default_path).await.unwrap();
        let persisted_default = b"mode: rule\nmixed-port: 7890\n";
        fs::write(
            initial_manager
                .config_dir()
                .join(ACTIVE_PROFILE_RESET_DEFAULT_BACKUP_FILE),
            &previous_default,
        )
        .await
        .unwrap();
        let intent = ActiveProfileResetIntentRecord::new(
            expected.clone(),
            current.clone(),
            persisted_default,
        );
        let intent_path = initial_manager
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_INTENT_FILE);
        fs::write(&intent_path, serde_json::to_vec(&intent).unwrap())
            .await
            .unwrap();

        let restarted = manager(&directory);
        assert!(restarted.initialize_profile_namespace().await.is_err());
        restarted
            .initialize_profile_namespace_for_reset_recovery()
            .await
            .unwrap();
        let outcome = restarted
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::ResumePendingIntent)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            ActiveProfileReferenceResetOutcome::Committed {
                previous: expected.clone(),
                current: current.clone(),
            }
        );
        assert_eq!(
            fs::read(default_path).await.unwrap().as_slice(),
            persisted_default
        );
        assert!(!intent_path.exists());
        let restarted_profile_store = restarted.engine_profile_store();
        assert_eq!(
            ActiveProfileReferenceStore::read(restarted_profile_store)
                .await
                .unwrap(),
            current
        );
    }

    #[tokio::test]
    async fn reset_resume_without_intent_allows_normal_typed_start() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);
        let expected = manager.migrate_legacy_mihomo_selection().await.unwrap();
        assert_eq!(
            manager
                .reset_profiles_to_default(ActiveProfileReferenceResetRequest::ResumePendingIntent)
                .await
                .unwrap(),
            ActiveProfileReferenceResetOutcome::NoPendingIntent
        );

        let outcome = manager
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
                expected: expected.clone(),
                default_document: "mode: direct\n".to_owned(),
            })
            .await
            .unwrap();
        assert!(matches!(
            outcome,
            ActiveProfileReferenceResetOutcome::Committed { previous, current }
                if previous == expected && current.revision() == previous.revision().next().unwrap()
        ));
    }

    #[tokio::test]
    async fn legacy_reset_intent_upgrades_when_default_file_proves_exact_bytes() {
        let directory = TempDir::new().unwrap();
        let initial_manager = manager(&directory);
        let expected = initial_manager
            .migrate_legacy_mihomo_selection()
            .await
            .unwrap();
        let current = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            initial_manager.engine_id(),
            ProfileId::new("default").unwrap(),
            expected.revision().next().unwrap(),
        ));
        let default_path = initial_manager
            .existing_profile_document_path("default")
            .await
            .unwrap();
        let previous_default = fs::read(&default_path).await.unwrap();
        let recovered_default = b"mode: rule\nmixed-port: 7890\n";
        fs::write(
            initial_manager
                .config_dir()
                .join(ACTIVE_PROFILE_RESET_DEFAULT_BACKUP_FILE),
            previous_default,
        )
        .await
        .unwrap();
        let legacy_intent = serde_json::json!({
            "schema_version": LEGACY_ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION,
            "expected_reference": expected.clone(),
            "target_reference": current.clone(),
            "default_document_sha256": sha256_hex(recovered_default),
        });
        let intent_path = initial_manager
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_INTENT_FILE);
        fs::write(&intent_path, serde_json::to_vec(&legacy_intent).unwrap())
            .await
            .unwrap();
        fs::write(&default_path, recovered_default)
            .await
            .unwrap();

        let restarted = manager(&directory);
        restarted
            .initialize_profile_namespace_for_reset_recovery()
            .await
            .unwrap();
        let outcome = restarted
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::ResumePendingIntent)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            ActiveProfileReferenceResetOutcome::Committed {
                previous: expected,
                current,
            }
        );
        assert_eq!(
            fs::read(default_path).await.unwrap(),
            recovered_default
        );
        let complete = fs::read(
            restarted
                .config_dir()
                .join(ACTIVE_PROFILE_RESET_COMPLETE_FILE),
        )
        .await
        .unwrap();
        assert!(matches!(
            parse_reset_intent_record(&complete).unwrap(),
            ParsedActiveProfileResetIntentRecord::CurrentV2(_)
        ));
        assert!(!intent_path.exists());
    }

    #[tokio::test]
    async fn reset_restart_finishes_committed_cleanup_with_changed_default_input() {
        let directory = TempDir::new().unwrap();
        let initial_manager = manager(&directory);
        let expected = initial_manager
            .migrate_legacy_mihomo_selection()
            .await
            .unwrap();
        let current = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            initial_manager.engine_id(),
            ProfileId::new("default").unwrap(),
            expected.revision().next().unwrap(),
        ));
        let persisted_default = "mode: rule\nmixed-port: 7890\n";
        assert!(matches!(
            initial_manager
                .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
                    expected: expected.clone(),
                    default_document: persisted_default.to_owned(),
                })
                .await
                .unwrap(),
            ActiveProfileReferenceResetOutcome::Committed { .. }
        ));

        let intent = ActiveProfileResetIntentRecord::new(
            expected.clone(),
            current.clone(),
            persisted_default.as_bytes(),
        );
        let intent_path = initial_manager
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_INTENT_FILE);
        let complete_path = initial_manager
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_COMPLETE_FILE);
        fs::write(&intent_path, serde_json::to_vec(&intent).unwrap())
            .await
            .unwrap();
        fs::remove_file(&complete_path).await.unwrap();
        let obsolete_path = initial_manager.config_dir().join("obsolete.yaml");
        fs::write(&obsolete_path, b"mode: direct\n")
            .await
            .unwrap();

        let restarted = manager(&directory);
        assert!(restarted.initialize_profile_namespace().await.is_err());
        restarted
            .initialize_profile_namespace_for_reset_recovery()
            .await
            .unwrap();
        let outcome = restarted
            .reset_profiles_to_default(ActiveProfileReferenceResetRequest::ResumePendingIntent)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            ActiveProfileReferenceResetOutcome::Committed {
                previous: expected,
                current,
            }
        );
        assert_eq!(
            fs::read(
                restarted
                    .existing_profile_document_path("default")
                    .await
                    .unwrap()
            )
            .await
            .unwrap()
            .as_slice(),
            persisted_default.as_bytes()
        );
        assert!(!obsolete_path.exists());
        assert!(!intent_path.exists());
    }

    #[tokio::test]
    async fn legacy_reset_intent_fails_closed_when_its_default_bytes_are_unrecoverable() {
        let directory = TempDir::new().unwrap();
        let initial_manager = manager(&directory);
        let expected = initial_manager
            .migrate_legacy_mihomo_selection()
            .await
            .unwrap();
        let current = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            initial_manager.engine_id(),
            ProfileId::new("default").unwrap(),
            expected.revision().next().unwrap(),
        ));
        let previous_default = fs::read(
            initial_manager
                .existing_profile_document_path("default")
                .await
                .unwrap(),
        )
        .await
        .unwrap();
        let legacy_target_bytes = b"mode: legacy-target\n";
        let intent_value = serde_json::json!({
            "schema_version": LEGACY_ACTIVE_PROFILE_RESET_INTENT_SCHEMA_VERSION,
            "expected_reference": expected.clone(),
            "target_reference": current,
            "default_document_sha256": sha256_hex(legacy_target_bytes),
        });
        let intent_path = initial_manager
            .config_dir()
            .join(ACTIVE_PROFILE_RESET_INTENT_FILE);
        let intent_bytes = serde_json::to_vec(&intent_value).unwrap();
        fs::write(&intent_path, &intent_bytes).await.unwrap();

        let restarted = manager(&directory);
        restarted
            .initialize_profile_namespace_for_reset_recovery()
            .await
            .unwrap();
        assert!(
            restarted
                .reset_profiles_to_default(ActiveProfileReferenceResetRequest::ResumePendingIntent)
                .await
                .is_err()
        );

        assert_eq!(
            fs::read(
                restarted
                    .existing_profile_document_path("default")
                    .await
                    .unwrap()
            )
            .await
            .unwrap()
            .as_slice(),
            previous_default.as_slice()
        );
        assert_eq!(fs::read(intent_path).await.unwrap(), intent_bytes);
        let restarted_profile_store = restarted.engine_profile_store();
        assert_eq!(
            ActiveProfileReferenceStore::read(restarted_profile_store)
                .await
                .unwrap(),
            expected
        );
    }

    

    #[tokio::test]
    async fn migration_persists_the_exact_legacy_mihomo_profile_once() {
        let directory = TempDir::new().unwrap();
        let config = manager(&directory);
        config
            .save_inactive_profile("default", "mixed-port: 7890\n")
            .await
            .unwrap();
        config
            .save_inactive_profile("legacy-choice", "mixed-port: 7890\n")
            .await
            .unwrap();
        fs::write(
            config.settings_file(),
            "[default]\nprofile = \"legacy-choice\"\n",
        )
        .await
        .unwrap();

        let migrated = config.migrate_legacy_mihomo_selection().await.unwrap();

        assert!(matches!(
            &migrated,
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == crate::engine_id()
                    && reference.profile_id.as_str() == "legacy-choice"
                    && reference.revision.value() == 1
        ));
        let record = config.read_active_profile_record_unlocked().await.unwrap();
        assert_eq!(
            record.legacy_migration,
            LegacySelectionMigration::MigratedDesktopMihomoV0
        );

        let explicit_clear = config
            .engine_profile_store()
            .compare_and_set(
                migrated.revision(),
                ActiveProfileReferenceTarget::Unselected,
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
            .unwrap();
        let local_pointer = config.get_current_unlocked().await.unwrap();
        config.migrate_legacy_mihomo_selection().await.unwrap();

        assert_eq!(
            config.engine_profile_store().read().await.unwrap(),
            explicit_clear
        );
        assert_eq!(config.get_current_unlocked().await.unwrap(), local_pointer);
        assert_eq!(
            config
                .read_active_profile_record_unlocked()
                .await
                .unwrap()
                .legacy_migration,
            LegacySelectionMigration::ExplicitSelection
        );

        let restarted = manager(&directory);
        let after_restart = restarted.migrate_legacy_mihomo_selection().await.unwrap();
        assert_eq!(after_restart, explicit_clear);
        assert_eq!(
            restarted.get_current_unlocked().await.unwrap(),
            local_pointer
        );
    }

    #[tokio::test]
    async fn pending_v0_migration_bootstraps_default_for_a_new_install() {
        let directory = TempDir::new().unwrap();
        let manager = manager(&directory);

        let selected = timeout(
            Duration::from_secs(3),
            manager.migrate_legacy_mihomo_selection(),
        )
        .await
        .expect("new-install migration must not reacquire its held reference lock")
        .unwrap();

        assert!(matches!(
            &selected,
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == crate::engine_id()
                    && reference.profile_id.as_str() == "default"
                    && reference.revision.value() == 1
        ));
        let workspace = manager
            .read_workspace(&ProfileId::new("default").unwrap())
            .await
            .unwrap();
        assert_eq!(workspace.document.identity.engine_id, crate::engine_id());
        assert_eq!(workspace.document.identity.profile_id.as_str(), "default");
        assert_eq!(
            manager.engine_profile_store().read().await.unwrap(),
            selected
        );
    }
