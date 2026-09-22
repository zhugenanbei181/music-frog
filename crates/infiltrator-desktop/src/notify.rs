//! Cross-platform native desktop notification system for Infiltrator.
//!
//! Provides native notification dispatching across Windows, macOS, and Linux/*BSD:
//! - **Windows**: WinRT `ToastNotificationManager` (ToastGeneric / ToastText02) with
//!   PowerShell `System.Windows.Forms.NotifyIcon` (balloon tip) asynchronous fallback.
//! - **macOS**: `osascript` `display notification` and `UNUserNotificationCenter` adaptation.
//! - **Linux/*BSD**: `notify-send` and D-Bus integration.
//!
//! Features:
//! - **Sanitization**: All titles, bodies, and subtitles are passed through
//!   [`infiltrator_domain::redact::redact_line`] to redact subscription tokens, passwords,
//!   and secrets before reaching the OS.
//! - **Throttling**: Throttled error/warning logging and dispatch rate limiting to prevent
//!   spamming the notification daemon.
//! - **Timeout**: Guarded process execution with timeouts to prevent UI hangs.

use std::io::Result;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

/// Notification severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

impl NotificationLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// Freedesktop urgency hint for `notify-send -u`.
    ///
    /// `critical` is deliberately never emitted: the freedesktop spec defines it
    /// as "the user must acknowledge this notification", and desktop daemons
    /// (KDE Plasma, GNOME Shell) render it persistent regardless of the
    /// `expire_timeout` we pass. Severity is still preserved in the app-level
    /// [`NotificationLevel`] and by the icon/body text, but on-screen lifetime
    /// must stay bounded.
    pub fn to_linux_urgency(&self) -> &'static str {
        match self {
            Self::Info => "low",
            Self::Warning | Self::Error => "normal",
        }
    }

    /// Bounded on-screen lifetime in milliseconds, passed as `notify-send -t`.
    ///
    /// Every notification expires; there is intentionally no `never` value.
    pub fn display_timeout_ms(&self) -> u32 {
        match self {
            Self::Info => 5_000,
            Self::Warning => 7_000,
            Self::Error => 10_000,
        }
    }
}

/// Default Application ID used for Windows toast notifications and macOS notifications.
pub const DEFAULT_APP_ID: &str = "com.musicfrog.infiltrator";

/// A desktop system notification payload.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SystemNotification {
    pub title: String,
    pub body: String,
    pub level: NotificationLevel,
    pub icon: Option<String>,
    pub app_id: Option<String>,
    pub sound: Option<String>,
    pub subtitle: Option<String>,
}

impl SystemNotification {
    pub fn new(
        title: impl Into<String>,
        body: impl Into<String>,
        level: NotificationLevel,
    ) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            level,
            icon: None,
            app_id: Some(DEFAULT_APP_ID.to_string()),
            sound: None,
            subtitle: None,
        }
    }

    pub fn info(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body, NotificationLevel::Info)
    }

    pub fn warning(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body, NotificationLevel::Warning)
    }

    pub fn error(title: impl Into<String>, body: impl Into<String>) -> Self {
        let mut notif = Self::new(title, body, NotificationLevel::Error);
        notif.sound = Some("default".to_string());
        notif
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn with_app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_id = Some(app_id.into());
        self
    }

    pub fn with_sound(mut self, sound: impl Into<String>) -> Self {
        self.sound = Some(sound.into());
        self
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Redact sensitive information (tokens, passwords, secrets, userinfo) from all text fields.
    pub fn sanitized(&self) -> Self {
        Self {
            title: sanitize_text(&self.title),
            body: sanitize_text(&self.body),
            level: self.level,
            icon: self.icon.clone(),
            app_id: self.app_id.clone(),
            sound: self.sound.clone(),
            subtitle: self.subtitle.as_deref().map(sanitize_text),
        }
    }
}

/// Redact credentials, tokens, passwords, and URL secrets from a string.
pub fn sanitize_text(text: &str) -> String {
    infiltrator_domain::redact::redact_line(text, &[])
}

