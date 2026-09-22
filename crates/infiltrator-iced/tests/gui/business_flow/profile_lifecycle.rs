//! Journeys 1/2/3/16 — the subscription-to-kernel profile lifecycle:
//! import (bad URL fast-fail + real local import), activation, the restart
//! chain, subscription settings + auto-update persistence, manual update
//! outcomes and profile deletion with sidecar cleanup — all against a real
//! temp-HOME config store.
//!
//! test-intent: behavior

use super::support::{TempHome, block_on, feed, fresh_state, list_profiles, subscribed_profile};
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::subscription_import::{
    SubscriptionBatchReport, SubscriptionUpdateOutcome, SubscriptionUpdateReport,
};
use infiltrator_core::subscription_io::HttpSubscriptionSource;
use infiltrator_domain::profiles::sanitize_profile_name;
use std::path::PathBuf;

const LOCAL_IMPORT_YAML: &str = "mixed-port: 7890\nmode: rule\n";

fn subscription_report(profile: &str) -> SubscriptionUpdateReport {
    SubscriptionUpdateReport {
        profile_name: profile.to_string(),
        outcome: SubscriptionUpdateOutcome::Updated {
            new_bytes: 1,
            node_count: 1,
        },
        etag: None,
        last_modified: None,
        quota: None,
        usage_warning: false,
        expiry_warning: false,
        reloaded_core: false,
        backed_up: true,
    }
}

fn not_modified_report(profile: &str) -> SubscriptionUpdateReport {
    SubscriptionUpdateReport {
        profile_name: profile.to_string(),
        outcome: SubscriptionUpdateOutcome::NotModified {
            etag: Some("\"abc\"".to_string()),
            last_modified: Some("Tue, 22 Sep 2026 09:00:00 GMT".to_string()),
        },
        etag: Some("\"abc\"".to_string()),
        last_modified: Some("Tue, 22 Sep 2026 09:00:00 GMT".to_string()),
        quota: None,
        usage_warning: false,
        expiry_warning: false,
        reloaded_core: false,
        backed_up: false,
    }
}

