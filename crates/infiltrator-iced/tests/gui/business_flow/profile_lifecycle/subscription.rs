use super::*;

/// DUAL-07-02 / 07-04 / 07-12 — the per-profile User-Agent and insecure-TLS
/// fields are real editor state (previously `UpdateSubscriptionUserAgent`
/// was a dead message), the stored values reload from the shared profile
/// metadata, and a `304 Not Modified` update surfaces the honest not-modified
/// toast instead of a fabricated success.
#[test]
fn subscription_fetch_options_load_and_not_modified_feedback() {
    let home = TempHome::acquire("sub-fetch-options");
    home.seed_profile("Paid", LOCAL_IMPORT_YAML);
    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();

    block_on(async {
        let manager = config_manager().await.unwrap();
        let mut metadata = manager.get_profile_metadata("Paid").await.unwrap();
        metadata.subscription_url = Some("https://sub.example.com/token".into());
        metadata.user_agent = Some("ClashVerge/2.0".into());
        metadata.insecure_skip_verify = true;
        metadata.etag = Some("\"abc\"".into());
        metadata.last_modified = Some("Tue, 22 Sep 2026 09:00:00 GMT".into());
        manager
            .update_profile_metadata("Paid", &metadata)
            .await
            .unwrap();
    });

    feed(&mut state, Message::ProfilesLoaded(Ok(list_profiles())));
    feed(
        &mut state,
        Message::SelectSubscriptionProfile("Paid".into()),
    );
    assert_eq!(
        state.profile.subscription_user_agent, "ClashVerge/2.0",
        "stored User-Agent loads into the editor"
    );
    assert!(
        state.profile.subscription_insecure_skip_verify,
        "stored insecure-TLS preference loads into the editor"
    );

    // Both controls mutate real state (not dead dispatches).
    feed(
        &mut state,
        Message::UpdateSubscriptionUserAgent("Custom-UA/9".into()),
    );
    feed(
        &mut state,
        Message::UpdateSubscriptionInsecureSkipVerify(false),
    );
    assert_eq!(state.profile.subscription_user_agent, "Custom-UA/9");
    assert!(!state.profile.subscription_insecure_skip_verify);

    // A 304 update renders the not-modified toast, not a success toast.
    let units = feed(
        &mut state,
        Message::SubscriptionUpdatedNow(Ok(not_modified_report("Paid"))),
    );
    assert!(units >= 2, "LoadProfiles + toast legs");
    let lang = Lang(&state.shell.lang);
    let (text, status) = subscription_update_toast(&lang, &not_modified_report("Paid"));
    assert_eq!(status, ToastStatus::Info);
    assert!(
        text.contains("未变更"),
        "304 surfaces the honest not-modified toast, got {text:?}"
    );
    let (_, success_status) = subscription_update_toast(&lang, &subscription_report("Paid"));
    assert_eq!(success_status, ToastStatus::Success);
}

/// A source that fails the first `failures_before_success` conditional fetches
/// and then reports modified content, counting every attempt.
struct FlakyUpdateSource {
    failures_before_success: usize,
    calls: AtomicUsize,
}

