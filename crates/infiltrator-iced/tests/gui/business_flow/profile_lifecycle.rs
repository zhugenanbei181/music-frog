//! Journeys 1/2/3/16 — the subscription-to-kernel profile lifecycle:
//! import (bad URL fast-fail + real local import), activation, the restart
//! chain, subscription settings + auto-update persistence, manual update
//! outcomes and profile deletion with sidecar cleanup — all against a real
//! temp-HOME config store.
//!
//! test-intent: behavior

use super::support::{TempHome, block_on, feed, fresh_state, list_profiles, subscribed_profile};
use crate::configs_dir::{config_manager, configs_dir};
use crate::host::runtime::application_runtime;
use crate::host::storage::subscription_import_port;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::update::core::profile_apply::activate_profile;
use crate::update::profile::subscription::subscription_update_toast;
use infiltrator_application::profile_application::{
    ProfileActivationOutcome, ProfileRuntimeAccess,
};
use infiltrator_application::profile_options_application::ProfileOptionsApplication;
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use infiltrator_composition::mihomo_configuration::{
    delete_profile_options_fixture, save_profile_options_fixture,
};
use infiltrator_contract::core_control::CoreControlLabel;
use infiltrator_contract::subscription_import::{
    SubscriptionBatchReport, SubscriptionContentEffectOutcome, SubscriptionFilterDraft,
    SubscriptionScheduleDraft, SubscriptionUpdateOutcome, SubscriptionUpdateReport,
};
use infiltrator_core::subscription_io::HttpSubscriptionSource;
use infiltrator_domain::config::validate_yaml;
use infiltrator_domain::profile_options::{FilterSpec, ProfileOptions};
use infiltrator_domain::profiles::sanitize_profile_name;
use infiltrator_domain::subscription::CheckedSubscriptionUrl;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::subscription_source::{
    ConditionalDocumentResult, ConditionalFetchHeaders, SubscriptionDocument, SubscriptionSource,
};
use infiltrator_shared::locales::Lang;
use infiltrator_test_support::subscription_filter_fixture::{FIXTURE_DOCUMENT, observation};
use lurker_core_contract::error::InfiltratorError;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_ports::error::PortError;
use std::fs::write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time;
use tokio::fs::read_to_string;

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
        content_effect: SubscriptionContentEffectOutcome::NotAttempted,
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
        content_effect: SubscriptionContentEffectOutcome::NotAttempted,
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
    assert!(
        state.profile.url_import_phase.is_importing(),
        "import is in flight"
    );
    assert!(units >= 1, "import task spawned");

    // The import task body, run for real: sanitize → application subscription
    // use-case on a closed port fails without any network egress.
    let name = sanitize_profile_name("Bad Sub").unwrap();
    let remote = block_on(async {
        let store = config_manager().await.unwrap();
        let application = profiles_for_mihomo(store);
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
    assert!(!state.profile.url_import_phase.is_importing());
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
    write(&source, LOCAL_IMPORT_YAML).unwrap();
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
    assert!(state.profile.local_import_phase.is_importing());

    // Task body for real: read_to_string → validate_yaml → save → activate.
    let stored = block_on(async {
        let content = read_to_string(&source).await.unwrap();
        validate_yaml(&content).unwrap();
        let manager = config_manager().await.unwrap();
        ProfileStore::save_inactive_profile(manager.as_ref(), "Travel Node", &content)
            .await
            .unwrap();
        activate_profile(ProfileRuntimeAccess::Offline, "Travel Node")
            .await
            .unwrap()
    });
    assert_eq!(
        stored,
        ProfileActivationOutcome::SelectedWithoutRuntime,
        "no runtime → profile selection is complete and Core startup remains required"
    );

    let units = feed(&mut state, Message::LocalProfileImported(Ok(Some(stored))));
    assert!(!state.profile.local_import_phase.is_importing());
    assert!(state.profile.local_import_path.is_empty(), "form cleared");
    // Selecting without a runtime completes import; starting the Core remains
    // an explicit lifecycle action.
    assert_eq!(units, 2, "LoadProfiles + success toast");

    // ---- explicit activation of another profile ----
    feed(&mut state, Message::ProfilesLoaded(Ok(list_profiles())));
    state.shell.error_msg = Some("stale".into());
    let units = feed(&mut state, Message::SetActiveProfile("default".into()));
    assert!(
        state.shell.error_msg.is_none(),
        "activation clears the banner"
    );
    assert_eq!(units, 1);

    let activation =
        block_on(async { activate_profile(ProfileRuntimeAccess::Offline, "default").await })
            .unwrap();
    assert_eq!(activation, ProfileActivationOutcome::SelectedWithoutRuntime);
    let units = feed(
        &mut state,
        Message::ProfileActivationFinished(Ok(activation)),
    );
    assert_eq!(
        units, 1,
        "offline activation refreshes the profile list only"
    );

    // The current pointer really moved on disk.
    let current = block_on(async { config_manager().await.unwrap().get_current().await.unwrap() });
    assert_eq!(current, "default");

    assert_eq!(
        state.core_control_projection().label,
        CoreControlLabel::Unavailable,
        "profile activation cannot start a Core without its application owner"
    );
    assert!(state.runtime.lifecycle_pending.is_none());
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
    assert!(!state.profile.subscription_save_phase.is_saving());

    feed(&mut state, Message::UpdateSubscriptionUrl("  ".into()));
    let units = feed(&mut state, Message::SaveSubscriptionSettings);
    assert_eq!(units, 1, "auto-update without URL → toast only");
    assert!(!state.profile.subscription_save_phase.is_saving());

    // Happy path: gates pass, the persistence task spawns (not run here —
    // its URL leg would touch the OS keyring), and the result 回灌 reloads.
    feed(
        &mut state,
        Message::UpdateSubscriptionUrl("https://sub.example.com/token".into()),
    );
    feed(&mut state, Message::UpdateSubscriptionInterval("12".into()));
    let units = feed(&mut state, Message::SaveSubscriptionSettings);
    assert!(state.profile.subscription_save_phase.is_saving());
    assert_eq!(units, 1);
    let units = feed(&mut state, Message::SubscriptionSettingsSaved(Ok(())));
    assert!(!state.profile.subscription_save_phase.is_saving());
    assert_eq!(units, 2, "LoadProfiles + success-toast legs");

    // Persistence leg for real: the URL-less metadata write path (what the
    // tray auto-update toggle persists) lands in config.toml and round-trips
    // through the manager that LoadProfiles would read.
    block_on(async {
        let manager = config_manager().await.unwrap();
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
        home.profile_config_dir()
            .join("profile-state.toml")
            .exists(),
        "metadata belongs to the engine-scoped profile store"
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
    assert!(!state.profile.subscription_update_phase.is_updating());

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
    assert!(state.profile.subscription_update_phase.is_updating());
    assert!(units >= 1, "update task spawned");

    // Worker would fetch → parse → (active profile, no runtime) clear backup.
    let units = feed(
        &mut state,
        Message::SubscriptionUpdatedNow(Ok(subscription_report("Paid"))),
    );
    assert!(!state.profile.subscription_update_phase.is_updating());
    assert_eq!(units, 2, "LoadProfiles + success-toast legs");

    // Failure leg: banner + error toast, flag cleared.
    feed(&mut state, Message::UpdateSubscriptionNow);
    let units = feed(
        &mut state,
        Message::SubscriptionUpdatedNow(Err(InfiltratorError::Config("订阅拉取失败".into()))),
    );
    assert!(!state.profile.subscription_update_phase.is_updating());
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
    assert!(state.profile.subscription_update_phase.is_updating());
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
    assert!(!state.profile.subscription_update_phase.is_updating());

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
    block_on(save_profile_options_fixture(
        &home.profile_config_dir(),
        "Doomed",
        &ProfileOptions {
            mixin: Default::default(),
            filter: Some(FilterSpec {
                include_keywords: vec!["HK".into()],
                ..Default::default()
            }),
        },
    ))
    .unwrap();
    assert!(
        home.profile_config_dir()
            .join("options/Doomed.yaml")
            .exists()
    );

    let mut state = fresh_state();
    let listed = list_profiles();
    assert_eq!(listed.len(), 2);
    feed(&mut state, Message::ProfilesLoaded(Ok(listed)));

    let units = feed(&mut state, Message::DeleteProfile("Doomed".into()));
    assert_eq!(units, 1);

    // Task body for real: delete + best-effort sidecar cleanup.
    block_on(async {
        let manager = config_manager().await.unwrap();
        manager.delete_profile("Doomed").await.unwrap();
        let dir = configs_dir().await.unwrap();
        delete_profile_options_fixture(&dir, "Doomed")
            .await
            .unwrap();
    });

    let units = feed(&mut state, Message::ProfileDeleted(Ok(())));
    assert_eq!(units, 2, "LoadProfiles + toast legs");
    assert!(!home.profile_config_dir().join("Doomed.yaml").exists());
    assert!(
        !home
            .profile_config_dir()
            .join("options/Doomed.yaml")
            .exists()
    );
    assert!(
        home.profile_config_dir().join("Keeper.yaml").exists(),
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
    assert!(
        state.profile.subscription_update_phase.is_updating(),
        "handler armed"
    );

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
    assert!(!state.profile.subscription_update_phase.is_updating());
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
        Message::EditProfileAs(PathBuf::from("/configs/Paid.yaml"), EditorPane::Mixin),
    );
    assert!(units >= 1, "EditProfileAs dispatched");
    // The path binds asynchronously (ProfileContentLoaded); the pane is
    // what's selected synchronously.
    assert!(state.editor.editor_path.is_none(), "content not loaded yet");
    assert_eq!(
        state.editor.editor_pane,
        EditorPane::Mixin,
        "pane preselected"
    );
}

#[path = "profile_lifecycle/subscription.rs"]
mod subscription;
