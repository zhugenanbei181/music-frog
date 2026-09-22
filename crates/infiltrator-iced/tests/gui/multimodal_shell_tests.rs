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
fn the_palette_lists_the_shared_catalogue_and_wraps_like_bevy() {
    let (mut state, _) = AppState::new();
    let _ = state.update(chord("k", KeyModifiers::ctrl()));
    assert!(state.shell.command_palette_open);
    let catalogue = state.shell.command_catalogue.clone();
    assert!(catalogue.index_of("nav.dns").is_some());
    assert!(catalogue.index_of("action.toggle_mini_hud").is_some());
    assert!(catalogue.index_of("theme.toggle").is_some());

    // Filtering uses the shared substring rule (plus pinyin on top).
    let _ = state.update(Message::SetCommandQuery("dns".to_owned()));
    let filtered = state.filtered_command_indices();
    let ids: Vec<&str> = filtered
        .iter()
        .filter_map(|index| catalogue.entry(*index))
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(ids, vec!["nav.dns", "action.flush_dns_cache"]);

    // The cursor wraps exactly like the Bevy state machine.
    let _ = state.update(Message::SelectPrevCommand);
    assert_eq!(state.shell.command_selected_index, 1);
    let _ = state.update(Message::SelectNextCommand);
    assert_eq!(state.shell.command_selected_index, 0);
}

#[test]
fn the_palette_executes_shared_targets() {
    let (mut state, _) = AppState::new();
    let _ = state.update(chord("k", KeyModifiers::ctrl()));
    let _ = state.update(Message::ExecuteCommand(
        infiltrator_contract::command_catalogue::CommandTarget::Navigate(
            infiltrator_contract::command_catalogue::ShellPage::Dns,
        ),
    ));
    assert!(!state.shell.command_palette_open);
    assert_eq!(state.shell.current_route, crate::types::app::Route::Dns);

    // A global-chord target re-enters the single shortcut handler.
    let before = state.shell.theme_preference;
    let _ = state.update(Message::ExecuteCommand(
        infiltrator_contract::command_catalogue::CommandTarget::CycleTheme,
    ));
    assert_eq!(state.shell.theme_preference, before.next());

    // The Mini HUD row toggles the same flag the chord does.
    assert!(!state.shell.mini_hud_mode);
    let _ = state.update(Message::ExecuteCommand(
        infiltrator_contract::command_catalogue::CommandTarget::ToggleMiniHud,
    ));
    assert!(state.shell.mini_hud_mode);
}

#[test]
fn the_mini_hud_read_model_comes_from_live_projections() {
    use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};

    let (mut state, _) = AppState::new();
    state.diag.traffic = Some(infiltrator_domain::runtime::TrafficData {
        up: 4 * 1024,
        down: 3 * 1024 * 1024,
    });
    state.runtime.proxy_mode = Some("rule".to_owned());
    state.runtime.runtime_selected_proxy = "HK-01".to_owned();
    state.runtime.system_toggles =
        infiltrator_contract::system_toggle::SystemToggleSnapshot::from_legacy(
            true,
            Some(false),
            7,
        );
    state.runtime.traffic_waveform = TrafficWaveformSnapshot {
        generation: 4,
        revision: 2,
        samples: vec![
            TrafficSample {
                sampled_at_epoch_ms: Some(1),
                upload_bps: 1_024.0,
                download_bps: 4_096.0,
            },
            TrafficSample {
                sampled_at_epoch_ms: Some(2),
                upload_bps: 8_192.0,
                download_bps: 1_024.0,
            },
        ],
    };

    let model = state.mini_hud_read_model();
    assert_eq!(model.up_bytes_per_sec, 4 * 1024);
    assert_eq!(model.down_bytes_per_sec, 3 * 1024 * 1024);
    assert_eq!(model.exit_node, "HK-01");
    assert_eq!(
        model.next_value(infiltrator_contract::system_toggle::SystemToggle::SystemProxy),
        Some(false)
    );
    assert_eq!(
        model.next_value(infiltrator_contract::system_toggle::SystemToggle::Tun),
        Some(true)
    );
    assert!(model.status_line().contains("系统代理: 开"));
    assert!(!model.mode_zh.is_empty());

    // DUAL-15-03: the waveform strip is the shared projection of the live
    // samples — the very same bars the Bevy overlay rasterizes.
    assert_eq!(
        model.waveform,
        infiltrator_contract::mini_hud::MiniHudWaveformStrip::from_snapshot(
            &state.runtime.traffic_waveform
        )
    );
    assert_eq!(model.waveform.up, vec![125, 1_000]);
    assert_eq!(model.waveform.down, vec![500, 125]);

    // A surface without live waveform samples must not fabricate a strip.
    state.runtime.traffic_waveform = TrafficWaveformSnapshot::default();
    assert!(state.mini_hud_read_model().waveform.is_empty());
}