/// Journey 1 — 导入订阅 → 立即更新失败回灌 → 本地导入 → 激活 → 重启内核链。
///
/// Everything file-backed runs the real config-manager code inside a temp
/// HOME; the only remote leg (subscription import) fails fast on a closed
/// localhost port, which is exactly the failure path a dead provider takes.
#[test]
fn import_then_activate_then_restart_kernel_chain_round_trips_real_files() {
    let home = TempHome::acquire("import-activate");
    home.seed_profile("default", "mixed-port: 7890\nmode: rule\n");
    let mut state = fresh_state();

    // Startup回流: the bootstrap task would fetch the profile list.
    let listed = list_profiles();
    let units = feed(&mut state, Message::ProfilesLoaded(Ok(listed)));
    assert_eq!(units, 0);
    assert_eq!(state.profile.profiles.len(), 1);
    assert_eq!(state.profile.profiles[0].name, "default");
    assert!(state.profile.profiles[0].active, "current pointer honoured");

    // ---- subscription import: guard + real fast-failing remote leg ----
    feed(
        &mut state,
        Message::UpdateImportUrl("http://127.0.0.1:1/sub".into()),
    );
    feed(&mut state, Message::UpdateImportName("Bad Sub".into()));
    feed(&mut state, Message::UpdateImportActivate(true));
    let units = feed(&mut state, Message::ImportProfile);
    assert!(state.profile.is_importing, "import is in flight");
    assert!(units >= 1, "import task spawned");

    // The import task body, run for real: sanitize → application subscription
    // use-case on a closed port fails without any network egress.
    let name = sanitize_profile_name("Bad Sub").unwrap();
    let remote = block_on(async {
        let store = crate::configs_dir::config_manager().await.unwrap();
        let application = ProfileApplication::new(store);
        let source = HttpSubscriptionSource::with_default_clients();
        application
            .import_subscription(&source, &name, "http://127.0.0.1:1/sub")
            .await
    });
    assert!(remote.is_err(), "closed port must fail fast");

    let units = feed(
        &mut state,
        Message::ProfileImported(Err(InfiltratorError::Config("connection refused".into()))),
    );
    assert!(!state.profile.is_importing);
    assert!(state.shell.error_msg.is_some(), "failure hits the banner");
    // The error toast rides the single `Task::done(ShowToast)` unit asserted
    // below — it lands one runtime hop later, like every toast in this app.
    assert!(
        !state.profile.import_url.is_empty(),
        "failed import keeps the form for retry"
    );
    assert_eq!(units, 1, "failure path only re-raises the toast");

    // ---- local import: real read → validate → save through the manager ----
    let source = home.join("downloaded-source.yaml");
    std::fs::write(&source, LOCAL_IMPORT_YAML).unwrap();
    feed(
        &mut state,
        Message::UpdateLocalImportPath(source.to_string_lossy().into()),
    );
    feed(
        &mut state,
        Message::UpdateLocalImportName("Travel Node".into()),
    );
    feed(&mut state, Message::UpdateLocalImportActivate(true));
    let _task = feed(&mut state, Message::ImportLocalProfile);
    assert!(state.profile.is_importing_local);

    // Task body for real: read_to_string → validate_yaml → save → activate.
    let stored = block_on(async {
        let content = tokio::fs::read_to_string(&source).await.unwrap();
        infiltrator_domain::config::validate_yaml(&content).unwrap();
        let manager = crate::configs_dir::config_manager().await.unwrap();
        manager.save("Travel Node", &content).await.unwrap();
        crate::update::core::profile_apply::activate_profile(None, "Travel Node")
            .await
            .unwrap()
    });
    assert!(!stored, "no runtime → activation reports not-reloaded");

    let units = feed(&mut state, Message::LocalProfileImported(Ok(false)));
    assert!(!state.profile.is_importing_local);
    assert!(state.profile.local_import_path.is_empty(), "form cleared");
    // activate && !reloaded → the handler chains StartProxy for the restart.
    assert!(units >= 3, "LoadProfiles + toast + StartProxy chained");

    // ---- explicit activation of another profile ----
    feed(&mut state, Message::ProfilesLoaded(Ok(list_profiles())));
    state.shell.error_msg = Some("stale".into());
    let units = feed(&mut state, Message::SetActiveProfile("default".into()));
    assert!(
        state.shell.error_msg.is_none(),
        "activation clears the banner"
    );
    assert_eq!(units, 1);

    let was_running = block_on(async {
        crate::update::core::profile_apply::activate_profile(None, "default").await
    })
    .unwrap();
    assert!(!was_running);
    let units = feed(&mut state, Message::ProfileActivationFinished(Ok(false)));
    assert!(units >= 2, "activation without runtime chains StartProxy");

    // The current pointer really moved on disk.
    let current = block_on(async {
        crate::configs_dir::config_manager()
            .await
            .unwrap()
            .get_current()
            .await
            .unwrap()
    });
    assert_eq!(current, "default");

    // ---- restart-kernel chain: StartProxy → ProxyStarted(Err) 回灌 ----
    let token_before = state.runtime.lifecycle_token;
    let units = feed(&mut state, Message::StartProxy);
    assert_eq!(state.runtime.status, RuntimeStatus::Starting);
    assert_eq!(state.runtime.lifecycle_token, token_before + 1);
    assert!(units >= 1, "boot task spawned (no kernel binary here)");

    let units = feed(
        &mut state,
        Message::ProxyStarted(
            Err(InfiltratorError::Mihomo("启动失败".into())),
            token_before + 1,
        ),
    );
    assert!(matches!(state.runtime.status, RuntimeStatus::Error(_)));
    assert!(
        state.shell.error_msg.is_some(),
        "boot failure hits the banner"
    );
    assert!(
        units >= 1,
        "critical system-notification task armed (lazy, never delivered here)"
    );

    // A late arrival from a previous lifecycle is dropped on the floor.
    let units = feed(
        &mut state,
        Message::ProxyStarted(Err(InfiltratorError::Mihomo("late".into())), token_before),
    );
    assert_eq!(units, 0, "stale token ignored");
    assert!(matches!(state.runtime.status, RuntimeStatus::Error(_)));
}

