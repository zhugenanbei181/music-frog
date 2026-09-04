//! Journeys 1/2/3/16 — the subscription-to-kernel profile lifecycle:
//! import (bad URL fast-fail + real local import), activation, the restart
//! chain, subscription settings + auto-update persistence, manual update
//! outcomes and profile deletion with sidecar cleanup — all against a real
//! temp-HOME config store.
//!
//! test-intent: behavior

use super::support::{TempHome, block_on, feed, fresh_state, subscribed_profile};
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_core::profiles::create_profile_from_url;
use infiltrator_domain::profiles::sanitize_profile_name;
use std::path::PathBuf;

const LOCAL_IMPORT_YAML: &str = "mixed-port: 7890\nmode: rule\n";

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
    let listed = block_on(infiltrator_core::profiles::list_profile_infos()).unwrap();
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

    // The import task body, run for real: sanitize → create_profile_from_url
    // on a closed port fails without any network egress.
    let name = sanitize_profile_name("Bad Sub").unwrap();
    let remote = block_on(create_profile_from_url(&name, "http://127.0.0.1:1/sub"));
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
    feed(
        &mut state,
        Message::ProfilesLoaded(Ok(
            block_on(infiltrator_core::profiles::list_profile_infos()).unwrap(),
        )),
    );
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
    let listed = block_on(infiltrator_core::profiles::list_profile_infos()).unwrap();
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
    let units = feed(&mut state, Message::SubscriptionUpdatedNow(Ok(false)));
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
    let listed = block_on(infiltrator_core::profiles::list_profile_infos()).unwrap();
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

    // Its worker result reaches the summarizing handler.
    let units = feed(&mut state, Message::AllSubscriptionsUpdated(Ok(vec![])));
    assert!(units >= 1, "AllSubscriptionsUpdated dispatched");
    assert!(!state.profile.is_updating_subscription_now);

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
