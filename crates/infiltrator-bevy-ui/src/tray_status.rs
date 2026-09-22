//! Bevy-side tray capability report (DUAL-15-02).
//!
//! The Bevy shell has no OS tray host: no StatusNotifierItem, no muda or
//! tray-icon wiring, and no per-sample menu push. A missing tray is a
//! capability difference, not a gap to paper over, so this surface reports the
//! shared typed boundary ([`infiltrator_contract::tray_status::TraySupport`])
//! instead of fabricating a rate badge — while still speaking the shared
//! vocabulary (badge projection, refresh cadence), so a future Bevy tray host
//! only has to flip the report.

use bevy::app::{App, Plugin};
use bevy::ecs::resource::Resource;
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;
use infiltrator_contract::tray_status::{
    TRAY_RATE_REFRESH_INTERVAL_MS, TrayRateBadge, TraySupport,
};

/// The typed boundary this surface reports: no tray host exists here.
pub const fn support() -> TraySupport {
    TraySupport::Unsupported {
        reason: "bevy-shell-has-no-tray-host",
    }
}

/// Queryable tray capability, inserted by [`TrayStatusPlugin`].
///
/// The report carries the shared cadence as well as the support state, so a
/// host that does get a tray cannot invent its own push interval.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrayStatusReport {
    pub support: TraySupport,
    pub refresh_interval_ms: u64,
}

impl Default for TrayStatusReport {
    fn default() -> Self {
        Self {
            support: support(),
            refresh_interval_ms: TRAY_RATE_REFRESH_INTERVAL_MS,
        }
    }
}

impl TrayStatusReport {
    /// The live rate badge this surface may push for a waveform. A surface
    /// without a tray host answers `None` no matter what the waveform holds:
    /// reporting a badge nobody can display would be a fabrication.
    pub fn live_badge(self, snapshot: &TrafficWaveformSnapshot) -> Option<TrayRateBadge> {
        if self.support.live_rate_badge() {
            TrayRateBadge::from_waveform(snapshot)
        } else {
            None
        }
    }
}

/// Installs the honest tray capability report.
pub struct TrayStatusPlugin;

impl Plugin for TrayStatusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TrayStatusReport>();
    }
}