/// Journey 2 — 订阅设置保存 → 校验门 → auto_update 持久化（托盘入口语义）→
/// LoadProfiles 回流。URL-keyed metadata goes through the OS keyring in
/// production, so the persistence leg here exercises the URL-less metadata
/// write (same config.toml store, no secret service touch).
#[test]
fn subscription_settings_save_gates_persists_and_reloads_profiles() {
    let home = TempHome::acquire("subscription-settings");
    home.seed_profile("Paid", LOCAL_IMPORT_YAML);
    let mut state = fresh_state();

    state.shell.lang = "zh-CN".into();
    feed(
        &mut state,
        Message::ProfilesLoaded(Ok(vec![subscribed_profile(
            "Paid",
            true,
            Some("https://sub.example.com/token"),
        )])),
    );
    // Editor auto-synced from the loaded catalog.
    assert_eq!(state.profile.subscription_profile_name, "Paid");
    assert_eq!(
        state.profile.subscription_url,
        "https://sub.example.com/token"
    );
    assert_eq!(state.profile.subscription_update_interval_hours, "24");

    // Validation gates run synchronously, before any task spawns.
    feed(&mut state, Message::UpdateSubscriptionAutoUpdate(true));
    feed(&mut state, Message::UpdateSubscriptionInterval("0".into()));
    let units = feed(&mut state, Message::SaveSubscriptionSettings);
    assert_eq!(units, 1, "interval gate arms only the error toast");
    assert!(!state.profile.is_saving_subscription);

    feed(&mut state, Message::UpdateSubscriptionUrl("  ".into()));
    let units = feed(&mut state, Message::SaveSubscriptionSettings);
    assert_eq!(units, 1, "auto-update without URL → toast only");
    assert!(!state.profile.is_saving_subscription);

    // Happy path: gates pass, the persistence task spawns (not run here —
    // its URL leg would touch the OS keyring), and the result 回灌 reloads.
    feed(
        &mut state,
        Message::UpdateSubscriptionUrl("https://sub.example.com/token".into()),
    );
    feed(&mut state, Message::UpdateSubscriptionInterval("12".into()));
    let units = feed(&mut state, Message::SaveSubscriptionSettings);
    assert!(state.profile.is_saving_subscription);
    assert_eq!(units, 1);
    let units = feed(&mut state, Message::SubscriptionSettingsSaved(Ok(())));
    assert!(!state.profile.is_saving_subscription);
    assert_eq!(units, 2, "LoadProfiles + success-toast legs");

    // Persistence leg for real: the URL-less metadata write path (what the
    // tray auto-update toggle persists) lands in config.toml and round-trips
    // through the manager that LoadProfiles would read.
    block_on(async {
        let manager = crate::configs_dir::config_manager().await.unwrap();
        let mut meta = manager.get_profile_metadata("Paid").await.unwrap();
        meta.auto_update_enabled = false;
        meta.subscription_url = None;
        manager
            .update_profile_metadata("Paid", &meta)
            .await
            .unwrap();

        let reread = manager.get_profile_metadata("Paid").await.unwrap();
        assert!(!reread.auto_update_enabled, "auto-update flag persisted");
        assert!(reread.subscription_url.is_none());
    });
    let listed = list_profiles();
    feed(&mut state, Message::ProfilesLoaded(Ok(listed)));
    let paid = state
        .profile
        .profiles
        .iter()
        .find(|p| p.name == "Paid")
        .unwrap();
    assert!(
        !paid.auto_update_enabled,
        "LoadProfiles回流 reflects the store"
    );
    assert!(
        home.join("config.toml").exists(),
        "metadata store is the temp home"
    );
}

/// Journey 3 — 立即更新（手动版）: no selection → 「请选择」toast;
/// success/failure results 回灌 with counting toasts. The tray's bulk
/// "update all" entry is separately proven live in
/// [`super::profile_lifecycle::tray_bulk_entry_messages_reach_their_handlers`].
#[test]
fn manual_subscription_update_reports_zero_selection_and_outcomes() {
    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();

    // 0 订阅 → immediate toast, nothing in flight.
    let units = feed(&mut state, Message::UpdateSubscriptionNow);
    assert_eq!(
        units, 1,
        "guard toast armed (delivered one runtime hop later)"
    );
    assert!(!state.profile.is_updating_subscription_now);

    feed(
        &mut state,
        Message::ProfilesLoaded(Ok(vec![subscribed_profile(
            "Paid",
            true,
            Some("https://x"),
        )])),
    );
    feed(
        &mut state,
        Message::SelectSubscriptionProfile("Paid".into()),
    );
    assert_eq!(state.profile.subscription_profile_name, "Paid");

    let units = feed(&mut state, Message::UpdateSubscriptionNow);
    assert!(state.profile.is_updating_subscription_now);
    assert!(units >= 1, "update task spawned");

    // Worker would fetch → parse → (active profile, no runtime) clear backup.
    let units = feed(
        &mut state,
        Message::SubscriptionUpdatedNow(Ok(subscription_report("Paid"))),
    );
    assert!(!state.profile.is_updating_subscription_now);
    assert_eq!(units, 2, "LoadProfiles + success-toast legs");

    // Failure leg: banner + error toast, flag cleared.
    feed(&mut state, Message::UpdateSubscriptionNow);
    let units = feed(
        &mut state,
        Message::SubscriptionUpdatedNow(Err(InfiltratorError::Config("订阅拉取失败".into()))),
    );
    assert!(!state.profile.is_updating_subscription_now);
    assert!(
        state
            .shell
            .error_msg
            .as_deref()
            .unwrap_or("")
            .contains("订阅拉取失败")
    );
    assert_eq!(units, 1, "error-toast leg armed");
}

