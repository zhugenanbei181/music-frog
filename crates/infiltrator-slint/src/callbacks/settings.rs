//! Slint settings callbacks routed to the shared product owner.

use super::super::*;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub(super) fn register(
    window: &AppWindow,
    requests: Sender<WorkerRequest>,
    latest: Arc<Mutex<Option<SurfaceSnapshot>>>,
) {
    let weak = window.as_weak();
    let tx = requests.clone();
    let latest_for_user_agent = Arc::clone(&latest);
    let weak_for_user_agent = weak.clone();
    window.on_save_user_agent(move |user_agent| {
        let action = require_snapshot(&latest_for_user_agent).and_then(|snapshot| {
            let profile = snapshot
                .pages
                .profiles
                .data
                .as_ref()
                .and_then(|page| page.profiles.iter().find(|profile| profile.is_active))
                .ok_or_else(|| {
                    Failure::new(ErrorCode::NotReady, "No active profile was observed", true)
                })?;
            Ok(SlintAction::Command(
                CommandIntent::UpdateSubscriptionFetchSettings {
                    profile_id: profile.id.clone(),
                    user_agent: Some(user_agent.to_string()),
                    insecure_skip_verify: profile.insecure_skip_verify,
                },
            ))
        });
        queue_action(&tx, &weak_for_user_agent, action);
    });

    let weak_for_refresh_on_start = weak.clone();
    window.on_set_refresh_profile_on_start(move |enabled| {
        let _ = weak_for_refresh_on_start
            .clone()
            .upgrade_in_event_loop(move |window| {
                window.set_refresh_current_profile_on_start(false);
                if enabled {
                    window.set_notice(
                        "The shared settings contract has no startup profile-refresh preference"
                            .into(),
                    );
                }
            });
    });
    let tx = requests.clone();
    let weak_for_autostart = weak.clone();
    window.on_set_auto_start(move |enabled| {
        if cfg!(target_os = "android") {
            let weak = weak_for_autostart.clone();
            let _ = weak.clone().upgrade_in_event_loop(|window| {
                window.set_auto_start(false);
            });
            show_unsupported(
                &weak_for_autostart,
                "Android startup behavior is controlled by the Android host",
            );
            return;
        }
        queue_action(
            &tx,
            &weak_for_autostart,
            Ok(SlintAction::SetAutostart(enabled)),
        );
    });

    let tx = requests.clone();
    let weak_for_theme = weak.clone();
    window.on_set_theme_mode(move |mode| {
        let mode = match mode.as_str() {
            "system" | "light" | "dark" | "forest" | "amoled" => mode.to_string(),
            _ => {
                show_failure(
                    &weak_for_theme,
                    Failure::new(ErrorCode::InvalidInput, "Unknown theme mode", false),
                );
                return;
            }
        };
        queue_intent(
            &tx,
            &weak_for_theme,
            CommandIntent::UpdateSetting {
                key: "theme".to_owned(),
                value: mode,
            },
        );
    });

    let weak_for_invert = weak.clone();
    window.on_set_invert_theme_foreground(move |enabled| {
        let _ = weak_for_invert
            .clone()
            .upgrade_in_event_loop(move |window| window.set_invert_theme_foreground(enabled));
    });
    let weak_for_color_preview = weak.clone();
    window.on_preview_theme_color(move |hue, saturation, value| {
        let hex = hsv_hex(hue as f64, saturation as f64, value as f64);
        apply_theme_color(&weak_for_color_preview, &hex);
    });
    let weak_for_color_cancel = weak.clone();
    window.on_cancel_theme_color_preview(move || {
        let _ = weak_for_color_cancel
            .clone()
            .upgrade_in_event_loop(|window| {
                let hex = window.get_theme_color_code().to_string();
                if let Some(color) = parse_color(&hex) {
                    window.set_theme_color(color);
                }
                window.set_theme_color_draft(hex.clone().into());
                window.set_theme_color_code_draft(hex.into());
            });
    });
    let weak_for_color_apply = weak.clone();
    window.on_set_theme_color(move |hex| {
        let hex = hex.to_string();
        if parse_color(&hex).is_some() {
            apply_theme_color(&weak_for_color_apply, &hex);
        }
        let weak = weak_for_color_apply.clone();
        let valid = parse_color(&hex).is_some();
        let _ = weak.clone().upgrade_in_event_loop(move |window| {
            window.set_theme_color_saving(false);
            window.set_theme_color_error(if valid {
                "Custom accent color is previewed, but the shared settings owner does not persist it yet".into()
            } else {
                "Enter a six-digit hexadecimal color such as #016888".into()
            });
        });
        if valid {
            show_unsupported(&weak_for_color_apply, "Custom accent color is a surface-local preference and is not persisted by the shared settings owner yet");
        }
    });

    let tx = requests.clone();
    let weak_for_language = weak.clone();
    window.on_set_application_language(move |language| {
        match LanguagePreference::parse(language.as_str()) {
            Ok(preference) => queue_intent(
                &tx,
                &weak_for_language,
                CommandIntent::SetLanguage { preference },
            ),
            Err(failure) => show_failure(&weak_for_language, failure),
        }
    });
    let weak_for_scale = weak.clone();
    window.on_set_page_scale(move |percent| {
        let _ = weak_for_scale
            .clone()
            .upgrade_in_event_loop(move |window| window.set_page_scale_percent(percent));
    });
    let weak_for_hide_bar = weak.clone();
    window.on_set_hide_bottom_bar_on_scroll(move |enabled| {
        let _ = weak_for_hide_bar
            .clone()
            .upgrade_in_event_loop(move |window| window.set_hide_bottom_bar_on_scroll(enabled));
    });
    let weak_for_predictive_back = weak.clone();
    window.on_set_predictive_back_enabled(move |enabled| {
        let _ = weak_for_predictive_back
            .clone()
            .upgrade_in_event_loop(move |window| {
                let supported = window.get_predictive_back_supported();
                window.set_predictive_back_enabled(enabled && supported);
                if enabled && !supported {
                    window.set_app_settings_message(
                        "Predictive back progress needs the Android 14 host gesture adapter".into(),
                    );
                }
            });
    });
    let weak_for_background_card = weak.clone();
    window.on_set_hide_background_card(move |_| {
        let weak = weak_for_background_card.clone();
        let _ = weak
            .clone()
            .upgrade_in_event_loop(|window| window.set_hide_background_card(false));
        show_unsupported(
            &weak_for_background_card,
            "Hiding the Android recents card needs a window security adapter",
        );
    });
    let weak_for_traffic_notification = weak.clone();
    window.on_set_show_traffic_notification(move |enabled| {
        let _ = enabled;
        show_unsupported(
            &weak_for_traffic_notification,
            "Traffic notifications need an Android notification-service adapter",
        );
    });
    let weak_for_release_ui = weak.clone();
    window.on_set_release_ui_when_background(move |enabled| {
        let _ = enabled;
        show_unsupported(
            &weak_for_release_ui,
            "Background surface release needs an Android lifecycle adapter",
        );
    });
    let weak_for_battery = weak.clone();
    window.on_request_ignore_battery_optimization(move || {
        show_unsupported(
            &weak_for_battery,
            "Battery optimization exemption needs an Android activity result adapter",
        );
    });
    let weak_for_back_preview = weak.clone();
    window.on_cache_back_preview(move || {
        show_unsupported(
            &weak_for_back_preview,
            "Predictive-back image capture is not supplied by the NativeActivity host",
        );
    });
    let weak_for_back_commit = weak.clone();
    window.on_commit_back_preview(move || {
        show_unsupported(
            &weak_for_back_commit,
            "Predictive-back image capture is not supplied by the NativeActivity host",
        );
    });
}
