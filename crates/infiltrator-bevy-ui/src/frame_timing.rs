//! Bevy frame-timing sink — product env binding only.
//!
//! The Bevy-scoped plugin, systems, recorder and wire record are owned by
//! `zhuge-ui-kit-bevy::frame_timing`. This module names MusicFrog's opt-in
//! variables and run-level capture marker; a normal launch installs nothing.

use bevy::app::App;
use zhuge_ui_core::frame_timing::record::FrameTimingIdentity;
use zhuge_ui_kit_bevy::frame_timing::{
    BevyFrameTimingEnv, FrameReceiptStyle, install_bevy_frame_timing,
};

fn bevy_frame_timing_env() -> BevyFrameTimingEnv {
    BevyFrameTimingEnv {
        sink_var: "INFILTRATOR_FRAME_TIMING_SINK",
        budget_var: "INFILTRATOR_FRAME_TIMING_BUDGET_MS",
        default_budget_ms: 16.0,
        receipt_vars: &["INFILTRATOR_CAPTURE_MARKER"],
        receipt_style: FrameReceiptStyle::RunLevel,
        identity: FrameTimingIdentity::new("musicfrog", "musicfrog-desktop-shell", "bevy"),
        max_samples: 240,
    }
}

pub(crate) fn install(app: &mut App) -> bool {
    install_bevy_frame_timing(app, &bevy_frame_timing_env())
}