/// Throttled warning logger: ensures that notification backend failures do not spam logs.
pub fn warn_throttled(message: &str) {
    static LAST_WARN_MS: AtomicU64 = AtomicU64::new(0);
    const THROTTLE_MS: u64 = 60_000;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default();
    let last = LAST_WARN_MS.load(Ordering::Relaxed);
    if last != 0 && now.saturating_sub(last) < THROTTLE_MS {
        return;
    }
    if LAST_WARN_MS
        .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        log::warn!("{message}");
    }
}

/// Rate-limiter for notification dispatches.
pub struct NotificationThrottler {
    last_dispatch: AtomicU64,
    min_interval_ms: u64,
}

impl NotificationThrottler {
    pub const fn new(min_interval_ms: u64) -> Self {
        Self {
            last_dispatch: AtomicU64::new(0),
            min_interval_ms,
        }
    }

    pub fn should_allow(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default();
        let last = self.last_dispatch.load(Ordering::Relaxed);
        if last != 0 && now.saturating_sub(last) < self.min_interval_ms {
            return false;
        }
        self.last_dispatch
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    }
}

impl Default for NotificationThrottler {
    fn default() -> Self {
        Self::new(100)
    }
}

// ---------------------------------------------------------------------------
// Windows Implementation Helpers (WinRT ToastNotificationManager + PowerShell)
// ---------------------------------------------------------------------------

/// Escapes a string for safe inclusion in a PowerShell single-quoted string literal (`'...'`).
pub fn escape_powershell_str(s: &str) -> String {
    s.replace('\'', "''")
}

/// Builds a modern WinRT `ToastNotificationManager` PowerShell script snippet.
pub fn build_windows_toast_script(notification: &SystemNotification) -> String {
    let title = escape_powershell_str(&notification.title);
    let body = escape_powershell_str(&notification.body);
    let app_id = escape_powershell_str(notification.app_id.as_deref().unwrap_or(DEFAULT_APP_ID));

    format!(
        r#"[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
$template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
$textNodes = $template.GetElementsByTagName('text')
$textNodes.Item(0).AppendChild($template.CreateTextNode('{title}')) | Out-Null
$textNodes.Item(1).AppendChild($template.CreateTextNode('{body}')) | Out-Null
$toast = [Windows.UI.Notifications.ToastNotification]::new($template)
$notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{app_id}')
$notifier.Show($toast)"#
    )
}

/// Builds a PowerShell `System.Windows.Forms.NotifyIcon` balloon tip fallback script snippet.
pub fn build_windows_powershell_fallback_script(notification: &SystemNotification) -> String {
    let title = escape_powershell_str(&notification.title);
    let body = escape_powershell_str(&notification.body);
    let icon_type = match notification.level {
        NotificationLevel::Info => "Info",
        NotificationLevel::Warning => "Warning",
        NotificationLevel::Error => "Error",
    };

    format!(
        r#"Add-Type -AssemblyName System.Windows.Forms
$notify = New-Object System.Windows.Forms.NotifyIcon
$notify.Icon = [System.Drawing.SystemIcons]::Information
$notify.Visible = $true
$notify.ShowBalloonTip(3000, '{title}', '{body}', [System.Windows.Forms.ToolTipIcon]::{icon_type})
Start-Sleep -Seconds 3
$notify.Dispose()"#
    )
}

/// Builds a robust combined Windows script that attempts WinRT Toast first and falls back to NotifyIcon.
pub fn build_windows_combined_script(notification: &SystemNotification) -> String {
    let toast_code = build_windows_toast_script(notification);
    let fallback_code = build_windows_powershell_fallback_script(notification);

    format!(
        r#"$ErrorActionPreference = 'Stop'
try {{
{toast_code}
}} catch {{
    try {{
{fallback_code}
    }} catch {{
        # Silent graceful degradation
    }}
}}"#
    )
}