/// DUAL-07-11/13 — the toolbar batch-update entry and the safe-backup restore
/// both flow through the shared application seam: batch re-entry is guarded,
/// the summarized report drives the toast, and a missing backup is reported
/// honestly instead of fabricating a restore.
#[test]
fn subscription_batch_update_and_backup_restore_are_shared_application_wired() {
    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();
    state.profile.profiles = vec![subscribed_profile("Paid", true, Some("https://x"))];
    state.profile.subscription_profile_name = "Paid".into();

    // Toolbar click arms the shared batch path.
    let units = feed(&mut state, Message::UpdateAllSubscriptionsNow);
    assert!(units >= 1, "batch update task spawned");
    assert!(state.profile.is_updating_subscription_now);
    // Single-flight guard: a second click while in flight is a no-op.
    assert_eq!(
        feed(&mut state, Message::UpdateAllSubscriptionsNow),
        0,
        "re-entry is suppressed while the batch is in flight"
    );

    let report = SubscriptionBatchReport {
        total: 1,
        updated: 1,
        skipped: 0,
        outcomes: vec![subscription_report("Paid")],
        ..Default::default()
    };
    let units = feed(&mut state, Message::AllSubscriptionsUpdated(Ok(report)));
    assert_eq!(units, 2, "LoadProfiles + summarized batch toast legs");
    assert!(!state.profile.is_updating_subscription_now);

    // Missing backup is reported honestly (informational), not as a success.
    let units = feed(&mut state, Message::RestoreSubscriptionBackup);
    assert!(units >= 1, "restore backup task spawned");
    let units = feed(&mut state, Message::SubscriptionBackupRestored(Ok(false)));
    assert_eq!(units, 2, "LoadProfiles + informational toast legs");

    // A real restore reports success.
    feed(&mut state, Message::RestoreSubscriptionBackup);
    let units = feed(&mut state, Message::SubscriptionBackupRestored(Ok(true)));
    assert_eq!(units, 2, "LoadProfiles + restored toast legs");
}

/// Journey 16 — 删除 profile：真实删除 yaml + options sidecar 一并清理。
#[test]
fn delete_profile_removes_yaml_and_options_sidecar_from_disk() {
    let home = TempHome::acquire("delete-profile");
    home.seed_profile("Doomed", LOCAL_IMPORT_YAML);
    home.seed_profile("Keeper", LOCAL_IMPORT_YAML);

    // A stored, non-empty options sidecar exists for the doomed profile
    // (empty options would be treated as "no sidecar" by save_options).
    block_on(infiltrator_core::profile_options_io::save_options(
        &home.configs(),
        "Doomed",
        &infiltrator_domain::profile_options::ProfileOptions {
            mixin: Default::default(),
            filter: Some(infiltrator_domain::profile_options::FilterSpec {
                include_keywords: vec!["HK".into()],
                ..Default::default()
            }),
        },
    ))
    .unwrap();
    assert!(home.configs().join("options/Doomed.yaml").exists());

    let mut state = fresh_state();
    let listed = list_profiles();
    assert_eq!(listed.len(), 2);
    feed(&mut state, Message::ProfilesLoaded(Ok(listed)));

    let units = feed(&mut state, Message::DeleteProfile("Doomed".into()));
    assert_eq!(units, 1);

    // Task body for real: delete + best-effort sidecar cleanup.
    block_on(async {
        let manager = crate::configs_dir::config_manager().await.unwrap();
        manager.delete_profile("Doomed").await.unwrap();
        let dir = crate::configs_dir::configs_dir().await.unwrap();
        infiltrator_core::profile_options_io::delete_options(&dir, "Doomed").await;
    });

    let units = feed(&mut state, Message::ProfileDeleted(Ok(())));
    assert_eq!(units, 2, "LoadProfiles + toast legs");
    assert!(!home.configs().join("Doomed.yaml").exists());
    assert!(!home.configs().join("options/Doomed.yaml").exists());
    assert!(
        home.configs().join("Keeper.yaml").exists(),
        "sibling untouched"
    );
}

