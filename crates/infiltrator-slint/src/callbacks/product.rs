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
    window.on_export_profile(move |_| {
        show_unsupported(
            &weak_for_export_profile,
            "Profile export needs the shared export review and a platform file-output adapter",
        );
    });
    let weak_for_clear_traffic = weak.clone();
    window.on_clear_traffic_history(move || {
        show_unsupported(
            &weak_for_clear_traffic,
            "The shared owner publishes live traffic samples but no historical traffic store",
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
