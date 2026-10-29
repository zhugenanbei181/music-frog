//! Minimal Lurker Slint example (PUB-006.9).
//!
//! The example shows the shape of every Lurker surface:
//!
//! * the neutral contract owns the lifecycle vocabulary;
//! * the presentation kernel owns the projection to a renderable verdict;
//! * the surface owns nothing but the rendering and the typed user intent.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example minimal-slint
//! ```

#[allow(clippy::todo)]
mod generated_ui {
    slint::include_modules!();
}
use generated_ui::*;

use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::page::SurfaceOrigin;
use lurker_ui_presentation::core_status_projection::{
    OverviewBannerVerdict, project_banner_verdict,
};

fn main() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;

    {
        let window_weak = window.as_weak();
        window.on_start_core(move || {
            if let Some(window) = window_weak.upgrade() {
                publish(&window, CoreLifecycle::Running);
            }
        });
    }

    {
        let window_weak = window.as_weak();
        window.on_stop_core(move || {
            if let Some(window) = window_weak.upgrade() {
                publish(&window, CoreLifecycle::Stopped);
            }
        });
    }

    window.run()
}

/// Publishes the observed lifecycle and its shared projection to the surface.
fn publish(window: &AppWindow, lifecycle: CoreLifecycle) {
    let verdict = project_banner_verdict(SurfaceOrigin::Live, &lifecycle, true, false);
    window.set_core_status(format!("{lifecycle:?}").into());
    window.set_banner_verdict(verdict_text(&verdict).into());
}

/// Renders the shared verdict vocabulary of the presentation kernel.
fn verdict_text(verdict: &OverviewBannerVerdict) -> &'static str {
    match verdict {
        OverviewBannerVerdict::Demo => "demo fixture",
        OverviewBannerVerdict::MissingHost => "missing host",
        OverviewBannerVerdict::CoreStopped => "core stopped",
        OverviewBannerVerdict::VpnUnauthorized => "vpn unauthorized",
        OverviewBannerVerdict::RealRunning => "real running",
        OverviewBannerVerdict::Transitioning => "transitioning",
    }
}
