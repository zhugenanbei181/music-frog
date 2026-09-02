//! Journeys 12/13/14/17 — the notification task surface, factory reset over
//! a real temp HOME, language/theme persistence mirroring, and the toast
//! lifecycle (redaction + stale-index safety).
//!
//! test-intent: behavior

use super::support::{TempHome, block_on, feed, fresh_state, subscribed_profile};
use crate::types::app::{ConfirmAction, ToastStatus};
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use infiltrator_core::settings::{self, AppSettings};

/// Journey 12 — 通知事件面：SubscriptionAutoUpdated Ok → toast + 系统通知
/// 任务；notifications_enabled=false 时通知腿归零（`Task::none`，零开销）。
#[test]
fn subscription_auto_updated_notification_task_honours_the_master_switch() {
    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();
    state.shell.notifications_enabled = true;

    // Empty update list → silence (no toast, no notification, no tray work).
    let units = feed(
        &mut state,
        Message::SubscriptionAutoUpdated(Ok((vec![], false))),
    );
    assert_eq!(units, 0, "nothing updated → nothing emitted");
    assert!(state.shell.toasts.is_empty());

    // Notifications on: LoadProfiles + toast + system-notify legs.
    let units = feed(
        &mut state,
        Message::SubscriptionAutoUpdated(Ok((vec!["Paid".into()], false))),
    );
    assert_eq!(
        units, 3,
        "reload + success-toast + system-notification legs"
    );

    // Master switch off: the notification leg collapses to Task::none while
    // the in-app feedback stays.
    state.shell.notifications_enabled = false;
    let units = feed(
        &mut state,
        Message::SubscriptionAutoUpdated(Ok((vec!["Free".into()], false))),
    );
    assert_eq!(units, 2, "notification leg is a literal no-op");

    // Failure leg: warning toast + (when enabled) a Critical notification.
    state.shell.notifications_enabled = true;
    let units = feed(
        &mut state,
        Message::SubscriptionAutoUpdated(Err(infiltrator_core::error::InfiltratorError::Config(
            "拉取失败".into(),
        ))),
    );
    assert_eq!(units, 2, "warning-toast + critical-notification legs");
    assert!(state.shell.error_msg.is_some());
}

/// Journey 13 — 恢复出厂：临时 HOME 造 settings/logs/configs → 确认流 →
/// 真实 reset 执行 → FactoryResetFinished 回灌整体重置 → 默认 profile 回来
/// （系统代理/自启两条腿属于真实系统副作用，测试刻意不执行）。
#[test]
fn factory_reset_wipes_temp_home_and_boots_back_into_defaults() {
    let home = TempHome::acquire("factory-reset");
    // A used app directory: settings, logs, two profiles (one current).
    let settings = AppSettings::default();
    block_on(async {
        let path = settings::settings_path(&home).unwrap();
        settings::save_settings(&path, &settings).await.unwrap();
    });
    std::fs::create_dir_all(home.join("logs")).unwrap();
    std::fs::write(home.join("logs/app-2026-08-31.log"), "old logs").unwrap();
    home.seed_profile("default", "mixed-port: 7890\nmode: rule\n");
    home.seed_profile("Custom", "mixed-port: 7891\nmode: global\n");
    assert!(home.join("settings.toml").exists());
    assert!(home.configs().join("Custom.yaml").exists());

    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();
    state.runtime.status = RuntimeStatus::Running;

    // Settings page → confirmation staged → confirmed → reset armed.
    feed(
        &mut state,
        Message::RequestConfirmation(ConfirmAction::FactoryReset),
    );
    assert_eq!(state.shell.confirmation, Some(ConfirmAction::FactoryReset));
    let units = feed(&mut state, Message::ConfirmAction);
    assert!(state.shell.confirmation.is_none(), "dialog consumed");
    assert!(state.shell.is_factory_resetting);
    assert_eq!(state.runtime.status, RuntimeStatus::Stopped);
    assert!(units >= 1, "reset task armed (system legs skipped here)");

    // The filesystem part of the task body, for real: purge + reseed
    // defaults. (apply_system_proxy(None) / autostart disable are the two
    // system-touching lines and are deliberately not replicated.)
    let configs_dir = home.configs();
    let report = block_on(async {
        infiltrator_core::factory_reset::execute(&home, Some(&configs_dir)).unwrap()
    });
    assert!(
        report.warnings.is_empty(),
        "clean temp home resets warning-free"
    );
    block_on(infiltrator_core::profiles::reset_profiles_to_default()).unwrap();

    // Files are gone / back to factory shape.
    assert!(!home.join("settings.toml").exists(), "AppSettings wiped");
    assert!(!home.join("logs/app-2026-08-31.log").exists(), "logs wiped");
    assert!(
        !home.configs().join("Custom.yaml").exists(),
        "custom profile wiped"
    );
    assert!(
        home.configs().join("default.yaml").exists(),
        "default profile reseeded"
    );

    // Result 回灌: the whole state machine is replaced with a fresh one.
    let units = feed(&mut state, Message::FactoryResetFinished(Ok(())));
    assert!(!state.shell.is_factory_resetting);
    assert!(state.shell.error_msg.is_none());
    assert!(
        state.profile.profiles.is_empty(),
        "fresh state, catalog empty"
    );
    assert!(matches!(
        state.runtime.rebuild_flow,
        crate::types::runtime::RebuildFlowState::Idle
    ));
    assert!(units >= 4, "LoadProfiles + LoadKernels + settings + toast");

    // The post-reset LoadProfiles would list the reseeded default only.
    let listed = block_on(async {
        crate::configs_dir::config_manager()
            .await
            .unwrap()
            .list_profiles()
            .await
            .unwrap()
    });
    feed(&mut state, Message::ProfilesLoaded(Ok(listed)));
    assert_eq!(state.profile.profiles.len(), 1);
    assert_eq!(state.profile.profiles[0].name, "default");
    assert!(state.profile.profiles[0].active);

    // Failure leg: banner + localized failure toast, no state reset.
    let units = feed(
        &mut state,
        Message::FactoryResetFinished(Err(infiltrator_core::error::InfiltratorError::Config(
            "settings.toml 删除失败".into(),
        ))),
    );
    assert!(!state.shell.is_factory_resetting);
    assert!(state.shell.error_msg.is_some());
    assert_eq!(units, 1, "failure toast armed; state NOT reset");
}

