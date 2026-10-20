//! Parity capture scenarios for the Slint surface.
//!
//! A capture scenario names one business of the shared parity catalogue. The
//! activation opens the same surface state a user gesture opens, and the
//! bounds are the real on-screen geometry of the surface that was opened. A
//! receipt therefore proves an interaction and never a route change.

use crate::AppWindow;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use slint::ComponentHandle;

/// The geometry of the surface a capture activated, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Opens the surface state a parity scenario names.
///
/// The surface owns the state, so the activation only asks the surface to
/// open what a user gesture opens. A scenario the surface cannot open returns
/// `false` and is never published as activated.
pub fn activate(window: &AppWindow, scenario: &str, snapshot: &SurfaceSnapshot) -> bool {
    match scenario {
        "profiles-import-modal" => {
            window.set_profile_add_kind_index(0);
            window.set_profile_add_open(true);
            true
        }
        "profiles-delete-confirm" => {
            let Some(profiles) = snapshot
                .pages
                .profiles
                .data
                .as_ref()
                .map(|page| page.profiles.as_slice())
            else {
                return false;
            };
            let Some(profile) = profiles
                .iter()
                .find(|profile| !profile.is_active)
                .or_else(|| profiles.first())
            else {
                return false;
            };
            window.set_pending_delete_id(profile.id.clone().into());
            window.set_pending_delete_name(profile.name.clone().into());
            true
        }
        _ => false,
    }
}

/// Reads the real geometry of the sheet the surface opened.
///
/// The bounds are empty until the surface really opened a sheet, so a capture
/// never publishes the geometry of a surface that stayed closed.
pub fn bounds(window: &AppWindow) -> Option<CaptureBounds> {
    if !window.get_capture_sheet_open() {
        return None;
    }
    let scale = window.window().scale_factor();
    let bounds = CaptureBounds {
        x: window.get_capture_bounds_x() * scale,
        y: window.get_capture_bounds_y() * scale,
        width: window.get_capture_bounds_width() * scale,
        height: window.get_capture_bounds_height() * scale,
    };
    if bounds.width <= 0.0 || bounds.height <= 0.0 || bounds.x < 0.0 || bounds.y < 0.0 {
        return None;
    }
    Some(bounds)
}
