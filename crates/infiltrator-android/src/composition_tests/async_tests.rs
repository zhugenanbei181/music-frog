use super::*;
use crate::composition::core_application_with_readiness_and_provider;

#[tokio::test]
async fn unavailable_controller_keeps_core_lifecycle_and_local_profile_reads_available() {
    let (_home, store) = config_manager();
    store
        .save_inactive_profile("main", INITIAL_DOCUMENT)
        .await
        .unwrap();
    let owner = core_application_with_readiness_and_provider(
        TestBridge::new(),
        "invalid-controller-endpoint".to_owned(),
        None,
        AndroidCoreComposition::new(Arc::clone(&store)),
        Arc::new(TestReadiness),
        Arc::new(TestLifecycleCapabilityProvider),
    )
    .expect("unavailable controller operations do not block Core owner composition");

    assert!(matches!(
        run_scoped(&owner, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    let loaded = produced(
        run_scoped(
            &owner,
            CommandIntent::LoadProfileDocument {
                profile: Some("main".to_owned()),
            },
        )
        .await,
    );
    let document = match loaded {
        CommandOutput::ProfileDocumentLoaded(document) => *document,
        other => panic!("local profile loading remains available: {other:?}"),
    };
    assert_eq!(document.content, INITIAL_DOCUMENT);
}

#[tokio::test]
async fn core_start_sends_the_active_profile_identity_and_rust_resolved_arguments() {
    let (_home, store) = config_manager();
    store
        .save_inactive_profile(
            "main",
            "mode: rule\nexternal-controller: 127.0.0.1:9090\nsecret: test-secret\n",
        )
        .await
        .unwrap();
    select_profile(store.as_ref(), "main").await;
    let bridge = TestBridge::new();
    let owner = core_application_with_readiness_and_provider(
        bridge.clone(),
        "invalid-controller-endpoint".to_owned(),
        None,
        AndroidCoreComposition::new(Arc::clone(&store)),
        Arc::new(TestReadiness),
        Arc::new(TestCapabilityProvider),
    )
    .expect("Android owner runtime is constructible");

    assert!(matches!(
        run_scoped(&owner, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));

    let spec = bridge
        .launch_specs
        .lock()
        .expect("captured launch specs")
        .first()
        .cloned()
        .expect("Rust owner must submit one launch spec");
    let start_context = bridge
        .start_contexts
        .lock()
        .expect("captured Core start contexts")
        .first()
        .cloned()
        .expect("Rust owner must submit one typed Core start context");
    let CoreProcessStartContext::Managed {
        identity,
        generation,
        session_token,
    } = start_context
    else {
        panic!("Android Core launch must carry a managed owner context");
    };
    assert!(identity.same_instance(&owner.core_identity().expect("active Core identity")));
    assert_eq!(generation, CoreGeneration::new(owner.generation()),);
    assert_eq!(
        session_token,
        owner.session_token().expect("active session token")
    );
    let workspace = ProfileStore::load_active_workspace(store.engine_profile_store())
        .await
        .unwrap();
    let config_path = store.existing_profile_document_path("main").await.unwrap();
    let workdir = PathBuf::from("/android-test-data");
    assert_eq!(spec.configuration_identity().engine_id.as_str(), "mihomo");
    assert_eq!(spec.configuration_identity().profile_id.as_str(), "main");
    assert_eq!(
        spec.configuration_identity().document_revision,
        workspace.document.identity.revision
    );
    assert_eq!(spec.working_directory().as_path(), workdir);
    assert_eq!(
        spec.launch_arguments().configuration_path().as_str(),
        config_path.to_string_lossy()
    );
}

#[tokio::test]
async fn local_profile_load_ignores_runtime_version_but_apply_requires_exact_evidence() {
    assert_local_load_succeeds_and_runtime_apply_fails_closed(Arc::new(NoVersionReadiness)).await;
    assert_local_load_succeeds_and_runtime_apply_fails_closed(Arc::new(TestReadiness)).await;
}

#[tokio::test]
async fn owner_profile_commands_use_the_shared_store_and_codec_fail_closed() {
    let (_home, store) = config_manager();
    store
        .save_inactive_profile("main", INITIAL_DOCUMENT)
        .await
        .unwrap();
    let composition = AndroidCoreComposition::new(Arc::clone(&store));
    let owner = core_application_with_readiness_and_provider(
        TestBridge::new(),
        "invalid-controller-endpoint".to_owned(),
        None,
        composition,
        Arc::new(TestReadiness),
        Arc::new(TestCapabilityProvider),
    )
    .expect("Android owner runtime is constructible");

    assert!(matches!(
        run_scoped(&owner, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));

    let loaded = produced(
        run_scoped(
            &owner,
            CommandIntent::LoadProfileDocument {
                profile: Some("main".to_owned()),
            },
        )
        .await,
    );
    let document = match loaded {
        CommandOutput::ProfileDocumentLoaded(document) => *document,
        other => panic!("owner must return the composed profile document: {other:?}"),
    };
    assert_eq!(document.content, INITIAL_DOCUMENT);
    assert_eq!(
        document.workspace_identity.document.engine_id.as_str(),
        "mihomo"
    );
    assert_eq!(
        document.workspace_identity.document.format_id.as_str(),
        "mihomo-yaml"
    );

    let mut foreign_engine = document.workspace_identity.clone();
    foreign_engine.document.engine_id = EngineId::new("sing-box").unwrap();
    rejected(
        run_scoped(
            &owner,
            CommandIntent::SaveProfileDocument {
                workspace_identity: foreign_engine,
                content: SAVED_DOCUMENT.to_owned(),
                authorization: ProfileWriteAuthorization::RequireUnlock,
            },
        )
        .await,
        ErrorCode::Unsupported,
    );
    assert_eq!(store.load("main").await.unwrap(), INITIAL_DOCUMENT);

    let mut foreign_format = document.workspace_identity.clone();
    foreign_format.document.format_id = ProfileFormatId::new("sing-box-json").unwrap();
    rejected(
        run_scoped(
            &owner,
            CommandIntent::SaveProfileDocument {
                workspace_identity: foreign_format,
                content: SAVED_DOCUMENT.to_owned(),
                authorization: ProfileWriteAuthorization::RequireUnlock,
            },
        )
        .await,
        ErrorCode::Unsupported,
    );
    assert_eq!(store.load("main").await.unwrap(), INITIAL_DOCUMENT);

    rejected(
        run_scoped(
            &owner,
            CommandIntent::SaveProfileDocument {
                workspace_identity: document.workspace_identity.clone(),
                content: "rules: [\n".to_owned(),
                authorization: ProfileWriteAuthorization::RequireUnlock,
            },
        )
        .await,
        ErrorCode::Configuration,
    );
    assert_eq!(store.load("main").await.unwrap(), INITIAL_DOCUMENT);

    let saved = produced(
        run_scoped(
            &owner,
            CommandIntent::SaveProfileDocument {
                workspace_identity: document.workspace_identity,
                content: SAVED_DOCUMENT.to_owned(),
                authorization: ProfileWriteAuthorization::RequireUnlock,
            },
        )
        .await,
    );
    assert!(matches!(saved, CommandOutput::ProfileDocumentSaved(_)));
    assert_eq!(
        store.load("main").await.unwrap(),
        SAVED_DOCUMENT,
        "the exact ConfigManager supplied to the owner composition receives the write"
    );
}

#[tokio::test]
async fn stopped_owner_rejects_outbound_inventory_and_selection_before_controller_io() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("local controller mock binds");
    let address = listener.local_addr().expect("local controller address");
    let controller = format!("http://{address}");
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        while let Ok(Ok((mut stream, _))) =
            timeout(Duration::from_millis(250), listener.accept()).await
        {
            let mut request = [0; 2048];
            let read = stream
                .read(&mut request)
                .await
                .expect("controller request reads");
            let line = String::from_utf8_lossy(&request[..read])
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned();
            requests.push(line);
            let body = r#"{"proxies":{}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("controller response writes");
        }
        requests
    });
    let (_home, store) = config_manager();
    let owner = core_application_with_readiness_and_provider(
        TestBridge::new(),
        controller,
        None,
        AndroidCoreComposition::new(store),
        Arc::new(TestReadiness),
        Arc::new(TestCapabilityProvider),
    )
    .expect("Android owner and its outbound adapter compose");
    let owner_outbound = owner
        .outbound_port()
        .expect("Android owner registers one outbound adapter");
    assert!(Arc::ptr_eq(
        &owner_outbound,
        &owner
            .outbound_port()
            .expect("same owner port remains available")
    ));

    // The stopped owner has no active version, so the existing capability
    // gate denies both commands before either reaches its HTTP adapter.
    rejected(
        run_scoped(&owner, CommandIntent::ReadOutboundInventory).await,
        ErrorCode::Unsupported,
    );
    let engine = EngineId::new("mihomo").expect("Mihomo engine ID");
    let request = OutboundSelectionRequest::new(
        OutboundId::new(engine.clone(), "selector").expect("selector ID"),
        OutboundId::new(engine, "proxy-a").expect("outbound ID"),
        SnapshotRevision::new(1),
    )
    .expect("valid typed selection request");
    rejected(
        run_scoped(&owner, CommandIntent::SelectOutbound { request }).await,
        ErrorCode::Unsupported,
    );

    let requests = server.await.expect("controller mock finishes");
    assert!(
        requests.is_empty(),
        "stopped owner must reject both use cases before GET or PUT"
    );
}
