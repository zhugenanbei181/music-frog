//! Journeys 12/13/14/17 — the notification task surface, factory reset over
//! a real temp HOME, language/theme persistence mirroring, and the toast
//! lifecycle (redaction + stale-index safety).
//!
//! test-intent: behavior

use super::support::{TempHome, block_on, feed, fresh_state, list_profiles, subscribed_profile};
use crate::host::storage::settings_store;
use crate::test_mounts::command_harness::{recording_application, rejecting_application};
use crate::types::app::{ConfirmAction, ToastStatus};
use crate::types::message::Message;
use crate::types::runtime::{RebuildFlowState, RuntimeStatus};
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::language_choice::project_language;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_core::factory_reset::execute;
use infiltrator_core::profile_reset::reset_profiles_to_default;
use infiltrator_core::settings_io;
use infiltrator_domain::settings::AppSettings;
use std::fs::{create_dir_all, write};
use std::sync::Arc;

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
        Message::SubscriptionAutoUpdated(Err(InfiltratorError::Config("拉取失败".into()))),
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
        let path = settings_io::settings_path(&home).unwrap();
        settings_io::save_settings(&path, &settings).await.unwrap();
    });
    create_dir_all(home.join("logs")).unwrap();
    write(home.join("logs/app-2026-08-31.log"), "old logs").unwrap();
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
    let report = block_on(async { execute(&home, Some(&configs_dir)).unwrap() });
    assert!(
        report.warnings.is_empty(),
        "clean temp home resets warning-free"
    );
    block_on(reset_profiles_to_default()).unwrap();

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
    assert!(matches!(state.runtime.rebuild_flow, RebuildFlowState::Idle));
    assert!(units >= 4, "LoadProfiles + LoadKernels + settings + toast");

    // The post-reset LoadProfiles would list the reseeded default only.
    let listed = list_profiles();
    feed(&mut state, Message::ProfilesLoaded(Ok(listed)));
    assert_eq!(state.profile.profiles.len(), 1);
    assert_eq!(state.profile.profiles[0].name, "default");
    assert!(state.profile.profiles[0].active);

    // Failure leg: banner + localized failure toast, no state reset.
    let units = feed(
        &mut state,
        Message::FactoryResetFinished(Err(InfiltratorError::Config(
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

    let settings = SettingsApplication::new(block_on(settings_store()).unwrap());
    let initial = block_on(settings.load()).unwrap();
    state.observe_language_settings(&project_language(Some(&Ok(initial))));
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_settings(settings.clone()),
    ));
    state.commands = Some(application);
    let task = state.update(Message::SetLanguage("en-US".into()));
    assert_eq!(state.shell.lang, "zh-CN", "accepted is not saved");
    feed(&mut state, language_terminal(task));
    assert_eq!(state.shell.lang, "en-US");
    let task = state.update(Message::SetLanguage("zh".into()));
    feed(&mut state, language_terminal(task));
    assert_eq!(state.shell.lang, "zh-CN", "aliases normalize to zh-CN");
    let task = state.update(Message::SetLanguage("en-US".into()));
    feed(&mut state, language_terminal(task));
    let units = feed(&mut state, Message::SetTheme("light".into()));
    assert_eq!(units, 0);
    assert_eq!(state.shell.theme, iced::Theme::Light);

    let task = state.update(Message::SaveAppSettings);
    assert!(state.profile.is_saving_app_settings);
    let completion = language_terminal(task);
    assert!(
        matches!(completion, Message::AppSettingsSaved(Ok(()))),
        "the actual save task must finish successfully"
    );
    let units = feed(&mut state, completion);
    let saved = block_on(settings.load()).unwrap();
    assert_eq!(saved.language, "en-US");
    assert_eq!(saved.theme, "light");
    assert!(!state.profile.is_saving_app_settings);
    assert_eq!(units, 1, "success toast");
    assert!(
        home.join("settings.toml").exists(),
        "settings persisted in temp home"
    );

    // Startup path: the same file comes back through SettingsLoaded and
    // mirrors onto every UI domain field. Pin the pre-load language: a fresh
    // state follows the host system locale (`get_system_language`), which is
    // not deterministic across CI (C locale -> en-US) and dev machines.
    let mut state2 = fresh_state();
    state2.shell.lang = "zh-CN".into();
    feed(&mut state2, Message::SettingsLoaded(Ok(saved)));
    assert_eq!(state2.shell.lang, "en-US");
    assert_eq!(state2.shell.theme, iced::Theme::Light);
    assert!(
        state2.profile.webdav_sync_interval_mins == "60",
        "webdav defaults mirrored from the stored settings"
    );

    let task = state.update(Message::SetLanguage("system".into()));
    feed(&mut state, language_terminal(task));
    let task = state.update(Message::SaveAppSettings);
    let completion = language_terminal(task);
    assert!(matches!(completion, Message::AppSettingsSaved(Ok(()))));
    feed(&mut state, completion);
    assert_eq!(
        block_on(settings.load()).unwrap().language,
        "system",
        "ordinary settings save must retain the preference instead of the resolved language"
    );

    // A corrupt settings file degrades to the error banner, not a crash.
    let mut state3 = fresh_state();
    let units = feed(
        &mut state3,
        Message::SettingsLoaded(Err(InfiltratorError::Config("TOML parse error".into()))),
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
    let live_id = state.shell.toast_ids[0];
    feed(&mut state, Message::RemoveToast(live_id));
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

fn language_terminal(task: Task<Message>) -> Message {
    block_on(async {
        let mut stream = into_stream(task).expect("actual persistence task");
        let Some(Action::Output(message)) = stream.next().await else {
            panic!("actual save terminal result")
        };
        assert!(stream.next().await.is_none());
        message
    })
}

#[test]
fn rejected_language_command_retains_the_current_locale_and_exposes_retry() {
    let mut state = fresh_state();
    let (application, handler) = rejecting_application();
    state.commands = Some(application);
    state.observe_language_settings(&project_language(Some(&Ok(AppSettings::default()))));
    let current = state.shell.lang.clone();
    let task = state.update(Message::SetLanguage("en-US".into()));
    feed(&mut state, language_terminal(task));
    assert_eq!(state.shell.lang, current);
    assert_eq!(
        state
            .shell
            .language_choice
            .failure
            .as_ref()
            .unwrap()
            .message,
        "profile is read-only"
    );
    assert!(state.shell.language_choice.requested.is_some());
    assert_eq!(handler.0.lock().unwrap().len(), 1);
}
