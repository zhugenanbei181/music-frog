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
        if !age_key.is_empty() {
            show_failure(
                &weak_for_import,
                Failure::unsupported(
                    "Age-encrypted subscription import is not exposed by the shared import owner",
                ),
            );
            return;
        }
        queue_action(
            &tx,
            &weak_for_import,
            require_snapshot(&latest_for_import).map(|_| {
                SlintAction::Command(CommandIntent::ImportSubscription {
                    profile_id: name,
                    channel: SubscriptionImportChannel::Url,
                    source: url,
                })
            }),
        );
    });

    let weak_for_file = weak.clone();
    window.on_choose_profile_file(move |_, _| {
        show_unsupported(
            &weak_for_file,
            "Local file selection needs a platform file-picker adapter",
        );
    });

    let tx = requests.clone();
    let weak_for_profile = weak.clone();
    window.on_set_current_profile(move |profile_id| {
        let profile_id = profile_id.to_string();
        if profile_id.is_empty() {
            show_unsupported(
                &weak_for_profile,
                "The shared profile owner does not expose clearing the active profile",
            );
        } else {
            queue_intent(
                &tx,
                &weak_for_profile,
                CommandIntent::SwitchProfile { profile_id },
            );
        }
    });

    let weak_for_move = weak.clone();
    window.on_move_profile(move |_, _| {
        show_unsupported(
            &weak_for_move,
            "Profile ordering is not part of the shared profile contract yet",
        );
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
    window.on_open_profile_settings(move |profile_id| {
        open_profile_settings(
            &weak_for_settings,
            &latest_for_settings,
            profile_id.to_string(),
        );
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
        if !age_key.is_empty() {
            show_unsupported(
                &weak_for_save,
                "Changing an Age key requires the shared credential editor",
            );
            return;
        }
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
            if profile.name != name {
                return Err(Failure::unsupported(
                    "Profile renaming is not part of the shared profile contract yet",
                ));
            }
            if profile.write_protection != ProfileWriteProtection::RemoteSubscription {
                return Err(Failure::unsupported(
                    "Local profile metadata editing is not part of the shared profile contract yet",
                ));
            }
            let draft = SubscriptionScheduleDraft {
                url: if url.trim().is_empty() {
                    profile.url.clone()
                } else {
                    url
                },
                auto_update_enabled: profile.auto_update_enabled,
                update_interval_hours: profile
                    .update_interval_hours
                    .map(|hours| hours.to_string())
                    .unwrap_or_default(),
                cron_expression: profile.cron_expression.clone(),
            };
            Ok(SlintAction::Command(
                CommandIntent::UpdateSubscriptionSchedule { profile_id, draft },
            ))
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
    let weak_for_override = weak.clone();
    window.on_toggle_profile_override(move |_, _| {
        show_unsupported(
            &weak_for_override,
            "Named Lumen overrides do not map to the shared per-profile Mixin overlay",
        );
    });
    let weak_for_override_move = weak.clone();
    window.on_move_profile_override(move |_, _| {
        show_unsupported(
            &weak_for_override_move,
            "Named Lumen override ordering has no shared owner",
        );
    });
}