#[test]
fn the_mini_hud_view_renders_the_shared_strip() {
    use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};

    let (mut state, _) = AppState::new();
    state.shell.mini_hud_mode = true;
    state.runtime.traffic_waveform = TrafficWaveformSnapshot {
        generation: 1,
        revision: 2,
        samples: vec![
            TrafficSample {
                sampled_at_epoch_ms: None,
                upload_bps: 512.0,
                download_bps: 1_024.0,
            },
            TrafficSample {
                sampled_at_epoch_ms: None,
                upload_bps: 2_048.0,
                download_bps: 512.0,
            },
        ],
    };
    assert!(!state.mini_hud_read_model().waveform.is_empty());
    {
        let _view = crate::view::mini_hud::mini_hud_view(&state);
    }

    // The same state with no live samples still renders an empty strip.
    state.runtime.traffic_waveform = TrafficWaveformSnapshot::default();
    assert!(state.mini_hud_read_model().waveform.is_empty());
    let _empty_view = crate::view::mini_hud::mini_hud_view(&state);
}

#[test]
fn the_mini_hud_drag_moves_the_persisted_placement() {
    let (mut state, _) = AppState::new();
    state.shell.mini_hud_placement =
        infiltrator_contract::mini_hud::MiniHudPlacement::new(100, 200);
    let _ = state.update(Message::MiniHudDisplayKnown(Some(iced::Size::new(
        1920.0, 1080.0,
    ))));

    // First move anchors on the cursor; the delta moves the placement mirror.
    let _ = state.update(Message::MiniHudMoved { x: 500.0, y: 400.0 });
    assert_eq!(
        state.shell.mini_hud_placement,
        infiltrator_contract::mini_hud::MiniHudPlacement::new(100, 200)
    );
    let _ = state.update(Message::MiniHudMoved { x: 540.0, y: 430.0 });
    assert_eq!(state.shell.mini_hud_placement.x, 140);
    assert_eq!(state.shell.mini_hud_placement.y, 230);

    // Releasing clears the anchor and snapshots the display for the
    // application's clamp/snap pass.
    let _ = state.update(Message::MiniHudDragReleased);
    assert!(state.shell.mini_hud_drag_anchor.is_none());
    assert!(state.shell.mini_hud_display.is_some());
}

#[test]
fn always_on_top_mirrors_the_pin_onto_the_shared_placement() {
    let (mut state, _) = AppState::new();
    assert!(!state.shell.always_on_top);
    let _ = state.update(Message::SetAlwaysOnTop(true));
    assert!(state.shell.always_on_top);
    assert!(state.shell.mini_hud_placement.pinned);
    let _ = state.update(Message::SetAlwaysOnTop(false));
    assert!(!state.shell.mini_hud_placement.pinned);
}