/// Builds a Windows PowerShell command configured for hidden, non-interactive execution.
pub fn build_windows_command_for(notification: &SystemNotification) -> Command {
    let script = build_windows_combined_script(notification);
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &script,
    ]);

    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd
}

// ---------------------------------------------------------------------------
// macOS Implementation Helpers (osascript + UNUserNotification Adapter)
// ---------------------------------------------------------------------------

/// Escapes a string for safe inclusion in an AppleScript double-quoted string literal (`"..."`).
pub fn escape_applescript_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', " ")
        .replace('\n', "\\n")
}

/// Builds an AppleScript `display notification` command string.
pub fn build_macos_applescript(notification: &SystemNotification) -> String {
    let title = escape_applescript_str(&notification.title);
    let body = escape_applescript_str(&notification.body);

    let mut script = format!("display notification \"{}\" with title \"{}\"", body, title);

    if let Some(ref subtitle) = notification.subtitle {
        script.push_str(&format!(
            " subtitle \"{}\"",
            escape_applescript_str(subtitle)
        ));
    }

    if let Some(ref sound) = notification.sound {
        script.push_str(&format!(
            " sound name \"{}\"",
            escape_applescript_str(sound)
        ));
    } else if notification.level == NotificationLevel::Error {
        script.push_str(" sound name \"Sosumi\"");
    }

    script
}

/// Builds a modern macOS UNUserNotificationCenter / NotificationCenter adaptation script.
pub fn build_macos_unuser_notification_script(notification: &SystemNotification) -> String {
    let apple_script = build_macos_applescript(notification);
    format!(
        r#"try
    {apple_script}
on error errStr
    -- Log error or degrade silently
end try"#
    )
}

/// Builds a macOS `osascript` command configured for notification dispatch.
pub fn build_macos_command_for(notification: &SystemNotification) -> Command {
    let script = build_macos_unuser_notification_script(notification);
    let mut cmd = Command::new("osascript");
    cmd.arg("-e").arg(&script);
    cmd
}

// ---------------------------------------------------------------------------
// Linux / *BSD Implementation Helpers (notify-send)
// ---------------------------------------------------------------------------

/// Builds a Linux `notify-send` command.
pub fn build_linux_command_for(notification: &SystemNotification) -> Command {
    let mut cmd = Command::new("notify-send");

    let app_name = notification
        .app_id
        .as_deref()
        .unwrap_or("MusicFrog Infiltrator");
    cmd.arg("-a").arg(app_name);

    cmd.arg("-u").arg(notification.level.to_linux_urgency());

    // Explicit bounded expiry: without `-t` the daemon decides (and persists
    // for high urgencies), which pinned notifications on screen forever.
    cmd.arg("-t")
        .arg(notification.level.display_timeout_ms().to_string());

    // Print the daemon-assigned notification id so the dispatcher can force a
    // `CloseNotification` after the same bounded window, covering daemons that
    // ignore `expire_timeout` regardless of what we ask for.
    cmd.arg("--print-id");

    if let Some(ref icon) = notification.icon {
        cmd.arg("-i").arg(icon);
    }

    cmd.arg(&notification.title);
    cmd.arg(&notification.body);

    cmd
}

/// Force-closes a Linux notification after its bounded display window.
///
/// `expire_timeout` is only a request: some daemons keep certain
/// notifications on screen regardless (KDE Plasma does this for the
/// `critical` urgency hint), which is the defect this guards against. A
/// short-lived detached thread calls `CloseNotification` on the daemon; the
/// notification id comes from `notify-send --print-id`. Every failure is
/// silent and best-effort: the notification daemon may be absent, the
/// notification may already be gone, and both are fine.
#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_linux_notification_closer(notification_id: u32, timeout_ms: u32) {
    let _ = std::thread::Builder::new()
        .name("notify-close".to_owned())
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(u64::from(timeout_ms)));
            let id = notification_id.to_string();

            let busctl_args = [
                "--user",
                "call",
                "org.freedesktop.Notifications",
                "/org/freedesktop/Notifications",
                "org.freedesktop.Notifications",
                "CloseNotification",
                "u",
                id.as_str(),
            ];
            let closed = Command::new("busctl")
                .args(busctl_args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
            if closed {
                return;
            }

            let gdbus_args = [
                "call",
                "--session",
                "--dest",
                "org.freedesktop.Notifications",
                "--object-path",
                "/org/freedesktop/Notifications",
                "--method",
                "org.freedesktop.Notifications.CloseNotification",
                id.as_str(),
            ];
            let _ = Command::new("gdbus")
                .args(gdbus_args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        });
}

