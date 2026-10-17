//! Slint profiles callbacks routed to the shared product owner.

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
    let latest_for_import = Arc::clone(&latest);
    let weak_for_import = weak.clone();
    window.on_add_url_profile(move |name, url, age_key| {
        let name = name.to_string();
        let url = url.to_string();
        let private_key = if !age_key.is_empty() {
            match AgePrivateKey::new(age_key.to_string()) {
                Ok(key) => Some(key),
                Err(error) => {
                    show_failure(
                        &weak_for_import,
                        Failure::new(ErrorCode::InvalidInput, error.to_string(), false),
                    );
                    return;
                }
            }
        } else {
            None
        };
        queue_action(
            &tx,
            &weak_for_import,
            require_snapshot(&latest_for_import).map(|_| {
                SlintAction::ImportEncryptedSubscription {
                    name,
                    url,
                    private_key,
                }
            }),
        );
    });

    let tx_for_file = requests.clone();
    let weak_for_file = weak.clone();
    let latest_for_file = Arc::clone(&latest);
    window.on_choose_profile_file(move |name, age_key| {
        let name = name.to_string();
        let private_key = if !age_key.is_empty() {
            match AgePrivateKey::new(age_key.to_string()) {
                Ok(key) => Some(key),
                Err(error) => {
                    show_failure(
                        &weak_for_file,
                        Failure::new(ErrorCode::InvalidInput, error.to_string(), false),
                    );
                    return;
                }
            }
        } else {
            None
        };
        #[cfg(not(target_os = "android"))]
        {
            let picked = rfd::FileDialog::new()
                .add_filter("Configs & Subscriptions", &["yaml", "yml", "json", "txt"])
                .pick_file();
            if let Some(path) = picked {
                let path_str = path.to_string_lossy().to_string();
                let final_name = if name.trim().is_empty() {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("profile")
                        .to_string()
                } else {
                    name
                };
                queue_action(
                    &tx_for_file,
                    &weak_for_file,
                    require_snapshot(&latest_for_file).map(|_| SlintAction::ImportProfileFile {
                        name: final_name,
                        path: path_str,
                        private_key,
                    }),
                );
            }
        }
        #[cfg(target_os = "android")]
        {
            show_unsupported(
                &weak_for_file,
                "Android SAF file selection is handled through Intent",
            );
        }
    });

    let tx = requests.clone();
    let weak_for_profile = weak.clone();
    window.on_set_current_profile(move |profile_id| {
        let profile_id = profile_id.to_string();
        if profile_id.is_empty() {
            queue_intent(&tx, &weak_for_profile, CommandIntent::ClearActiveProfile);
        } else {
            queue_intent(
                &tx,
                &weak_for_profile,
                CommandIntent::SwitchProfile { profile_id },
            );
        }
    });

    let weak_for_move = weak.clone();
    let latest_for_move = Arc::clone(&latest);
    let tx_for_move = requests.clone();
    window.on_move_profile(move |profile_id, move_up| {
        let profile_id = profile_id.to_string();
        let action = require_snapshot(&latest_for_move).and_then(|snapshot| {
            let page = snapshot.pages.profiles.data.as_ref().ok_or_else(|| {
                Failure::new(ErrorCode::NotReady, "Profiles page was not observed", true)
            })?;
            let current_order: Vec<String> = page.profiles.iter().map(|p| p.id.clone()).collect();
            let Some(idx) = current_order.iter().position(|id| id == &profile_id) else {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "Profile not found in catalog",
                    false,
                ));
            };
            if move_up && idx == 0 {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "Profile is already at the top",
                    false,
                ));
            }
            if !move_up && idx + 1 >= current_order.len() {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "Profile is already at the bottom",
                    false,
                ));
            }
            let mut target_order = current_order;
            let target_idx = if move_up { idx - 1 } else { idx + 1 };
            target_order.swap(idx, target_idx);
            Ok(SlintAction::Command(CommandIntent::ReorderProfiles {
                order: target_order,
            }))
        });
        queue_action(&tx_for_move, &weak_for_move, action);
    });

    let weak_for_delete_dialog = weak.clone();
    let latest_for_delete_dialog = Arc::clone(&latest);
    window.on_delete_profile(move |profile_id| {
        let id = profile_id.to_string();
        let name = latest_for_delete_dialog
            .lock()
            .ok()
            .and_then(|snapshot| snapshot.clone())
            .and_then(|snapshot| {
                snapshot
                    .pages
                    .profiles
                    .data
                    .as_ref()?
                    .profiles
                    .iter()
                    .find(|profile| profile.id == id)
                    .map(|profile| profile.name.clone())
            })
            .unwrap_or_else(|| id.clone());
        let _ = weak_for_delete_dialog
            .clone()
            .upgrade_in_event_loop(move |window| {
                window.set_pending_delete_id(id.into());
                window.set_pending_delete_name(name.into());
            });
    });

    let tx = requests.clone();
    let weak_for_refresh = weak.clone();
    window.on_refresh_profile(move |profile_id| {
        queue_intent(
            &tx,
            &weak_for_refresh,
            CommandIntent::UpdateProfile {
                profile_id: profile_id.to_string(),
            },
        );
    });

    let latest_for_settings = Arc::clone(&latest);
    let weak_for_settings = weak.clone();
    let tx_for_settings = requests.clone();
    window.on_open_profile_settings(move |profile_id| {
        open_profile_settings(
            &weak_for_settings,
            &latest_for_settings,
            profile_id.to_string(),
            &tx_for_settings,
        );
    });

    let tx_for_clear_key = requests.clone();
    let weak_for_clear_key = weak.clone();
    let latest_for_clear_key = Arc::clone(&latest);
    window.on_clear_profile_age_key(move |profile_id| {
        let profile_id = profile_id.to_string();
        let action = require_snapshot(&latest_for_clear_key)
            .map(|_| SlintAction::ClearProfileAgeCredential { profile_id });
        queue_action(&tx_for_clear_key, &weak_for_clear_key, action);
    });

    let tx = requests.clone();
    let weak_for_preview = weak.clone();
    window.on_show_profile_preview(move |profile_id| {
        let profile_id = profile_id.to_string();
        let weak = weak_for_preview.clone();
        let _ = weak.clone().upgrade_in_event_loop({
            let profile_id = profile_id.clone();
            move |window| {
                window.set_content_preview_profile(profile_id.into());
                window.set_content_preview("".into());
            }
        });
        queue_intent(
            &tx,
            &weak_for_preview,
            CommandIntent::LoadProfileDocument {
                profile: Some(profile_id),
            },
        );
    });

    let tx = requests.clone();
    let latest_for_save = Arc::clone(&latest);
    let weak_for_save = weak.clone();
    window.on_save_profile_settings(move |profile_id, name, url, age_key| {
        let profile_id = profile_id.to_string();
        let name = name.to_string();
        let url = url.to_string();
        let private_key = if !age_key.is_empty() {
            match AgePrivateKey::new(age_key.to_string()) {
                Ok(key) => Some(key),
                Err(error) => {
                    show_failure(
                        &weak_for_save,
                        Failure::new(ErrorCode::InvalidInput, error.to_string(), false),
                    );
                    return;
                }
            }
        } else {
            None
        };
        let action = require_snapshot(&latest_for_save).and_then(|snapshot| {
            let profile = snapshot
                .pages
                .profiles
                .data
                .as_ref()
                .and_then(|page| {
                    page.profiles
                        .iter()
                        .find(|profile| profile.id == profile_id)
                })
                .ok_or_else(|| {
                    Failure::new(ErrorCode::NotReady, "Profile was not observed", true)
                })?;
            let name_update = if profile.name != name {
                Some(name)
            } else {
                None
            };
            if profile.write_protection != ProfileWriteProtection::RemoteSubscription
                && (url.trim() != profile.url.trim() && !url.trim().is_empty()
                    || private_key.is_some())
            {
                return Err(Failure::unsupported(
                    "Local profile metadata editing is not part of the shared profile contract yet",
                ));
            }
            Ok(SlintAction::SaveProfileSettings {
                profile_id,
                name: name_update,
                url: if url.trim().is_empty() {
                    profile.url.clone()
                } else {
                    url
                },
                private_key,
            })
        });
        let weak = weak_for_save.clone();
        let _ = weak.clone().upgrade_in_event_loop(|window| {
            window.set_profile_settings_saving(true);
        });
        queue_action(&tx, &weak_for_save, action);
    });

    let weak_for_cancel_settings = weak.clone();
    window.on_cancel_profile_settings(move || {
        let _ = weak_for_cancel_settings
            .clone()
            .upgrade_in_event_loop(|window| window.set_profile_settings_id("".into()));
    });

    let tx_for_qr = requests.clone();
    let weak_for_qr = weak.clone();
    window.on_scan_profile_qr(move || {
        queue_action(&tx_for_qr, &weak_for_qr, Ok(SlintAction::ScanQrCode));
    });
    let tx_for_toggle_override = requests.clone();
    let weak_for_override = weak.clone();
    let latest_for_toggle_override = Arc::clone(&latest);
    window.on_toggle_profile_override(move |override_id, enabled| {
        let action = require_snapshot(&latest_for_toggle_override).and_then(|snapshot| {
            let page = snapshot.pages.profiles.data.as_ref().ok_or_else(|| {
                Failure::new(ErrorCode::NotReady, "Profiles page was not observed", true)
            })?;
            let active_profile = page.profiles.iter().find(|p| p.is_active).ok_or_else(|| {
                Failure::new(ErrorCode::NotReady, "No active profile is selected", true)
            })?;
            Ok(SlintAction::Command(
                CommandIntent::SetProfileOverrideBinding {
                    profile_id: active_profile.id.clone(),
                    override_id: override_id.to_string(),
                    enabled,
                },
            ))
        });
        queue_action(&tx_for_toggle_override, &weak_for_override, action);
    });

    let tx_for_move_override = requests.clone();
    let weak_for_override_move = weak.clone();
    let latest_for_move_override = Arc::clone(&latest);
    window.on_move_profile_override(move |override_id, move_up| {
        let override_id = override_id.to_string();
        let action = require_snapshot(&latest_for_move_override).and_then(|snapshot| {
            let page = snapshot.pages.profiles.data.as_ref().ok_or_else(|| {
                Failure::new(ErrorCode::NotReady, "Profiles page was not observed", true)
            })?;
            let active_profile = page.profiles.iter().find(|p| p.is_active).ok_or_else(|| {
                Failure::new(ErrorCode::NotReady, "No active profile is selected", true)
            })?;
            let bindings = page
                .named_overrides
                .bindings_for_profile(&active_profile.id);
            let current_order: Vec<String> =
                bindings.iter().map(|b| b.override_id.to_string()).collect();
            let Some(idx) = current_order.iter().position(|id| id == &override_id) else {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "Override not bound to profile",
                    false,
                ));
            };
            if move_up && idx == 0 {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "Override is already at the top",
                    false,
                ));
            }
            if !move_up && idx + 1 >= current_order.len() {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "Override is already at the bottom",
                    false,
                ));
            }
            let mut target_order = current_order;
            let target_idx = if move_up { idx - 1 } else { idx + 1 };
            target_order.swap(idx, target_idx);
            Ok(SlintAction::Command(
                CommandIntent::ReorderProfileOverrides {
                    profile_id: active_profile.id.clone(),
                    order: target_order,
                },
            ))
        });
        queue_action(&tx_for_move_override, &weak_for_override_move, action);
    });
}