/// DUAL-15-04: the placement update that the desktop host port accepted is
/// drained into the window task exactly once, and only while the host window
/// is live.
#[test]
fn the_placement_update_drains_the_host_window_requests() {
    use infiltrator_ports::mini_hud_window::MiniHudWindowHandle;

    let (mut state, _) = AppState::new();
    let handle = crate::mini_hud_window::IcedMiniHudWindowHandle::host();
    handle.mark_live(false);
    let _ = handle.take_pending();
    let placement = infiltrator_contract::mini_hud::MiniHudPlacement::new(0, 12);

    // A host window that is not live refuses the request: nothing to drain.
    assert!(!handle.apply_placement(placement));
    let _ = state.update(Message::MiniHudPlacementUpdated(Ok(placement)));
    assert!(handle.take_pending().is_empty());

    // A live window accepts it, and the update path consumes it.
    handle.mark_live(true);
    assert!(handle.apply_placement(placement));
    let _ = state.update(Message::MiniHudPlacementUpdated(Ok(placement)));
    assert!(
        handle.take_pending().is_empty(),
        "the accepted request became a window task, not a replayed queue entry"
    );
    assert_eq!(state.shell.mini_hud_placement, placement);
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

/// DUAL-15-14: the Iced token set resolves the shared design contract, so the
/// surface the Bevy widget mirror is checked against cannot itself drift.
#[test]
fn the_iced_tokens_resolve_the_shared_design_contract() {
    use crate::view::theme;
    use infiltrator_contract::design_tokens::{RgbaToken, skin_core};
    use infiltrator_contract::theme::ThemeSkin;

    let tokens_for = |skin: ThemeSkin| match skin {
        ThemeSkin::Dark => &theme::DARK,
        ThemeSkin::Light => &theme::LIGHT,
        ThemeSkin::Forest => &theme::FOREST,
        ThemeSkin::Amoled => &theme::AMOLED,
    };
    let assert_color = |color: iced::Color, token: RgbaToken, field: &str| {
        assert_eq!(
            (color.r, color.g, color.b, color.a),
            (token.r, token.g, token.b, token.a),
            "the Iced token `{field}` drifted from the shared contract"
        );
    };

    for skin in ThemeSkin::ALL {
        let core = skin_core(skin);
        let resolved = tokens_for(skin);
        assert_color(resolved.canvas, core.canvas, "canvas");
        assert_color(resolved.sidebar, core.sidebar, "sidebar");
        assert_color(resolved.card_bg, core.card, "card");
        assert_color(resolved.card_border, core.card_border, "card_border");
        assert_color(resolved.control_bg, core.control_bg, "control_bg");
        assert_color(resolved.text_primary, core.ink, "ink");
        assert_color(resolved.text_secondary, core.ink_dim, "ink_dim");
        assert_color(resolved.accent, core.accent, "accent");
        assert_color(resolved.on_accent, core.on_accent, "on_accent");
        assert_color(resolved.success, core.success, "success");
        assert_color(resolved.warning, core.warning, "warning");
        assert_color(resolved.danger, core.danger, "danger");
    }

    use infiltrator_contract::design_tokens::{radius, space};
    assert_eq!(theme::SP_XS, space::XS);
    assert_eq!(theme::SP_SM, space::SM);
    assert_eq!(theme::SP_MD, space::MD);
    assert_eq!(theme::SP_LG, space::LG);
    assert_eq!(theme::SP_XL, space::XL);
    assert_eq!(theme::SP_XXL, space::XXL);
    assert_eq!(theme::R_CARD, radius::CARD);
    assert_eq!(theme::R_CONTROL, radius::CONTROL);
    // The hairline is a real token, not a page-local 1.0 literal: every Iced
    // border calls `theme::HAIRLINE`, and the guard forbids raw
    // `width: 1.0` borders in the whole Iced shell.
    assert_eq!(
        theme::HAIRLINE,
        infiltrator_contract::design_tokens::metrics::HAIRLINE
    );
}

/// DUAL-15-08: the shell tracks the only window power fact Iced exposes and
/// resolves it through the shared cadence policy.
#[test]
fn the_shell_tracks_window_focus_for_the_shared_cadence() {
    use infiltrator_contract::cadence::RenderCadence;

    let (mut state, _) = AppState::new();
    assert!(state.shell.window_focused, "a live window starts focused");
    assert_eq!(
        RenderCadence::from_focused(state.shell.window_focused),
        RenderCadence::Active
    );

    let _ = state.update(Message::WindowFocusChanged(false));
    assert!(!state.shell.window_focused);
    assert_eq!(
        RenderCadence::from_focused(state.shell.window_focused),
        RenderCadence::Background,
        "the background frame tick drops to the shared 2 FPS rate"
    );
    assert_eq!(RenderCadence::Background.frame_time_ms(), 500);

    let _ = state.update(Message::WindowFocusChanged(true));
    assert!(state.shell.window_focused);
    assert_eq!(
        RenderCadence::from_focused(state.shell.window_focused),
        RenderCadence::Active
    );
}

// ---------------------------------------------------------------------------
// DUAL-15-02: the shared tray-status contract (live rate badge + capability)
// ---------------------------------------------------------------------------

/// A tray controller that records every pushed spec, so the live-badge push
/// path is observable headlessly (no D-Bus, no backend).
struct RecordingTrayController {
    pushed: std::sync::Arc<std::sync::Mutex<Vec<crate::tray::spec::TraySpec>>>,
}

impl crate::tray::spec::TrayController for RecordingTrayController {
    fn update_spec(&self, spec: crate::tray::spec::TraySpec) {
        self.pushed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(spec);
    }

    fn shutdown(&mut self) {}
}

fn live_waveform(
    up: f64,
    down: f64,
) -> infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot {
    use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};
    TrafficWaveformSnapshot {
        generation: 1,
        revision: 7,
        samples: vec![TrafficSample {
            sampled_at_epoch_ms: Some(1_700_000_000_000),
            upload_bps: up,
            download_bps: down,
        }],
    }
}

