//! 0.20 OS 系统通知：订阅自动更新 / WebDAV 周期同步 / 内核错误在窗口不可见时
//! 也必须能触达用户，因此走桌面通知守护进程而不是应用内 toast（用户主动操作
//! 的反馈仍然只走 toast）。
//!
//! 设计要点：
//! * Linux/*BSD 走 [`notify_rust`] 的 zbus 纯 Rust 后端（与托盘 ksni 共享
//!   zbus 5 依赖树），若 D-Bus 失败则平滑降级至 `notify-send` 命令行；
//! * macOS 走 `osascript display notification` 与 `UNUserNotificationCenter` 适配；
//! * Windows 走 WinRT `ToastNotificationManager` (ToastGeneric / ToastText02)
//!   及 PowerShell `System.Windows.Forms.NotifyIcon` 异步气泡回退。
//! * 静默降级：任何失败只 `log::warn!`（限频，见 [`warn_throttled`]），
//!   绝不 panic、绝不回弹 toast。
//! * [`AppState::system_notify`] 是唯一生产入口：尊重
//!   `shell.notifications_enabled`（关闭时零开销返回 `Task::none()`），
//!   通知提交放 `spawn_blocking` 并加 2s 超时护栏，完成后丢弃结果。
//! * 测试钩子（仅测试用，非产品功能）：环境变量
//!   `INFILTRATOR_FORCE_NOTIFY=1` 时 demo 模式不再短路
//!   `system_notify`（`notifications_enabled` 开关语义不变），且应用启动
//!   时会经真实 `send`/通知链发一条探针通知（[`startup_probe_task`]），
//!   供 `scripts/desktop-smoke.sh notify` 在私有总线上验证「应用侧 →
//!   通知守护进程」全链。设置该变量的会话可能向桌面弹出测试通知，
//!   不要在生产环境启用。
//!
//! 标题本地化：调用方传 locale key（`notify_*`，见 locales_table.rs），
//! 正文拼数据（profile 名 / 错误串，`send` 内部统一过
//! [`crate::utils::sanitize_ui_text`] 做脱敏）。

use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_desktop::notify::{NotificationLevel, warn_throttled};
use infiltrator_shared::locales::{Lang, Localizer};

/// 桌面通知紧急程度（映射 org.freedesktop.Notifications urgency hint）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotifyUrgency {
    Low,
    Normal,
    Critical,
}

impl From<NotifyUrgency> for NotificationLevel {
    fn from(urgency: NotifyUrgency) -> Self {
        match urgency {
            NotifyUrgency::Low => NotificationLevel::Info,
            NotifyUrgency::Normal => NotificationLevel::Info,
            NotifyUrgency::Critical => NotificationLevel::Error,
        }
    }
}

/// 同步提交一条系统通知；正文/标题先过 [`crate::utils::sanitize_ui_text`]。
/// 返回守护进程是否受理（仅用于日志/测试，调用方无需处理）。
pub fn send(title: &str, body: &str, urgency: NotifyUrgency) -> bool {
    let title = crate::utils::sanitize_ui_text(title);
    let body = crate::utils::sanitize_ui_text(body);
    backend::send(&title, &body, urgency)
}

#[cfg(all(unix, not(target_os = "macos")))]
mod backend {
    use super::NotifyUrgency;
    use infiltrator_desktop::notify::{SystemNotification, SystemNotifier, warn_throttled};

    fn to_daemon_urgency(urgency: NotifyUrgency) -> notify_rust::Urgency {
        match urgency {
            NotifyUrgency::Low => notify_rust::Urgency::Low,
            NotifyUrgency::Normal => notify_rust::Urgency::Normal,
            NotifyUrgency::Critical => notify_rust::Urgency::Critical,
        }
    }

