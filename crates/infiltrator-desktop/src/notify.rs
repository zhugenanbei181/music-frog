use std::process::Command;
use std::io::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct SystemNotification {
    pub title: String,
    pub body: String,
    pub level: NotificationLevel,
    pub icon: Option<String>,
}

pub struct SystemNotifier;

impl Default for SystemNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemNotifier {
    pub fn new() -> Self {
        Self
    }

    #[cfg(target_os = "linux")]
    pub fn build_command(&self, notification: &SystemNotification) -> Command {
        let urgency = match notification.level {
            NotificationLevel::Info => "low",
            NotificationLevel::Warning => "normal",
            NotificationLevel::Error => "critical",
        };

        let mut cmd = Command::new("notify-send");
        cmd.arg(&notification.title)
           .arg(&notification.body)
           .arg("-u")
           .arg(urgency);

        if let Some(ref icon) = notification.icon {
            cmd.arg("-i").arg(icon);
        }
        
        cmd
    }

    #[cfg(target_os = "macos")]
    pub fn build_command(&self, notification: &SystemNotification) -> Command {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            notification.body.replace("\"", "\\\""),
            notification.title.replace("\"", "\\\"")
        );

        let mut cmd = Command::new("osascript");
        cmd.arg("-e").arg(&script);
        cmd
    }

    #[cfg(target_os = "windows")]
    pub fn build_command(&self, notification: &SystemNotification) -> Command {
        // Fallback using powershell
        let script = format!(
            "[reflection.assembly]::loadwithpartialname('System.Windows.Forms'); \
            [System.Windows.Forms.MessageBox]::Show('{}', '{}')",
            notification.body.replace("'", "''"),
            notification.title.replace("'", "''")
        );

        let mut cmd = Command::new("powershell");
        cmd.arg("-Command").arg(&script);
        cmd
    }

    pub fn send(&self, notification: &SystemNotification) -> Result<()> {
        let mut cmd = self.build_command(notification);
        // Graceful degradation: never panics, ignore missing daemon
        let _ = cmd.spawn();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_struct() {
        let n = SystemNotification {
            title: "Test".to_string(),
            body: "Body".to_string(),
            level: NotificationLevel::Info,
            icon: None,
        };
        assert_eq!(n.title, "Test");
        assert_eq!(n.body, "Body");
        assert_eq!(n.level, NotificationLevel::Info);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_command_generation() {
        let notifier = SystemNotifier::new();
        let n = SystemNotification {
            title: "Hello".to_string(),
            body: "World".to_string(),
            level: NotificationLevel::Error,
            icon: Some("icon.png".to_string()),
        };
        let cmd = notifier.build_command(&n);
        let prog = cmd.get_program();
        assert_eq!(prog, "notify-send");
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args.len(), 6);
    }
}