fn info_rate_label(spec: &crate::tray::spec::TraySpec) -> Option<String> {
    use crate::tray::spec::{TRAY_ACTION_INFO_RATE, TrayMenuItem};
    fn scan(items: &[TrayMenuItem]) -> Option<String> {
        for item in items {
            match item {
                TrayMenuItem::Action { id, label, .. } if *id == TRAY_ACTION_INFO_RATE => {
                    return Some(label.clone());
                }
                TrayMenuItem::Submenu { items, .. } => {
                    if let Some(found) = scan(items) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }
    scan(&spec.menu.items)
}

#[test]
fn the_tray_spec_carries_the_live_rate_badge_from_the_shared_waveform() {
    use infiltrator_contract::tray_status::TrayRateBadge;

    let (mut state, _) = AppState::new();
    state.shell.lang = "zh-CN".to_string();
    state.runtime.traffic_waveform = live_waveform(2_048.0, 512.0);

    let badge = TrayRateBadge::from_waveform(&state.runtime.traffic_waveform).expect("real sample");
    assert_eq!(badge.badge_text(), "↑ 2.0 KB/s · ↓ 512 B/s");

    let spec = state.current_tray_spec();
    assert!(
        spec.tooltip.contains(&badge.badge_text()),
        "tooltip must carry the shared badge text: {}",
        spec.tooltip
    );
    let info_line = info_rate_label(&spec).expect("info submenu carries the rate line");
    assert!(info_line.contains(&badge.badge_text()));
    assert!(info_line.contains("实时速率"));
}

#[test]
fn a_host_without_live_samples_shows_no_rate_badge() {
    let (state, _) = AppState::new();
    assert!(
        state.runtime.traffic_waveform.samples.is_empty(),
        "no live sample is the honest starting state"
    );
    let spec = state.current_tray_spec();
    assert!(!spec.tooltip.contains('↑'));
    assert!(!spec.tooltip.contains('↓'));
    assert!(info_rate_label(&spec).is_none());
}

#[test]
fn the_rate_badge_push_is_deduplicated_and_uses_the_shared_interval() {
    use infiltrator_contract::tray_status::{TRAY_RATE_REFRESH_INTERVAL_MS, TrayRateBadge};
    use std::time::Duration;

    assert_eq!(
        crate::tray::TRAY_RATE_REFRESH_INTERVAL,
        Duration::from_millis(TRAY_RATE_REFRESH_INTERVAL_MS),
        "the Iced throttle must be the shared constant, not a local one"
    );

    let (mut state, _) = AppState::new();
    state.shell.lang = "zh-CN".to_string();
    let pushed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    state.shell.tray_controller = Some(Box::new(RecordingTrayController {
        pushed: std::sync::Arc::clone(&pushed),
    }));

    state.runtime.traffic_waveform = live_waveform(1_000.0, 2_000.0);
    state.refresh_tray_rates();
    assert_eq!(pushed.lock().unwrap().len(), 1);

    // Same display text → no second push, even with the cooldown expired.
    state.shell.tray_refresh_cooldown = None;
    state.refresh_tray_rates();
    assert_eq!(
        pushed.lock().unwrap().len(),
        1,
        "an unchanged badge is quiet"
    );

    // A changed badge inside the shared interval stays throttled...
    state.shell.tray_refresh_cooldown = Some(std::time::Instant::now());
    state.runtime.traffic_waveform = live_waveform(3_000.0, 4_000.0);
    state.refresh_tray_rates();
    assert_eq!(pushed.lock().unwrap().len(), 1, "throttled inside 1000 ms");

    // ...and lands once the interval has elapsed.
    state.shell.tray_refresh_cooldown = None;
    state.refresh_tray_rates();
    assert_eq!(pushed.lock().unwrap().len(), 2);
    let latest = pushed.lock().unwrap()[1].tooltip.clone();
    let badge = TrayRateBadge::from_waveform(&state.runtime.traffic_waveform).expect("sample");
    assert!(latest.contains(&badge.badge_text()));
}

// ---------------------------------------------------------------------------
// DUAL-15-13: frameless window chrome (shared contract + real host ops)
// ---------------------------------------------------------------------------

#[test]
fn the_frameless_window_settings_consume_the_shared_chrome_contract() {
    use infiltrator_contract::window_chrome::WindowChrome;

    let chrome = WindowChrome::FRAMELESS;
    let settings = crate::window_chrome::window_settings((1180.0, 780.0), (420.0, 560.0));
    assert!(!settings.decorations, "the host runs frameless");
    assert_eq!(settings.decorations, chrome.os_decorations());
    assert!(chrome.needs_custom_controls());
    assert_eq!(
        crate::view::chrome::strip_height_px(),
        chrome.chrome_height_px() as f32
    );

    let support = crate::window_chrome::support();
    assert!(support.is_hosted());
    assert!(support.drag(), "the press path goes to the OS drag");
    assert!(support.maximize());
    assert_eq!(support.unsupported_reason(), None);
}

#[test]
fn every_chrome_message_maps_to_one_real_host_window_op() {
    use crate::window_chrome::ChromeRequest;

    assert_eq!(
        ChromeRequest::from_message(&Message::WindowChromeDragRequested),
        Some(ChromeRequest::Drag)
    );
    assert_eq!(
        ChromeRequest::from_message(&Message::WindowChromeToggleMaximize),
        Some(ChromeRequest::ToggleMaximize)
    );
    assert_eq!(
        ChromeRequest::from_message(&Message::WindowChromeMinimize),
        Some(ChromeRequest::Minimize)
    );
    assert_eq!(
        ChromeRequest::from_message(&Message::WindowChromeClose),
        Some(ChromeRequest::Close)
    );
    assert_eq!(
        ChromeRequest::from_message(&Message::ToggleMiniHudMode),
        None
    );
}

#[test]
fn the_chrome_strip_mounts_above_the_shell_and_never_fabricates_a_window() {
    let (mut state, _) = AppState::new();
    state.shell.lang = "zh-CN".to_string();

    // The strip is part of the mounted shell view (frameless hosts have no OS
    // title bar to fall back on).
    state.shell.mini_hud_mode = false;
    {
        let _shell = state.view();
    }

    // No resolved window id yet: the press is a no-op, never a fake handle.
    assert!(state.shell.window_id.is_none());
    let _ = state.update(Message::WindowChromeDragRequested);
    let _ = state.update(Message::WindowChromeMinimize);
    let _ = state.update(Message::WindowChromeToggleMaximize);

    // With a resolved window the update path accepts the requests.
    let _ = state.update(Message::WindowIdResolved(Some(iced::window::Id::unique())));
    assert!(state.shell.window_id.is_some());
    let _ = state.update(Message::WindowChromeDragRequested);
    let _ = state.update(Message::WindowChromeToggleMaximize);
}

// ---------------------------------------------------------------------------
// DUAL-15-10: the shared accessibility-semantics grammar
// ---------------------------------------------------------------------------

#[test]
fn every_shared_semantic_node_resolves_a_localized_label_and_role() {
    use infiltrator_contract::a11y::ShellA11yNode;

    let (mut state, _) = AppState::new();
    state.shell.lang = "zh-CN".to_string();
    for node in ShellA11yNode::ALL {
        let label = state.a11y_label(node);
        assert!(!label.is_empty(), "{node:?} must resolve a label");
        assert_ne!(label, node.label_key(), "{node:?} label key must resolve");
        assert_eq!(state.a11y_role(node), node.role());
    }

    state.shell.lang = "en-US".to_string();
    for node in ShellA11yNode::ALL {
        let label = state.a11y_label(node);
        assert_ne!(label, node.label_key(), "{node:?} must resolve in en-US");
    }
    assert_eq!(
        state.a11y_label(ShellA11yNode::GlobalStatusDot),
        "Kernel status"
    );
}

#[test]
fn the_shell_carries_the_shared_labels_as_visible_tooltips() {
    use infiltrator_contract::a11y::ShellA11yNode;

    let (mut state, _) = AppState::new();
    state.shell.lang = "zh-CN".to_string();

    // The full shell view (chrome strip, sidebar status dot, live rate
    // readout) and the Mini HUD view both build with the shared labels.
    {
        let _shell = state.view();
    }
    state.shell.mini_hud_mode = true;
    {
        let _hud = state.view();
    }
    assert_eq!(
        state.a11y_label(ShellA11yNode::TrafficReadout),
        "上下行实时速率"
    );
    assert_eq!(state.a11y_label(ShellA11yNode::MiniHudCard), "网速悬浮窗");
}