/// Documented product defect (router gap): the tray bulk entries and the
/// one-click editor entries produce messages that NO router arm dispatches,
/// so their fully-written handlers are unreachable through
/// `AppState::update()`. This test is a tripwire: when the routers learn
/// these messages, flip it into positive handler assertions.
#[test]
fn tray_bulk_entry_messages_reach_their_handlers() {
    let mut state = fresh_state();
    state.profile.profiles = vec![subscribed_profile("Paid", true, Some("https://x"))];

    // Message produced by TrayIntent::UpdateAllProfilesNow (tray submenu):
    let units = feed(&mut state, Message::UpdateAllSubscriptionsNow);
    assert!(units >= 1, "update-all task armed through the router");
    assert!(state.profile.is_updating_subscription_now, "handler armed");

    // Its worker result reaches the summarizing handler. The shared batch
    // report is what the toast leg consumes.
    let units = feed(
        &mut state,
        Message::AllSubscriptionsUpdated(Ok(SubscriptionBatchReport {
            total: 1,
            updated: 1,
            ..Default::default()
        })),
    );
    assert!(units >= 1, "AllSubscriptionsUpdated dispatched");
    assert!(!state.profile.is_updating_subscription_now);
    assert_eq!(
        units, 2,
        "LoadProfiles + summarized batch toast legs are armed"
    );

    // Message produced by TrayIntent::SetProfileAutoUpdate (per-profile
    // checkmark): the click persists through the real metadata path.
    let units = feed(
        &mut state,
        Message::SetProfileAutoUpdate {
            name: "Paid".into(),
            enabled: false,
        },
    );
    assert!(units >= 1, "SetProfileAutoUpdate dispatched");
    let units = feed(&mut state, Message::ProfileAutoUpdateSet(Ok("Paid".into())));
    assert!(units >= 1, "ProfileAutoUpdateSet dispatched");

    // Message produced by the Profiles page one-click 覆写/过滤 buttons
    // (view/profiles.rs): opens the editor with the pane preselected.
    let units = feed(
        &mut state,
        Message::EditProfileAs(
            PathBuf::from("/configs/Paid.yaml"),
            crate::types::options::EditorPane::Mixin,
        ),
    );
    assert!(units >= 1, "EditProfileAs dispatched");
    // The path binds asynchronously (ProfileContentLoaded); the pane is
    // what's selected synchronously.
    assert!(state.editor.editor_path.is_none(), "content not loaded yet");
    assert_eq!(
        state.editor.editor_pane,
        crate::types::options::EditorPane::Mixin,
        "pane preselected"
    );
}

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
        let manager = crate::configs_dir::config_manager().await.unwrap();
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
    let lang = infiltrator_shared::locales::Lang(&state.shell.lang);
    let (text, status) = crate::update::profile::subscription::subscription_update_toast(
        &lang,
        &not_modified_report("Paid"),
    );
    assert_eq!(status, crate::types::app::ToastStatus::Info);
    assert!(
        text.contains("未变更"),
        "304 surfaces the honest not-modified toast, got {text:?}"
    );
    let (_, success_status) = crate::update::profile::subscription::subscription_update_toast(
        &lang,
        &subscription_report("Paid"),
    );
    assert_eq!(success_status, crate::types::app::ToastStatus::Success);
}

/// A source that fails the first `failures_before_success` conditional fetches
/// and then reports modified content, counting every attempt.
struct FlakyUpdateSource {
    failures_before_success: usize,
    calls: std::sync::atomic::AtomicUsize,
}

