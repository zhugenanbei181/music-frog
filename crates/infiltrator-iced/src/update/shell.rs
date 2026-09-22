//! Group 15 multimodal-shell update handlers: shared appearance preference,
//! global shortcut registry capture/dispatch, and the toast ingestion point.
//!
//! Split out of `update/ui.rs` so the shell-domain handlers stay together and
//! the UI dispatcher keeps its line budget.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::shortcuts::ShortcutAction;
use infiltrator_shared::locales::{Lang, Localizer};

impl AppState {
    /// Handlers for the shared appearance/shortcut/notification messages.
    /// Returns `None` when the message is not part of this domain, so the
    /// caller can keep its own arm chain exhaustive.
    pub(crate) fn update_shell(&mut self, message: Message) -> Option<Task<Message>> {
        match message {
            Message::ToggleTheme | Message::CycleThemePreference => {
                let next = self.shell.theme_preference.next();
                self.shell.apply_theme_preference(next);
                Some(Task::none())
            }
            Message::SetTheme(theme_name) => {
                let preference =
                    infiltrator_contract::theme::ThemePreference::from_setting(&theme_name);
                self.shell.apply_theme_preference(preference);
                Some(Task::none())
            }
            Message::SystemThemeChanged(prefers_dark) => {
                self.shell.system_prefers_dark = prefers_dark;
                if self.shell.theme_preference.follows_system() {
                    self.shell
                        .apply_theme_preference(self.shell.theme_preference);
                }
                Some(Task::none())
            }
            Message::BeginHotkeyCapture(action) => {
                self.shell.hotkey_capture = Some(action);
                Some(Task::none())
            }
            Message::CancelHotkeyCapture => {
                self.shell.hotkey_capture = None;
                Some(Task::none())
            }
            Message::KeyboardChord { key, modifiers } => {
                Some(self.handle_keyboard_chord(key, modifiers))
            }
            Message::ToggleHotkeyEnabled(action) => {
                let enabled = !self.shell.shortcut_registry.is_active(action);
                // Optimistic in-memory toggle; the facade write is the
                // authoritative follow-up (and reports failures as a toast).
                self.shell.shortcut_registry.set_enabled(action, enabled);
                Some(Task::perform(
                    crate::shortcuts_store::set_enabled(action, enabled),
                    |result| Message::ShortcutsUpdated(result.map_err(|error| error.to_string())),
                ))
            }
            Message::ResetHotkey(action) => Some(Task::perform(
                crate::shortcuts_store::reset_action(action),
                |result| Message::ShortcutsUpdated(result.map_err(|error| error.to_string())),
            )),
            Message::ShortcutsUpdated(result) => Some(match result {
                Ok(registry) => {
                    self.shell.shortcut_registry = registry;
                    Task::none()
                }
                Err(error) => Task::done(Message::ShowToast(error, ToastStatus::Error)),
            }),
            Message::ShowToast(content, status) => Some(self.push_toast(content, status)),
            Message::RemoveToast(id) => {
                if let Some(position) = self.shell.toast_ids.iter().position(|entry| *entry == id) {
                    self.shell.toast_ids.remove(position);
                    self.shell.toasts.remove(position);
                }
                Some(Task::none())
            }
            _ => None,
        }
    }

