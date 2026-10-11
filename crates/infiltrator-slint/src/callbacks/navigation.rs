//! Slint navigation callbacks routed to the shared product owner.

use super::super::*;
use std::sync::{Arc, Mutex};

pub(super) fn register(window: &AppWindow, latest: Arc<Mutex<Option<SurfaceSnapshot>>>) {
    let weak = window.as_weak();
    // Navigation and appearance previews belong to this shared component.
    let weak_for_navigation = weak.clone();
    let latest_for_navigation = Arc::clone(&latest);
    window.on_navigate(move |page| {
        let warning = latest_for_navigation
            .lock()
            .ok()
            .and_then(|snapshot| {
                snapshot.as_ref().map(|snapshot| {
                    lumen_projection::snapshot_window_warning(snapshot, page.as_str())
                })
            })
            .unwrap_or_default();
        let _ = weak_for_navigation
            .clone()
            .upgrade_in_event_loop(move |window| {
                window.set_notice("".into());
                window.set_snapshot_window_warning(warning.into());
            });
    });
    let weak_for_scheme = weak.clone();
    window.on_color_scheme_changed(move |dark| {
        let _ = weak_for_scheme
            .clone()
            .upgrade_in_event_loop(move |window| window.set_application_dark_mode(dark));
    });
}
