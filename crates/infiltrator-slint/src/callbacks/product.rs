//! Slint product callbacks routed to the shared product owner.

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
    let weak_for_confirm_delete = weak.clone();
    window.on_confirm_delete_profile(move |profile_id| {
        queue_intent(
            &tx,
            &weak_for_confirm_delete,
            CommandIntent::DeleteProfile {
                profile_id: profile_id.to_string(),
            },
        );
    });
    let weak_for_export_profile = weak.clone();
    let latest_for_export_profile = Arc::clone(&latest);
    let tx_for_export_profile = requests.clone();
    window.on_export_profile(move |profile_id| {
        let profile_id = profile_id.to_string();
        #[cfg(not(target_os = "android"))]
        {
            let picked = rfd::FileDialog::new()
                .set_file_name(&format!("{profile_id}.yaml"))
                .add_filter("YAML Config", &["yaml", "yml"])
                .save_file();
            if let Some(destination) = picked {
                queue_action(
                    &tx_for_export_profile,
                    &weak_for_export_profile,
                    require_snapshot(&latest_for_export_profile).map(|_| SlintAction::ExportProfile {
                        profile_id,
                        destination,
                    }),
                );
            }
        }
        #[cfg(target_os = "android")]
        {
            show_unsupported(&weak_for_export_profile, "Android SAF file export is handled through Intent");
        }
    });
    let tx_for_clear_traffic = requests.clone();
    let weak_for_clear_traffic = weak.clone();
    window.on_clear_traffic_history(move || {
        let _ = weak_for_clear_traffic
            .clone()
            .upgrade_in_event_loop(|window| {
                window.set_traffic_today_bars(slint_model(Vec::<TrafficBar>::new()));
                window.set_traffic_week_bars(slint_model(Vec::<TrafficBar>::new()));
                window.set_traffic_today_has_data(false);
                window.set_traffic_week_has_data(false);
                window.set_traffic_today_down("0 B".into());
                window.set_traffic_today_up("0 B".into());
                window.set_traffic_week_down("0 B".into());
                window.set_traffic_week_up("0 B".into());
                window.set_traffic_week_average_down("0 B".into());
                window.set_traffic_week_average_up("0 B".into());
            });
        queue_intent(
            &tx_for_clear_traffic,
            &weak_for_clear_traffic,
            CommandIntent::ClearTrafficHistory,
        );
    });
    let weak_for_license_search = weak.clone();
    let latest_for_license_search = Arc::clone(&latest);
    window.on_search_licenses(move |_| {
        refresh_projection(&weak_for_license_search, &latest_for_license_search);
    });
    let weak_for_open_license = weak.clone();
    window.on_open_license(move |id| {
        if id.as_str() != "product.third-party-notices" {
            show_unsupported(
                &weak_for_open_license,
                "No local license notice exists for that catalog entry",
            );
            return;
        }
        let _ = weak_for_open_license
            .clone()
            .upgrade_in_event_loop(|window| {
                window.set_license_detail_name("MusicFrog third-party notices".into());
                window.set_license_detail_version(env!("CARGO_PKG_VERSION").into());
                window.set_license_detail_identifier("See included notices".into());
                window.set_license_detail_text(lumen_projection::THIRD_PARTY_NOTICES.into());
                window.set_license_detail_error("".into());
                window.set_license_detail_revision(window.get_license_detail_revision() + 1);
            });
    });
    let weak_for_license_retry = weak.clone();
    let latest_for_license_retry = Arc::clone(&latest);
    window.on_retry_license_catalog(move || {
        refresh_projection(&weak_for_license_retry, &latest_for_license_retry);
    });
}
