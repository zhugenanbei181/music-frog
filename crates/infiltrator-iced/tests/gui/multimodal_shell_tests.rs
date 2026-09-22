//! Group 15 multimodal-shell tests (DUAL-15-06/09/12) on the real `AppState`:
//! appearance preference resolution, the shared shortcut registry dispatch,
//! and the toast dedup/capacity policy.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use infiltrator_contract::shortcuts::{KeyModifiers, ShortcutAction, ShortcutChord};
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};

fn chord(key: &str, modifiers: KeyModifiers) -> Message {
    Message::KeyboardChord {
        key: key.to_string(),
        modifiers,
    }
}

#[test]
fn the_shell_follows_the_os_appearance_while_the_preference_is_system() {
    let (mut state, _) = AppState::new();
    assert_eq!(state.shell.theme_preference, ThemePreference::System);

    let _ = state.update(Message::SystemThemeChanged(false));
    assert_eq!(state.shell.theme, iced::Theme::Light);
    let _ = state.update(Message::SystemThemeChanged(true));
    assert_eq!(state.shell.theme, iced::Theme::Dark);

    // A pinned skin ignores later OS signals.
    let _ = state.update(Message::SetTheme("forest".to_string()));
    assert_eq!(
        state.shell.theme_preference,
        ThemePreference::Fixed(ThemeSkin::Forest)
    );
    assert!(crate::view::theme::is_forest(&state.shell.theme));
    let _ = state.update(Message::SystemThemeChanged(false));
    assert!(crate::view::theme::is_forest(&state.shell.theme));
}

#[test]
fn a_system_preference_survives_the_settings_round_trip() {
    let (mut state, _) = AppState::new();
    let settings = infiltrator_domain::settings::AppSettings {
        theme: "system".to_string(),
        ..Default::default()
    };
    let _ = state.update(Message::SettingsLoaded(Ok(settings)));
    assert_eq!(state.shell.theme_preference, ThemePreference::System);
    assert_eq!(
        state.shell.theme_preference.as_setting(),
        "system",
        "the save path persists the preference, not the resolved skin"
    );

    let pinned = infiltrator_domain::settings::AppSettings {
        theme: "EyeForest".to_string(),
        ..Default::default()
    };
    let _ = state.update(Message::SettingsLoaded(Ok(pinned)));
    assert_eq!(
        state.shell.theme_preference,
        ThemePreference::Fixed(ThemeSkin::Forest)
    );
}

#[test]
fn the_theme_preference_cycle_visits_every_shared_skin() {
    let (mut state, _) = AppState::new();
    let mut seen = Vec::new();
    for _ in 0..ThemeSkin::ALL.len() {
        let _ = state.update(Message::ToggleTheme);
        seen.push(state.shell.theme_preference);
    }
    assert_eq!(
        seen,
        vec![
            ThemePreference::Fixed(ThemeSkin::Dark),
            ThemePreference::Fixed(ThemeSkin::Light),
            ThemePreference::Fixed(ThemeSkin::Forest),
            ThemePreference::Fixed(ThemeSkin::Amoled),
        ]
    );
    // Amoled is a real, distinguishable painting (not dark by another name).
    assert!(crate::view::theme::is_amoled(&state.shell.theme));
    assert_ne!(state.shell.theme, iced::Theme::Dark);
}

#[test]
fn unknown_theme_settings_honestly_resolve_to_system() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::SetTheme("banana".to_string()));
    assert_eq!(state.shell.theme_preference, ThemePreference::System);
}