/// Journey 14 — 语言/主题切换 → UI 域断言 → 持久化 → SettingsLoaded 镜像
/// 回灌（重启路径）。
#[test]
fn language_and_theme_switches_persist_and_mirror_back_on_startup() {
    let home = TempHome::acquire("lang-theme");
    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();
    assert_eq!(state.shell.theme, iced::Theme::Dark);

    // User switches language and theme through the UI.
    feed(&mut state, Message::SetLanguage("en-US".into()));
    assert_eq!(state.shell.lang, "en-US");
    feed(&mut state, Message::SetLanguage("zh".into()));
    assert_eq!(state.shell.lang, "zh-CN", "aliases normalize to zh-CN");
    feed(&mut state, Message::SetLanguage("en-US".into()));
    let units = feed(&mut state, Message::ToggleTheme);
    assert_eq!(units, 0);
    assert_eq!(state.shell.theme, iced::Theme::Light);

    // Save runs the load-modify-save task; execute its body for real.
    let _task = feed(&mut state, Message::SaveAppSettings);
    assert!(state.profile.is_saving_app_settings);
    let saved = block_on(async {
        let base_dir = mihomo_platform::paths::get_home_dir().unwrap();
        let path = settings::settings_path(&base_dir).unwrap();
        let mut stored = settings::load_settings(&path).await.unwrap_or_default();
        stored.language = state.shell.lang.clone();
        stored.theme = "light".into();
        stored.editor_path = None;
        settings::save_settings(&path, &stored).await.unwrap();
        settings::load_settings(&path).await.unwrap()
    });
    let units = feed(&mut state, Message::AppSettingsSaved(Ok(())));
    assert!(!state.profile.is_saving_app_settings);
    assert_eq!(units, 1, "success toast");
    assert!(
        home.join("settings.toml").exists(),
        "settings persisted in temp home"
    );

    // Startup path: the same file comes back through SettingsLoaded and
    // mirrors onto every UI domain field.
    let mut state2 = fresh_state();
    assert_eq!(state2.shell.lang, "zh-CN");
    feed(&mut state2, Message::SettingsLoaded(Ok(saved)));
    assert_eq!(state2.shell.lang, "en-US");
    assert_eq!(state2.shell.theme, iced::Theme::Light);
    assert!(
        state2.profile.webdav_sync_interval_mins == "60",
        "webdav defaults mirrored from the stored settings"
    );

    // A corrupt settings file degrades to the error banner, not a crash.
    let mut state3 = fresh_state();
    let units = feed(
        &mut state3,
        Message::SettingsLoaded(Err(infiltrator_core::error::InfiltratorError::Config(
            "TOML parse error".into(),
        ))),
    );
    assert!(state3.shell.error_msg.is_some());
    assert_eq!(units, 0);
}

/// Journey 17 — Toast 生命周期：脱敏单一入口 + 过期索引移除不 panic +
/// 5s 自动消退任务武装。
#[test]
fn toast_lifecycle_redacts_secrets_and_survives_stale_removal() {
    let mut state = fresh_state();

    let units = feed(
        &mut state,
        Message::ShowToast(
            "update failed: https://sub.example.com/d?token=tok1234".into(),
            ToastStatus::Error,
        ),
    );
    assert_eq!(units, 1, "auto-dismiss task armed");
    let (content, status) = &state.shell.toasts[0];
    assert_eq!(status, &ToastStatus::Error);
    assert!(content.contains("token=***"), "secret redacted: {content}");
    assert!(!content.contains("tok1234"), "raw token must not render");

    // Stale index (toast already gone) is ignored instead of panicking.
    let units = feed(&mut state, Message::RemoveToast(999));
    assert_eq!(units, 0);
    assert_eq!(state.shell.toasts.len(), 1);

    // Removing the real index clears the toast.
    feed(&mut state, Message::RemoveToast(0));
    assert!(state.shell.toasts.is_empty());

    // The auto-update notification path re-checks the subscription catalog:
    // the editor resyncs from the loaded profiles after reload.
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
}