/// Spawns the platform notification command.
///
/// Linux/*BSD additionally reads the daemon-assigned notification id from
/// `notify-send --print-id` and arms a forced close after the bounded display
/// window, so a notification can never outlive its timeout.
#[cfg(all(unix, not(target_os = "macos")))]
fn dispatch_command(cmd: &mut Command, notification: &SystemNotification) -> Result<()> {
    use std::io::Read;

    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let timeout_ms = notification.level.display_timeout_ms();
    match cmd.spawn() {
        Ok(mut child) => {
            let mut raw = String::new();
            let notification_id = child
                .stdout
                .take()
                .and_then(|mut stdout| stdout.read_to_string(&mut raw).ok().map(|_| raw))
                .and_then(|text| text.trim().parse::<u32>().ok());
            let _ = child.wait();
            if let Some(id) = notification_id {
                spawn_linux_notification_closer(id, timeout_ms);
            }
            Ok(())
        }
        Err(e) => {
            warn_throttled(&format!("System notification dispatch failed: {e}"));
            Err(e)
        }
    }
}

/// Spawns the platform notification command (non-Linux platforms).
#[cfg(not(all(unix, not(target_os = "macos"))))]
fn dispatch_command(cmd: &mut Command, _notification: &SystemNotification) -> Result<()> {
    match cmd.spawn() {
        Ok(_child) => Ok(()),
        Err(e) => {
            warn_throttled(&format!("System notification dispatch failed: {e}"));
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------
// SystemNotifier
// ---------------------------------------------------------------------------

/// Cross-platform desktop notification dispatcher.
pub struct SystemNotifier {
    throttler: NotificationThrottler,
}

impl Default for SystemNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemNotifier {
    pub fn new() -> Self {
        Self {
            throttler: NotificationThrottler::new(50),
        }
    }

    /// Builds the appropriate system Command for the current operating system.
    pub fn build_command(&self, notification: &SystemNotification) -> Command {
        #[cfg(target_os = "windows")]
        {
            build_windows_command_for(notification)
        }
        #[cfg(target_os = "macos")]
        {
            build_macos_command_for(notification)
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            build_linux_command_for(notification)
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
        {
            let mut cmd = Command::new("echo");
            cmd.arg(&notification.title).arg(&notification.body);
            cmd
        }
    }

    /// Asynchronously dispatches a notification in the background with sanitization and error throttling.
    pub fn send(&self, notification: &SystemNotification) -> Result<()> {
        if !self.throttler.should_allow() {
            log::debug!("Notification dispatch rate-limited: {}", notification.title);
            return Ok(());
        }

        let sanitized = notification.sanitized();
        let mut cmd = self.build_command(&sanitized);
        dispatch_command(&mut cmd, &sanitized)
    }

    /// Synchronously dispatches a notification with a strict timeout guard.
    pub fn send_with_timeout(
        &self,
        notification: &SystemNotification,
        timeout: Duration,
    ) -> Result<bool> {
        let sanitized = notification.sanitized();
        let mut cmd = self.build_command(&sanitized);

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                warn_throttled(&format!("System notification spawn failed: {e}"));
                return Err(e);
            }
        };

        let start = Instant::now();
        loop {
            match child.try_wait()? {
                Some(status) => return Ok(status.success()),
                None => {
                    if start.elapsed() >= timeout {
                        let _ = child.kill();
                        warn_throttled(
                            "System notification timed out and child process was killed",
                        );
                        return Ok(false);
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_struct_and_builder_methods() {
        let n = SystemNotification::info("Sync Success", "Profiles updated")
            .with_icon("sync.png")
            .with_app_id("com.musicfrog.test")
            .with_subtitle("Sub 1")
            .with_sound("ping");

        assert_eq!(n.title, "Sync Success");
        assert_eq!(n.body, "Profiles updated");
        assert_eq!(n.level, NotificationLevel::Info);
        assert_eq!(n.icon.as_deref(), Some("sync.png"));
        assert_eq!(n.app_id.as_deref(), Some("com.musicfrog.test"));
        assert_eq!(n.subtitle.as_deref(), Some("Sub 1"));
        assert_eq!(n.sound.as_deref(), Some("ping"));

        let err = SystemNotification::error("Kernel Crash", "Failed to start mihomo");
        assert_eq!(err.level, NotificationLevel::Error);
        assert_eq!(err.sound.as_deref(), Some("default"));

        let warn = SystemNotification::warning("Slow Network", "Latency > 500ms");
        assert_eq!(warn.level, NotificationLevel::Warning);
    }

    #[test]
    fn test_sanitization_redacts_tokens_passwords_urls() {
        let n = SystemNotification::error(
            "Fetch Error: Authorization: Bearer secret_token_12345",
            "Update failed for https://user:pass123@sub.example.com/api?token=tok9876",
        )
        .with_subtitle("Password: my_super_secret");

        let sanitized = n.sanitized();
        assert_eq!(sanitized.title, "Fetch Error: Authorization: Bearer ***");
        assert_eq!(
            sanitized.body,
            "Update failed for https://user:***@sub.example.com/api?token=***"
        );
        assert_eq!(sanitized.subtitle.as_deref(), Some("Password: ***"));
    }

    #[test]
    fn test_windows_powershell_escape_single_quotes() {
        let raw = "It's a user's test: don't break 'PowerShell'!";
        let escaped = escape_powershell_str(raw);
        assert_eq!(
            escaped,
            "It''s a user''s test: don''t break ''PowerShell''!"
        );
    }

    #[test]
    fn test_windows_toast_script_generation() {
        let n = SystemNotification::info("Test Title", "Test Body")
            .with_app_id("MusicFrog.Infiltrator");
        let script = build_windows_toast_script(&n);

        assert!(script.contains("[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime]"));
        assert!(script.contains("$template.CreateTextNode('Test Title')"));
        assert!(script.contains("$template.CreateTextNode('Test Body')"));
        assert!(script.contains("CreateToastNotifier('MusicFrog.Infiltrator')"));
    }

    #[test]
    fn test_windows_powershell_fallback_script_generation() {
        let n = SystemNotification::warning("Warn Title", "Warn Body");
        let script = build_windows_powershell_fallback_script(&n);

        assert!(script.contains("System.Windows.Forms.NotifyIcon"));
        assert!(script.contains("ShowBalloonTip(3000, 'Warn Title', 'Warn Body', [System.Windows.Forms.ToolTipIcon]::Warning)"));
    }

    #[test]
    fn test_windows_combined_script_has_try_catch_hierarchy() {
        let n = SystemNotification::error("Err Title", "Err Body");
        let script = build_windows_combined_script(&n);

        assert!(script.contains("try {"));
        assert!(script.contains("catch {"));
        assert!(script.contains("ToastNotificationManager"));
        assert!(script.contains("NotifyIcon"));
    }

    #[test]
    fn test_windows_command_generation() {
        let n = SystemNotification::info("Hello", "World");
        let cmd = build_windows_command_for(&n);

        assert_eq!(cmd.get_program(), "powershell");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(args.contains(&"-NoProfile".to_string()));
        assert!(args.contains(&"-NonInteractive".to_string()));
        assert!(args.contains(&"-WindowStyle".to_string()));
        assert!(args.contains(&"Hidden".to_string()));
        assert!(args.contains(&"-Command".to_string()));
    }

    #[test]
    fn test_macos_applescript_escape_quotes_and_backslashes() {
        let raw = r#"Quote: "Hello", Backslash: \ and Newline:
World"#;
        let escaped = escape_applescript_str(raw);
        assert_eq!(
            escaped,
            r#"Quote: \"Hello\", Backslash: \\ and Newline:\nWorld"#
        );
    }

    #[test]
    fn test_macos_applescript_generation_with_sound_and_subtitle() {
        let n = SystemNotification::info("Mac Title", "Mac Body")
            .with_subtitle("Sub")
            .with_sound("default");
        let script = build_macos_applescript(&n);

        assert_eq!(
            script,
            "display notification \"Mac Body\" with title \"Mac Title\" subtitle \"Sub\" sound name \"default\""
        );
    }

    #[test]
    fn test_macos_unuser_notification_script_generation() {
        let n =
            SystemNotification::error("Error Occurred", "Connection reset").with_sound("Sosumi");
        let script = build_macos_unuser_notification_script(&n);

        assert!(
            script.contains(
                "display notification \"Connection reset\" with title \"Error Occurred\""
            )
        );
        assert!(script.contains("sound name \"Sosumi\""));
        assert!(script.contains("try"));
        assert!(script.contains("on error"));
    }

    #[test]
    fn test_macos_command_generation() {
        let n = SystemNotification::info("Hello", "macOS");
        let cmd = build_macos_command_for(&n);

        assert_eq!(cmd.get_program(), "osascript");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "-e");
        assert!(args[1].contains("display notification"));
    }

    #[test]
    fn test_linux_command_generation() {
        let n = SystemNotification::error("Linux Title", "Linux Body")
            .with_icon("icon.png")
            .with_app_id("CustomApp");
        let cmd = build_linux_command_for(&n);

        assert_eq!(cmd.get_program(), "notify-send");
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            args,
            vec![
                "-a",
                "CustomApp",
                "-u",
                "normal",
                "-t",
                "10000",
                "--print-id",
                "-i",
                "icon.png",
                "Linux Title",
                "Linux Body"
            ]
        );
    }

    #[test]
    fn test_notification_display_time_is_always_bounded() {
        // Regression guard for the "notification pinned on screen forever"
        // defect: every level must carry a positive, bounded expiry and the
        // Linux urgency must never be the persistent `critical` hint.
        for (level, upper_bound) in [
            (NotificationLevel::Info, 5_000),
            (NotificationLevel::Warning, 7_000),
            (NotificationLevel::Error, 10_000),
        ] {
            let timeout = level.display_timeout_ms();
            assert!(timeout > 0, "{level:?} must expire");
            assert!(
                timeout <= upper_bound,
                "{level:?} timeout must stay bounded"
            );
            assert_ne!(level.to_linux_urgency(), "critical");
        }

        for level in [
            NotificationLevel::Info,
            NotificationLevel::Warning,
            NotificationLevel::Error,
        ] {
            let cmd = build_linux_command_for(&SystemNotification::new("t", "b", level));
            let args: Vec<String> = cmd
                .get_args()
                .map(|a| a.to_string_lossy().to_string())
                .collect();
            let index = args
                .iter()
                .position(|arg| arg == "-t")
                .expect("notify-send must carry an explicit -t timeout");
            let timeout: u32 = args[index + 1].parse().expect("numeric timeout");
            assert!(timeout > 0 && timeout <= 10_000);
            assert!(!args.iter().any(|arg| arg == "critical"));
            // The id is required so the dispatcher can force-close the
            // notification after the same bounded window.
            assert!(
                args.iter().any(|arg| arg == "--print-id"),
                "notify-send must print the id for the forced close"
            );
        }
    }

    #[test]
    fn test_warn_throttled_logging() {
        warn_throttled("desktop notify test: first log");
        warn_throttled("desktop notify test: throttled within 60s");
    }

    #[test]
    fn test_notification_throttler_rate_limiting() {
        let throttler = NotificationThrottler::new(100);
        assert!(throttler.should_allow());
        // Immediate subsequent call should be throttled
        assert!(!throttler.should_allow());
    }
}
