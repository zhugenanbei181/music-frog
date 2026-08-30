use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Represents an action dispatched from the system tray menu.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TrayMenuAction {
    ToggleProxy(bool),
    SwitchMode(String),
    SelectProfile(String),
    OpenMainWindow,
    ShowDiagnostics,
    QuitApp,
}

/// A snapshot of the application state relevant to the system tray.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TrayStateSnapshot {
    pub is_proxy_enabled: bool,
    pub current_mode: String,
    pub active_profile: String,
    pub window_visible: bool,
    pub speed_display: String,
}

/// Dispatches events from the system tray to the application core.
#[derive(Clone, Debug)]
pub struct TrayEventDispatcher {
    sender: mpsc::Sender<TrayMenuAction>,
}

impl TrayEventDispatcher {
    /// Creates a new dispatcher and its receiving channel with the given capacity.
    pub fn new(capacity: usize) -> (Self, mpsc::Receiver<TrayMenuAction>) {
        let (sender, receiver) = mpsc::channel(capacity);
        (Self { sender }, receiver)
    }

    /// Dispatches an action from the tray.
    /// This uses `try_send` to ensure non-blocking execution suitable for UI threads.
    pub fn dispatch(&self, action: TrayMenuAction) -> Result<()> {
        self.sender
            .try_send(action)
            .context("Failed to dispatch tray event: channel may be full or closed")
    }

    /// Formats the tray tooltip based on the current state snapshot.
    pub fn format_tooltip(snapshot: &TrayStateSnapshot) -> String {
        let status = if snapshot.is_proxy_enabled {
            "Enabled"
        } else {
            "Disabled"
        };
        format!(
            "Status: {}\nMode: {}\nProfile: {}\nSpeed: {}",
            status, snapshot.current_mode, snapshot.active_profile, snapshot.speed_display
        )
    }

    /// Formats the label for a menu item given the action and whether it's active.
    pub fn format_menu_item_label(action: &TrayMenuAction, active: bool) -> String {
        let check = if active { " [x]" } else { "" };
        match action {
            TrayMenuAction::ToggleProxy(enable) => {
                let verb = if *enable { "Enable" } else { "Disable" };
                format!("{} Proxy", verb)
            }
            TrayMenuAction::SwitchMode(mode) => format!("Mode: {}{}", mode, check),
            TrayMenuAction::SelectProfile(profile) => format!("Profile: {}{}", profile, check),
            TrayMenuAction::OpenMainWindow => "Open Dashboard".to_string(),
            TrayMenuAction::ShowDiagnostics => "Diagnostics".to_string(),
            TrayMenuAction::QuitApp => "Quit".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_dispatch_and_receive() {
        let (dispatcher, mut receiver) = TrayEventDispatcher::new(10);
        let action = TrayMenuAction::ToggleProxy(true);
        assert!(dispatcher.dispatch(action.clone()).is_ok());

        let received = receiver.blocking_recv().expect("Failed to receive action");
        assert_eq!(received, action);
    }

    #[test]
    fn test_channel_overflow() {
        let (dispatcher, mut receiver) = TrayEventDispatcher::new(1);
        // First dispatch should succeed
        assert!(dispatcher.dispatch(TrayMenuAction::OpenMainWindow).is_ok());
        
        // Channel is full, second dispatch should error (non-blocking)
        let res = dispatcher.dispatch(TrayMenuAction::QuitApp);
        assert!(res.is_err());
        
        // Ensure the channel still works after being full if space frees up
        receiver.try_recv().unwrap();
        assert!(dispatcher.dispatch(TrayMenuAction::ShowDiagnostics).is_ok());
    }

    #[test]
    fn test_format_tooltip() {
        let snapshot = TrayStateSnapshot {
            is_proxy_enabled: true,
            current_mode: "Global".to_string(),
            active_profile: "US-West".to_string(),
            window_visible: false,
            speed_display: "1.2 MB/s".to_string(),
        };
        let tooltip = TrayEventDispatcher::format_tooltip(&snapshot);
        assert_eq!(
            tooltip,
            "Status: Enabled\nMode: Global\nProfile: US-West\nSpeed: 1.2 MB/s"
        );
        
        let snapshot_disabled = TrayStateSnapshot {
            is_proxy_enabled: false,
            ..snapshot
        };
        let tooltip2 = TrayEventDispatcher::format_tooltip(&snapshot_disabled);
        assert!(tooltip2.starts_with("Status: Disabled"));
    }

    #[test]
    fn test_format_menu_item_label() {
        assert_eq!(
            TrayEventDispatcher::format_menu_item_label(&TrayMenuAction::ToggleProxy(true), false),
            "Enable Proxy"
        );
        assert_eq!(
            TrayEventDispatcher::format_menu_item_label(&TrayMenuAction::ToggleProxy(false), false),
            "Disable Proxy"
        );
        assert_eq!(
            TrayEventDispatcher::format_menu_item_label(
                &TrayMenuAction::SwitchMode("PAC".to_string()),
                true
            ),
            "Mode: PAC [x]"
        );
        assert_eq!(
            TrayEventDispatcher::format_menu_item_label(
                &TrayMenuAction::SelectProfile("Default".to_string()),
                false
            ),
            "Profile: Default"
        );
        assert_eq!(
            TrayEventDispatcher::format_menu_item_label(&TrayMenuAction::OpenMainWindow, false),
            "Open Dashboard"
        );
        assert_eq!(
            TrayEventDispatcher::format_menu_item_label(&TrayMenuAction::ShowDiagnostics, false),
            "Diagnostics"
        );
        assert_eq!(
            TrayEventDispatcher::format_menu_item_label(&TrayMenuAction::QuitApp, false),
            "Quit"
        );
    }

    #[test]
    fn test_multithread_concurrency() {
        let (dispatcher, mut receiver) = TrayEventDispatcher::new(100);
        let mut handles = vec![];

        for i in 0..10 {
            let d = dispatcher.clone();
            handles.push(thread::spawn(move || {
                let _ = d.dispatch(TrayMenuAction::SwitchMode(format!("Mode-{}", i)));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let mut count = 0;
        while receiver.try_recv().is_ok() {
            count += 1;
        }
        assert_eq!(count, 10, "Should have received 10 messages from threads");
    }
}