impl FlakyUpdateSource {
    fn new(failures_before_success: usize) -> Self {
        Self {
            failures_before_success,
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl SubscriptionSource for FlakyUpdateSource {
    async fn fetch(
        &self,
        _profile: &str,
        _url: &CheckedSubscriptionUrl,
    ) -> Result<SubscriptionDocument, PortError> {
        Err(PortError::Network("use fetch_conditional".into()))
    }

    async fn fetch_conditional(
        &self,
        _profile: &str,
        _url: &CheckedSubscriptionUrl,
        _headers: &ConditionalFetchHeaders,
    ) -> Result<ConditionalDocumentResult, PortError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call <= self.failures_before_success {
            return Err(PortError::Network(format!("attempt {call} failed")));
        }
        Ok(ConditionalDocumentResult::Modified {
            document: SubscriptionDocument {
                content: "proxies:\n  - name: n1\n    type: ss\n".into(),
                userinfo: None,
            },
            etag: Some("\"refresh-etag\"".into()),
            last_modified: None,
        })
    }
}

/// DUAL-07-05 / 07-06 — the Iced manual update path runs through the shared
/// `SubscriptionRefreshApplication`: it retries a transient failure through the
/// injected runtime seam and rejects a concurrent refresh of the same profile
/// with a typed `InvalidState` instead of double-downloading.
#[test]
fn subscription_refresh_retries_and_single_flights_through_shared_application() {
    let home = TempHome::acquire("sub-refresh-application");
    home.seed_profile("Paid", LOCAL_IMPORT_YAML);

    block_on(async {
        use infiltrator_application::subscription_refresh_application::SubscriptionRefreshApplication;
        use infiltrator_domain::subscription_scheduler_policy::RetryBackoffPolicy;
        use lurker_core_contract::error::ErrorCode;

        let manager = config_manager().await.unwrap();
        let mut metadata = manager.get_profile_metadata("Paid").await.unwrap();
        metadata.subscription_url = Some("https://sub.example.com/token".into());
        manager
            .update_profile_metadata("Paid", &metadata)
            .await
            .unwrap();

        let application = profiles_for_mihomo(manager);
        let refresh = SubscriptionRefreshApplication::new(
            application,
            application_runtime(),
            RetryBackoffPolicy::new(vec![time::Duration::from_millis(1)]),
        );
        let source = FlakyUpdateSource::new(1);

        let report = refresh
            .refresh_profile(&source, "Paid")
            .await
            .expect("retry succeeds");
        assert!(matches!(
            report.outcome,
            SubscriptionUpdateOutcome::Updated { .. }
        ));
        assert_eq!(source.calls(), 2, "one transient failure then a success");

        let guard = refresh.begin_refresh("Paid").expect("claim slot");
        let busy = refresh
            .refresh_profile(&source, "Paid")
            .await
            .expect_err("concurrent refresh is rejected");
        assert_eq!(busy.code, ErrorCode::InvalidState);
        assert_eq!(source.calls(), 2, "no duplicate download while in flight");
        drop(guard);

        refresh
            .refresh_profile(&source, "Paid")
            .await
            .expect("slot released");
        assert_eq!(source.calls(), 3);
    });
}

/// DUAL-07-08: the Iced filter editor state round-trips into the shared
/// `profile_options` pipeline, which reshapes the stored document.
#[test]
fn subscription_filter_panel_rides_the_shared_pipeline() {
    let home = TempHome::acquire("sub-filter-panel");
    home.seed_profile(
        "Paid",
        "proxies:\n  - name: 香港-01\n    type: ss\n  - name: 广告-02\n    type: vmess\n",
    );
    let mut state = fresh_state();

    state.editor.editor_path = Some(home.profile_config_dir().join("Paid.yaml"));
    feed(&mut state, Message::LoadProfileFilter);

    // The view model loads the stored draft; editing replaces it.
    feed(
        &mut state,
        Message::ProfileFilterLoaded {
            token: 1,
            profile: "Paid".into(),
            result: observation(
                "Paid",
                FIXTURE_DOCUMENT,
                SubscriptionFilterDraft {
                    include: "香港".into(),
                    ..Default::default()
                },
            ),
        },
    );
    assert_eq!(state.editor.filter_editor.draft.include, "香港");
    feed(
        &mut state,
        Message::UpdateFilterInclude("香港, 日本".into()),
    );
    feed(&mut state, Message::UpdateFilterExclude("广告".into()));
    assert_eq!(state.editor.filter_editor.draft.include, "香港, 日本");

    // The shared options facade delegates the neutral draft to the scoped
    // Mihomo codec and commits its document/sidecar pair to the real store.
    let draft = state.editor.filter_editor.draft.clone();
    block_on(async {
        let manager = config_manager().await.unwrap();
        let profiles = profiles_for_mihomo(manager.clone());
        let application = ProfileOptionsApplication::new(profiles.clone());
        let runtime = ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline;
        let report = application
            .save_filter(
                runtime,
                &profiles
                    .load_workspace(&ProfileId::new("Paid").unwrap())
                    .await
                    .unwrap()
                    .identity(),
                &draft,
            )
            .await
            .expect("shared filter runs")
            .report;
        assert_eq!(report.total_input, 2);
        assert_eq!(report.passed, 1);

        let stored = manager.load("Paid").await.unwrap();
        assert!(stored.contains("香港-01"));
        assert!(!stored.contains("广告-02"), "excluded node is dropped");

        let options = application.load(Some("Paid")).await.expect("options");
        assert_eq!(options.filter.exclude, "广告");
    });
}

/// DUAL-07-01: the Iced local-import task reads through the shared host port
/// instead of owning a second filesystem path.
#[test]
fn local_import_reads_through_the_host_import_port() {
    let home = TempHome::acquire("sub-import-port");
    let source = home.join("picked.yaml");
    write(&source, LOCAL_IMPORT_YAML).unwrap();

    let port = subscription_import_port();
    let content =
        block_on(port.read_local_file(&source.to_string_lossy())).expect("port reads file");
    assert_eq!(content, LOCAL_IMPORT_YAML);

    let missing = block_on(port.read_local_file("/definitely/not/here.yaml"));
    assert!(missing.is_err(), "a missing local file is an honest error");
}

/// DUAL-07-03: the Iced subscription editor loads and validates a cron
/// expression; a malformed one is rejected before any save task is spawned.
#[test]
fn subscription_cron_editor_loads_and_validates() {
    let home = TempHome::acquire("sub-cron-editor");
    home.seed_profile("Paid", LOCAL_IMPORT_YAML);
    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();

    block_on(async {
        let manager = config_manager().await.unwrap();
        let mut metadata = manager.get_profile_metadata("Paid").await.unwrap();
        metadata.subscription_url = Some("https://sub.example.com/token".into());
        metadata.auto_update_enabled = true;
        metadata.update_interval_hours = Some(24);
        metadata.cron_expression = Some("0 */6 * * *".into());
        manager
            .update_profile_metadata("Paid", &metadata)
            .await
            .unwrap();
    });

    feed(&mut state, Message::ProfilesLoaded(Ok(list_profiles())));
    feed(
        &mut state,
        Message::SelectSubscriptionProfile("Paid".into()),
    );
    assert_eq!(
        state.profile.subscription_cron_expression, "0 */6 * * *",
        "stored cron expression loads into the editor"
    );

    // A malformed expression is rejected before the save spawns.
    feed(
        &mut state,
        Message::UpdateSubscriptionCron("not a cron".into()),
    );
    let units = feed(&mut state, Message::SaveSubscriptionSettings);
    assert!(units >= 1, "rejection raises a toast");
    assert!(
        !state.profile.subscription_save_phase.is_saving(),
        "a malformed cron never starts the save task"
    );

    // A valid expression is accepted and the save task starts.
    feed(
        &mut state,
        Message::UpdateSubscriptionCron("0 */6 * * *".into()),
    );
    feed(&mut state, Message::SaveSubscriptionSettings);
    assert!(state.profile.subscription_save_phase.is_saving());
    feed(&mut state, Message::SubscriptionSettingsSaved(Ok(())));
    assert!(!state.profile.subscription_save_phase.is_saving());
}

/// DUAL-07-09/14: the subscription editor's schedule draft and auto-reload
/// preference are validated and persisted by the shared application, and
/// enabling a reload this host cannot perform is a typed refusal instead of a
/// stored no-op.
#[test]
fn subscription_policy_and_auto_reload_are_shared_application_wired() {
    let home = TempHome::acquire("sub-policy");
    home.seed_profile("Paid", LOCAL_IMPORT_YAML);
    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();

    feed(&mut state, Message::ProfilesLoaded(Ok(list_profiles())));
    feed(
        &mut state,
        Message::SelectSubscriptionProfile("Paid".into()),
    );
    assert!(
        state.profile.subscription_auto_reload_core,
        "the editor loads the store's reload preference"
    );

    // Validation gates run before any task spawns: auto-update needs a URL.
    feed(&mut state, Message::UpdateSubscriptionUrl("  ".into()));
    feed(&mut state, Message::UpdateSubscriptionAutoUpdate(true));
    let units = feed(&mut state, Message::SaveSubscriptionSettings);
    assert_eq!(units, 1, "auto-update without URL → toast only");
    assert!(!state.profile.subscription_save_phase.is_saving());

    // Opt out for real so the editor state is the honest persisted one.
    block_on(async {
        let manager = config_manager().await.unwrap();
        profiles_for_mihomo(manager)
            .update_subscription_auto_reload("Paid", false)
            .await
            .expect("shared application persists the opt-out");
    });
    feed(&mut state, Message::ProfilesLoaded(Ok(list_profiles())));
    feed(
        &mut state,
        Message::SelectSubscriptionProfile("Paid".into()),
    );
    assert!(!state.profile.subscription_auto_reload_core);

    // No runtime seam on this state: re-enabling is refused up front, before
    // any write, instead of silently storing a preference that cannot work.
    feed(
        &mut state,
        Message::UpdateSubscriptionUrl("https://sub.example.com/token".into()),
    );
    feed(&mut state, Message::UpdateSubscriptionAutoReload(true));
    assert!(
        state.runtime.runtime.is_none(),
        "this journey runs without a host runtime seam"
    );
    assert!(
        state
            .profile
            .profiles
            .iter()
            .find(|profile| profile.name == "Paid")
            .is_some_and(|profile| !profile.auto_reload_core),
        "the persisted preference really is off before re-enabling"
    );
    let units = feed(&mut state, Message::SaveSubscriptionSettings);
    assert_eq!(
        units, 1,
        "typed unsupported toast instead of spawning the save task"
    );
    assert!(
        !state.profile.subscription_save_phase.is_saving(),
        "a reload this host cannot perform never starts a save"
    );

    // Disabling never needs the seam: the save task really spawns.
    feed(&mut state, Message::UpdateSubscriptionAutoReload(false));
    let units = feed(&mut state, Message::SaveSubscriptionSettings);
    assert!(state.profile.subscription_save_phase.is_saving());
    assert_eq!(units, 1);
    let units = feed(&mut state, Message::SubscriptionSettingsSaved(Ok(())));
    assert_eq!(units, 2, "LoadProfiles + success-toast legs");

    // The persistence leg for real: the shared schedule + reload methods land
    // in the config.toml store the editor reloads.
    block_on(async {
        let manager = config_manager().await.unwrap();
        let application = profiles_for_mihomo(manager.clone());
        application
            .update_subscription_schedule(
                "Paid",
                &SubscriptionScheduleDraft {
                    url: "https://sub.example.com/token".to_string(),
                    auto_update_enabled: true,
                    update_interval_hours: "12".to_string(),
                    cron_expression: Some("0 */6 * * *".to_string()),
                },
            )
            .await
            .expect("shared schedule persists");
        application
            .update_subscription_auto_reload("Paid", true)
            .await
            .expect("shared reload preference persists");

        let metadata = manager.get_profile_metadata("Paid").await.unwrap();
        assert_eq!(metadata.update_interval_hours, Some(12));
        assert_eq!(metadata.cron_expression.as_deref(), Some("0 */6 * * *"));
        assert!(
            metadata.auto_reload_core,
            "the reload preference round-trips through the store"
        );
    });

    // LoadProfiles回流 reflects every persisted fact in the editor.
    feed(&mut state, Message::ProfilesLoaded(Ok(list_profiles())));
    feed(
        &mut state,
        Message::SelectSubscriptionProfile("Paid".into()),
    );
    assert_eq!(state.profile.subscription_update_interval_hours, "12");
    assert_eq!(state.profile.subscription_cron_expression, "0 */6 * * *");
    assert!(
        state.profile.subscription_auto_reload_core,
        "the reload preference loads back from the store"
    );
}

/// The update toast distinguishes a completed workspace apply from stored,
/// unsupported, or failed content effects.
#[test]
fn subscription_toast_reports_the_content_effect_honestly() {
    let lang = Lang("zh-CN");
    let mut report = subscription_report("Paid");
    report.content_effect = SubscriptionContentEffectOutcome::Applied;
    let (_, applied_status) = subscription_update_toast(&lang, &report);
    assert_eq!(applied_status, ToastStatus::Success);

    report.content_effect = SubscriptionContentEffectOutcome::Unsupported;
    let (text, unsupported_status) = subscription_update_toast(&lang, &report);
    assert_eq!(unsupported_status, ToastStatus::Warning);
    assert!(
        text.contains("宿主") && text.contains("未更改"),
        "the unsupported effect names the host gap, got {text:?}"
    );

    report.content_effect = SubscriptionContentEffectOutcome::Failed {
        error: "apply rejected".to_string(),
    };
    let (text, failed_status) = subscription_update_toast(&lang, &report);
    assert_eq!(failed_status, ToastStatus::Warning);
    assert!(text.contains("apply rejected"), "got {text:?}");
}