impl FlakyUpdateSource {
    fn new(failures_before_success: usize) -> Self {
        Self {
            failures_before_success,
            calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl infiltrator_ports::subscription_source::SubscriptionSource for FlakyUpdateSource {
    async fn fetch(
        &self,
        _profile: &str,
        _url: &infiltrator_domain::subscription::CheckedSubscriptionUrl,
    ) -> Result<
        infiltrator_ports::subscription_source::SubscriptionDocument,
        infiltrator_ports::error::PortError,
    > {
        Err(infiltrator_ports::error::PortError::Network(
            "use fetch_conditional".into(),
        ))
    }

    async fn fetch_conditional(
        &self,
        _profile: &str,
        _url: &infiltrator_domain::subscription::CheckedSubscriptionUrl,
        _headers: &infiltrator_ports::subscription_source::ConditionalFetchHeaders,
    ) -> Result<
        infiltrator_ports::subscription_source::ConditionalDocumentResult,
        infiltrator_ports::error::PortError,
    > {
        let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        if call <= self.failures_before_success {
            return Err(infiltrator_ports::error::PortError::Network(format!(
                "attempt {call} failed"
            )));
        }
        Ok(
            infiltrator_ports::subscription_source::ConditionalDocumentResult::Modified {
                document: infiltrator_ports::subscription_source::SubscriptionDocument {
                    content: "proxies:\n  - name: n1\n    type: ss\n".into(),
                    userinfo: None,
                },
                etag: Some("\"refresh-etag\"".into()),
                last_modified: None,
            },
        )
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
        use infiltrator_contract::error::ErrorCode;
        use infiltrator_domain::subscription_scheduler_policy::RetryBackoffPolicy;

        let manager = crate::configs_dir::config_manager().await.unwrap();
        let mut metadata = manager.get_profile_metadata("Paid").await.unwrap();
        metadata.subscription_url = Some("https://sub.example.com/token".into());
        manager
            .update_profile_metadata("Paid", &metadata)
            .await
            .unwrap();

        let application = ProfileApplication::new(manager);
        let refresh = SubscriptionRefreshApplication::new(
            application,
            crate::host::runtime::application_runtime(),
            RetryBackoffPolicy::new(vec![std::time::Duration::from_millis(1)]),
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

    // The view model loads the stored draft; editing replaces it.
    feed(
        &mut state,
        Message::ProfileFilterLoaded(Ok(crate::types::options::FilterDraft {
            include: "香港".into(),
            ..Default::default()
        })),
    );
    assert_eq!(state.editor.filter_draft.include, "香港");
    feed(
        &mut state,
        Message::UpdateFilterInclude("香港, 日本".into()),
    );
    feed(&mut state, Message::UpdateFilterExclude("广告".into()));
    assert_eq!(state.editor.filter_draft.include, "香港, 日本");

    // The editor's draft compiles into the shared spec and runs the shared
    // pipeline over the real config-manager store.
    let spec = state.editor.filter_draft.to_spec().expect("draft compiles");
    block_on(async {
        let manager = crate::configs_dir::config_manager().await.unwrap();
        let application = ProfileApplication::new(manager.clone());
        let runtime: Option<
            std::sync::Arc<dyn infiltrator_ports::runtime_gateway::ManagedRuntime>,
        > = None;
        let report = application
            .apply_subscription_filter(runtime, "Paid", spec)
            .await
            .expect("shared filter runs");
        assert_eq!(report.total_input, 2);
        assert_eq!(report.passed, 1);

        let stored = manager.load("Paid").await.unwrap();
        assert!(stored.contains("香港-01"));
        assert!(!stored.contains("广告-02"), "excluded node is dropped");

        let options = application.load_options("Paid").await.expect("options");
        assert_eq!(
            options.filter.expect("filter persisted").exclude_keywords,
            vec!["广告".to_string()]
        );
    });
}

/// DUAL-07-01: the Iced local-import task reads through the shared host port
/// instead of owning a second filesystem path.
#[test]
fn local_import_reads_through_the_host_import_port() {
    let home = TempHome::acquire("sub-import-port");
    let source = home.join("picked.yaml");
    std::fs::write(&source, LOCAL_IMPORT_YAML).unwrap();

    let port = crate::host::storage::subscription_import_port();
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
        let manager = crate::configs_dir::config_manager().await.unwrap();
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
        !state.profile.is_saving_subscription,
        "a malformed cron never starts the save task"
    );

    // A valid expression is accepted and the save task starts.
    feed(
        &mut state,
        Message::UpdateSubscriptionCron("0 */6 * * *".into()),
    );
    feed(&mut state, Message::SaveSubscriptionSettings);
    assert!(state.profile.is_saving_subscription);
    feed(&mut state, Message::SubscriptionSettingsSaved(Ok(())));
    assert!(!state.profile.is_saving_subscription);
}