    /// The single toast ingestion point: redact (CORE-001), apply the shared
    /// dedup/capacity policy, then arm the auto-dismiss task.
    pub fn push_toast(&mut self, content: String, status: ToastStatus) -> Task<Message> {
        // Toast text originates from raw error chains (subscription updates,
        // transport failures) that can embed access tokens; redact here
        // before anything reaches the screen.
        let content = crate::utils::sanitize_ui_text(&content);
        let severity = match status {
            ToastStatus::Info => infiltrator_contract::toast::ToastSeverity::Info,
            ToastStatus::Success => infiltrator_contract::toast::ToastSeverity::Success,
            ToastStatus::Warning => infiltrator_contract::toast::ToastSeverity::Warning,
            ToastStatus::Error => infiltrator_contract::toast::ToastSeverity::Error,
        };
        let now_ms = self.shell.toast_epoch.elapsed().as_millis() as u64;
        if self.shell.toast_gate.admit(severity, &content, now_ms)
            == infiltrator_contract::toast::ToastAdmission::Coalesced
        {
            return Task::none();
        }

        let id = self.shell.next_toast_id;
        self.shell.next_toast_id = self.shell.next_toast_id.wrapping_add(1);
        self.shell.toasts.push((content, status));
        self.shell.toast_ids.push(id);
        let max_visible = self.shell.toast_gate.policy().max_visible;
        while self.shell.toasts.len() > max_visible {
            self.shell.toasts.remove(0);
            self.shell.toast_ids.remove(0);
        }

        Task::perform(
            async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                id
            },
            Message::RemoveToast,
        )
    }

    /// Resolve one raw key press against the shared shortcut registry, or
    /// capture it when a rebind is in flight.
    fn handle_keyboard_chord(
        &mut self,
        key: String,
        modifiers: infiltrator_contract::shortcuts::KeyModifiers,
    ) -> Task<Message> {
        let escape = key == "Escape";
        if let Some(action) = self.shell.hotkey_capture {
            if escape {
                self.shell.hotkey_capture = None;
                return Task::none();
            }
            let chord = infiltrator_contract::shortcuts::ShortcutChord::new(key, modifiers);
            // Immediate feedback against the live registry (the same rule the
            // application layer enforces), then persist through the shared
            // shortcut facade.
            if let Some(conflict) = self.shell.shortcut_registry.find_conflict(action, &chord) {
                let message = format!(
                    "{}: {}",
                    Localizer::tr(&Lang(&self.shell.lang), "hotkey_conflict"),
                    conflict.chord.display_string(false)
                );
                self.shell.hotkey_capture = None;
                return self.push_toast(message, ToastStatus::Warning);
            }
            self.shell.hotkey_capture = None;
            // Optimistic in-memory rebind (already conflict-checked above) so
            // the settings card reflects the capture immediately; the facade
            // returns the authoritative registry when the write completes.
            let _ = self
                .shell
                .shortcut_registry
                .bind_or_replace(action, chord.clone());
            return Task::perform(crate::shortcuts_store::capture(action, chord), |result| {
                Message::ShortcutsUpdated(result.map_err(|error| error.to_string()))
            });
        }
        if escape {
            return Task::done(Message::CloseCommandPalette);
        }
        // While the palette is open its list owns the arrow keys (the footer
        // hint ↑↓ is a keyboard contract, not a decoration).
        if self.shell.command_palette_open {
            match key.as_str() {
                "ArrowDown" => return self.update_ui(Message::SelectNextCommand),
                "ArrowUp" => return self.update_ui(Message::SelectPrevCommand),
                _ => {}
            }
        }
        let chord = infiltrator_contract::shortcuts::ShortcutChord::new(key, modifiers);
        let Some(action) = self.shell.shortcut_registry.resolve(&chord) else {
            return Task::none();
        };
        // Dispatch synchronously into the same handlers the UI uses; the
        // shortcut table is the only routing decision made here.
        self.on_shell_shortcut(action)
    }

    /// Route one bound shortcut action to its product handler. Shared with the
    /// command palette so a catalogue row and its global chord cannot diverge.
    pub(crate) fn on_shell_shortcut(&mut self, action: ShortcutAction) -> Task<Message> {
        match action {
            ShortcutAction::OpenCommandPalette => self.update_ui(Message::ToggleCommandPalette),
            ShortcutAction::ToggleSystemProxy => {
                let enabled = !self.runtime.system_toggles.system_proxy.is_enabled();
                self.update_ui(Message::SetSystemProxy(enabled))
            }
            ShortcutAction::ToggleTun => {
                let enabled = !self.runtime.system_toggles.tun.is_enabled();
                self.update_ui(Message::SetTunEnabled(enabled))
            }
            ShortcutAction::ToggleMiniHud => self.update_ui(Message::ToggleMiniHudMode),
            ShortcutAction::CycleTheme => self
                .update_shell(Message::CycleThemePreference)
                .unwrap_or_else(Task::none),
        }
    }
}