#[test]
fn the_keyboard_chord_dispatches_through_the_shared_registry() {
    let (mut state, _) = AppState::new();
    assert!(!state.shell.command_palette_open);

    // Ctrl+K opens the palette.
    let _ = state.update(chord("k", KeyModifiers::ctrl()));
    assert!(state.shell.command_palette_open);

    // An unbound chord is inert.
    let _ = state.update(chord("q", KeyModifiers::ctrl()));
    assert!(state.shell.command_palette_open);

    // Ctrl+Alt+M toggles the Mini HUD mode.
    assert!(!state.shell.mini_hud_mode);
    let _ = state.update(chord("M", KeyModifiers::ctrl_alt()));
    assert!(state.shell.mini_hud_mode);

    // Ctrl+Alt+D advances the appearance preference.
    let before = state.shell.theme_preference;
    let _ = state.update(chord("D", KeyModifiers::ctrl_alt()));
    assert_eq!(state.shell.theme_preference, before.next());

    // Disabling a binding stops it from dispatching.
    let _ = state.update(Message::ToggleHotkeyEnabled(ShortcutAction::ToggleMiniHud));
    assert!(
        !state
            .shell
            .shortcut_registry
            .is_active(ShortcutAction::ToggleMiniHud)
    );
    let mini_hud_before = state.shell.mini_hud_mode;
    let _ = state.update(chord("M", KeyModifiers::ctrl_alt()));
    assert_eq!(state.shell.mini_hud_mode, mini_hud_before);
}

#[test]
fn the_open_palette_owns_the_arrow_keys() {
    let (mut state, _) = AppState::new();
    let _ = state.update(chord("k", KeyModifiers::ctrl()));
    assert!(state.shell.command_palette_open);
    assert_eq!(state.shell.command_selected_index, 0);

    let _ = state.update(chord("ArrowDown", KeyModifiers::default()));
    assert_eq!(state.shell.command_selected_index, 1);
    let _ = state.update(chord("ArrowDown", KeyModifiers::default()));
    assert_eq!(state.shell.command_selected_index, 2);
    let _ = state.update(chord("ArrowUp", KeyModifiers::default()));
    assert_eq!(state.shell.command_selected_index, 1);

    // Closed palette: the arrows are inert again.
    let _ = state.update(Message::CloseCommandPalette);
    let _ = state.update(chord("ArrowDown", KeyModifiers::default()));
    assert_eq!(state.shell.command_selected_index, 0);
}

#[test]
fn capture_records_the_conflict_without_touching_the_binding() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::BeginHotkeyCapture(ShortcutAction::ToggleMiniHud));
    let before = state.shell.shortcut_registry.clone();
    // Ctrl+Alt+T belongs to the TUN toggle.
    let _ = state.update(chord("T", KeyModifiers::ctrl_alt()));
    assert_eq!(state.shell.hotkey_capture, None);
    assert_eq!(state.shell.shortcut_registry, before);
    let (conflict, status) = state.shell.toasts.last().expect("conflict toast");
    assert_eq!(status, &ToastStatus::Warning);
    assert!(
        conflict.contains("Ctrl+Alt+T"),
        "toast names the chord: {conflict}"
    );
}

#[test]
fn capture_accepts_a_free_chord_and_normalizes_the_key() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::BeginHotkeyCapture(ShortcutAction::CycleTheme));
    let _ = state.update(chord("j", KeyModifiers::ctrl_alt()));
    assert_eq!(
        state
            .shell
            .shortcut_registry
            .get(ShortcutAction::CycleTheme)
            .expect("bound")
            .chord,
        ShortcutChord::ctrl_alt_key("J")
    );
}

#[test]
fn identical_toasts_are_coalesced_and_the_stack_is_capped() {
    let (mut state, _) = AppState::new();
    let units = state.push_toast("controller unreachable".to_string(), ToastStatus::Error);
    assert_eq!(units.units(), 1, "the auto-dismiss task is armed");
    let _ = state.push_toast("controller unreachable".to_string(), ToastStatus::Error);
    assert_eq!(state.shell.toasts.len(), 1, "duplicate coalesced");

    for index in 0..5 {
        let _ = state.push_toast(format!("distinct message {index}"), ToastStatus::Info);
    }
    assert_eq!(
        state.shell.toasts.len(),
        state.shell.toast_gate.policy().max_visible
    );
    assert_eq!(
        state
            .shell
            .toasts
            .last()
            .map(|(content, _)| content.clone()),
        Some("distinct message 4".to_string())
    );
    assert_eq!(state.shell.toast_ids.len(), state.shell.toasts.len());
}
