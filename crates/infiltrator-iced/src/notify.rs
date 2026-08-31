//! 0.20 OS 系统通知：订阅自动更新 / WebDAV 周期同步 / 内核错误在窗口不可见时
//! 也必须能触达用户，因此走桌面通知守护进程而不是应用内 toast（用户主动操作
//! 的反馈仍然只走 toast）。
//!
//! 设计要点：
//! * Linux/*BSD 走 [`notify_rust`] 的 zbus 纯 Rust 后端（与托盘 ksni 共享
//!   zbus 5 依赖树）；macOS/Windows 等其余平台是 log-only stub（见
//!   `Cargo.toml` 的 target 门控）。
//! * 静默降级：任何失败只 `log::warn!`（限频，见 [`warn_throttled`]），
//!   绝不 panic、绝不回弹 toast。
//! * [`AppState::system_notify`] 是唯一生产入口：尊重
//!   `shell.notifications_enabled`（关闭时零开销返回 `Task::none()`），
//!   D-Bus 提交放 `spawn_blocking` 并加 2s 超时护栏，完成后丢弃结果。
//!
//! 标题本地化：调用方传 locale key（`notify_*`，见 locales_table.rs），
//! 正文拼数据（profile 名 / 错误串，`send` 内部统一过
//! [`crate::utils::sanitize_ui_text`] 做脱敏）。

use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_shared::locales::{Lang, Localizer};

/// 桌面通知紧急程度（映射 org.freedesktop.Notifications urgency hint）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyUrgency {
    Low,
    Normal,
    Critical,
}

/// warn 限频：首次失败立即记录，之后每个窗口期最多一条，避免通知守护进程
/// 不可用时刷爆日志（绝不 panic、绝不回弹 toast）。
fn warn_throttled(message: &str) {
    static LAST_WARN_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    const THROTTLE_MS: u64 = 60_000;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();
    let last = LAST_WARN_MS.load(std::sync::atomic::Ordering::Relaxed);
    if last != 0 && now.saturating_sub(last) < THROTTLE_MS {
        return;
    }
    // CAS 保证并发失败在一个窗口内只产出一条日志。
    if LAST_WARN_MS
        .compare_exchange(last, now, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed)
        .is_ok()
    {
        log::warn!("{message}");
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
    use super::{NotifyUrgency, warn_throttled};

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
                warn_throttled(&format!("system notification failed: {error}"));
                false
            }
        }
    }
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
mod backend {
    use super::{NotifyUrgency, warn_throttled};

    /// 该平台没有接入通知后端：记日志后如实报告未投递。
    pub(super) fn send(title: &str, body: &str, urgency: NotifyUrgency) -> bool {
        log::info!("system notification ({urgency:?}) not delivered on this platform: {title}: {body}");
        warn_throttled("system notifications are not supported on this platform");
        false
    }
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
        if !self.shell.notifications_enabled || self.shell.demo {
            return Task::none();
        }
        let title = Lang(&self.shell.lang).tr(title_key).into_owned();
        let body = body.to_string();
        Task::perform(
            async move {
                let attempt =
                    tokio::task::spawn_blocking(move || send(&title, &body, urgency));
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
mod tests {
    use super::*;

    /// 限频状态机冒烟：首次调用登记时间戳、窗口内的后续调用被静默。
    /// 纯内存行为，不触碰任何通知后端；断言以"不 panic、无副作用外泄"为准。
    #[test]
    fn warn_throttled_first_call_then_throttled() {
        warn_throttled("notify tests: first");
        warn_throttled("notify tests: throttled within window");
    }

    #[test]
    fn urgency_is_copy_and_comparable() {
        let low = NotifyUrgency::Low;
        let copy = low;
        assert_eq!(copy, NotifyUrgency::Low);
        assert_ne!(low, NotifyUrgency::Critical);
    }

    /// 仅 stub 平台（macOS/Windows 等）：send 恒为 false 且不 panic，
    /// 保证测试 determinism；Linux/*BSD 会触达真实 D-Bus，不在此冒烟。
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    #[test]
    fn send_is_a_noop_false_on_stub_platforms() {
        assert!(!send("notify_test", "token=abc123", NotifyUrgency::Low));
    }
}