    /// 阻塞式 D-Bus 提交，必须运行在阻塞线程上（见
    /// [`AppState::system_notify`] 的 spawn_blocking 护栏）。
    pub(super) fn send(title: &str, body: &str, urgency: NotifyUrgency) -> bool {
        let result = notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .urgency(to_daemon_urgency(urgency))
            .show();
        match result {
            Ok(_handle) => true,
            Err(error) => {
                warn_throttled(&format!(
                    "notify-rust D-Bus notification failed, falling back to notify-send: {error}"
                ));
                let notif = SystemNotification::new(title, body, urgency.into());
                let notifier = SystemNotifier::new();
                notifier.send(&notif).is_ok()
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod backend {
    use super::NotifyUrgency;
    use infiltrator_desktop::notify::{SystemNotification, SystemNotifier, warn_throttled};

    /// macOS 原生 osascript display notification 与 UNUserNotificationCenter 适配。
    pub(super) fn send(title: &str, body: &str, urgency: NotifyUrgency) -> bool {
        let notif = SystemNotification::new(title, body, urgency.into());
        let notifier = SystemNotifier::new();
        match notifier.send(&notif) {
            Ok(()) => true,
            Err(e) => {
                warn_throttled(&format!("macOS notification dispatch failed: {e}"));
                false
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod backend {
    use super::NotifyUrgency;
    use infiltrator_desktop::notify::{SystemNotification, SystemNotifier, warn_throttled};

    /// Windows 原生 WinRT ToastNotificationManager 及 PowerShell 异步回退。
    pub(super) fn send(title: &str, body: &str, urgency: NotifyUrgency) -> bool {
        let notif = SystemNotification::new(title, body, urgency.into());
        let notifier = SystemNotifier::new();
        match notifier.send(&notif) {
            Ok(()) => true,
            Err(e) => {
                warn_throttled(&format!("Windows WinRT/PowerShell notification dispatch failed: {e}"));
                false
            }
        }
    }
}

#[cfg(not(any(unix, target_os = "windows")))]
mod backend {
    use super::{NotifyUrgency, warn_throttled};

    /// 该平台没有接入通知后端：记日志后如实报告未投递。
    pub(super) fn send(title: &str, body: &str, urgency: NotifyUrgency) -> bool {
        log::info!(
            "system notification ({urgency:?}) not delivered on this platform: {title}: {body}"
        );
        warn_throttled("system notifications are not supported on this platform");
        false
    }
}

/// desktop-smoke 测试钩子开关（仅测试用）：`INFILTRATOR_FORCE_NOTIFY=1`。
/// 设置后 demo 模式的 `system_notify` 短路被绕过（`notifications_enabled`
/// 开关语义保持），且应用启动即发一条探针通知供 smoke 断言。
pub(crate) fn force_notify_requested() -> bool {
    std::env::var("INFILTRATOR_FORCE_NOTIFY").is_ok_and(|value| value.trim() == "1")
}

/// 探针通知的固定 ASCII 标题：desktop-smoke 在守护进程历史里按它断言。
pub(crate) const SMOKE_PROBE_TITLE: &str = "infiltrator-notify-probe";

/// desktop-smoke 专用启动探针（仅测试用）：`INFILTRATOR_FORCE_NOTIFY=1`
/// 时返回一个走真实 `send` → 通知后端链的任务，把一条标题为
/// [`SMOKE_PROBE_TITLE`] 的通知发给系统通知守护进程；未设置时返回
/// `Task::none()`（生产启动零开销）。由 `AppState::new` 在启动批次里挂载。
pub(crate) fn startup_probe_task() -> Task<Message> {
    if !force_notify_requested() {
        return Task::none();
    }
    let title = SMOKE_PROBE_TITLE.to_string();
    let body = format!(
        "app-to-daemon probe {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    );
    Task::perform(
        async move {
            let delivered =
                tokio::task::spawn_blocking(move || send(&title, &body, NotifyUrgency::Normal))
                    .await
                    .unwrap_or_else(|join_error| {
                        warn_throttled(&format!("notify probe task failed: {join_error}"));
                        false
                    });
            log::info!("notify probe delivered={delivered}");
        },
        |()| Message::Noop,
    )
}

impl AppState {
    /// 唯一生产入口：`shell.notifications_enabled == false` 时零开销返回
    /// `Task::none()`；否则在 `spawn_blocking` 上提交（2s 超时护栏，超时只
    /// 限频告警，UI 不被拖住），结果经 [`Task::discard`] 丢弃。
    /// `title_key` 是 `notify_*` locale key。
    pub(crate) fn system_notify(
        &self,
        title_key: &str,
        body: &str,
        urgency: NotifyUrgency,
    ) -> Task<Message> {
        if !self.shell.notifications_enabled {
            return Task::none();
        }
        // demo 模式默认不触达桌面通知守护进程；INFILTRATOR_FORCE_NOTIFY=1
        // 时放行（desktop-smoke 全链验证用，见模块文档）。
        if self.shell.demo && !force_notify_requested() {
            return Task::none();
        }
        let title = Lang(&self.shell.lang).tr(title_key).into_owned();
        let body = body.to_string();
        Task::perform(
            async move {
                let attempt = tokio::task::spawn_blocking(move || send(&title, &body, urgency));
                match tokio::time::timeout(std::time::Duration::from_secs(2), attempt).await {
                    Ok(Ok(delivered)) => delivered,
                    Ok(Err(join_error)) => {
                        warn_throttled(&format!("system notification task failed: {join_error}"));
                        false
                    }
                    Err(_elapsed) => {
                        warn_throttled("system notification timed out after 2s");
                        false
                    }
                }
            },
            |delivered: bool| delivered,
        )
        .discard()
    }
}

#[cfg(test)]
#[path = "../tests/gui/notify_tests.rs"]
mod notify_tests;
