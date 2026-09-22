//! Shared tray-status contract (DUAL-15-02).
//!
//! The system tray is a per-surface capability, not a shared window: the Iced
//! desktop host runs a real StatusNotifierItem backend, the Bevy shell has no
//! tray at all. What must not differ is the *vocabulary* both sides speak:
//!
//! * the live rate badge — projected from the newest real
//!   [`TrafficWaveformSnapshot`] sample, formatted once (bytes/s ladder) so the
//!   tray title, the tray tooltip and any future surface show identical text;
//! * the refresh cadence — a per-sample spec push would flood D-Bus, so the
//!   contract owns the maximum push rate the host may use;
//! * the capability report — a surface either hosts a tray (and says whether
//!   it can carry the live badge) or states the typed reason it does not.
//!   A missing tray is a capability difference: it must never fabricate a
//!   rate badge it cannot display.

use crate::traffic_waveform::TrafficWaveformSnapshot;

/// The minimum interval between two live rate-badge pushes to a tray backend.
///
/// The shared traffic waveform arrives per sample; the tray menu is rebuilt at
/// most once per interval so a 60 Hz sampling loop cannot spam D-Bus / the
/// native menu event loop.
pub const TRAY_RATE_REFRESH_INTERVAL_MS: u64 = 1_000;

/// Uplink symbol used by the shared badge text.
pub const TRAY_RATE_UP_SYMBOL: &str = "↑";
/// Downlink symbol used by the shared badge text.
pub const TRAY_RATE_DOWN_SYMBOL: &str = "↓";

/// A live rate reading projected from the newest real traffic sample.
///
/// Both rates are sanitized on construction: a non-finite or negative sample
/// clamps to `0 B/s` instead of leaking `NaN` into a tooltip. The badge is only
/// built from a *real* sample — an empty waveform yields `None`, never a
/// zeroed-out badge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrayRateBadge {
    pub up_bps: f64,
    pub down_bps: f64,
    /// The sample's timestamp, when the host provided one.
    pub sampled_at_epoch_ms: Option<i64>,
}

impl TrayRateBadge {
    /// Project the newest sample of a live waveform.
    pub fn from_waveform(snapshot: &TrafficWaveformSnapshot) -> Option<Self> {
        let sample = snapshot.samples.last()?;
        let up_bps = sanitize_bps(sample.upload_bps);
        let down_bps = sanitize_bps(sample.download_bps);
        if !sample.upload_bps.is_finite() || !sample.download_bps.is_finite() {
            return None;
        }
        Some(Self {
            up_bps,
            down_bps,
            sampled_at_epoch_ms: sample.sampled_at_epoch_ms,
        })
    }

    /// Whether the newest sample reports no traffic at all.
    pub fn is_idle(&self) -> bool {
        self.up_bps <= 0.0 && self.down_bps <= 0.0
    }

    /// The uplink reading as display text (`1.2 MB/s`).
    pub fn up_text(&self) -> String {
        Self::format_rate(self.up_bps)
    }

    /// The downlink reading as display text (`0 B/s`).
    pub fn down_text(&self) -> String {
        Self::format_rate(self.down_bps)
    }

    /// The compact duplex line a tray tooltip/title carries, e.g.
    /// `↑ 1.2 MB/s · ↓ 340.0 KB/s`.
    pub fn badge_text(&self) -> String {
        format!(
            "{TRAY_RATE_UP_SYMBOL} {} · {TRAY_RATE_DOWN_SYMBOL} {}",
            self.up_text(),
            self.down_text()
        )
    }

    /// Format one bytes-per-second reading with the shared decimal ladder
    /// (`B/s` / `KB/s` / `MB/s` / `GB/s`; `0 B/s` for unknown or non-finite
    /// input). Unit boundaries are decimal (1000-based) to match how the
    /// traffic projections report rates.
    pub fn format_rate(bps: f64) -> String {
        if !bps.is_finite() || bps <= 0.0 {
            return "0 B/s".to_string();
        }
        const LADDER: [(&str, f64); 3] = [
            ("GB/s", 1_000_000_000.0),
            ("MB/s", 1_000_000.0),
            ("KB/s", 1_000.0),
        ];
        for (unit, scale) in LADDER {
            if bps >= scale {
                return format!("{:.1} {unit}", bps / scale);
            }
        }
        format!("{} B/s", bps.round() as u64)
    }
}

/// What one surface can honestly do with the shared tray contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraySupport {
    /// The surface hosts a real OS tray.
    Hosted {
        /// Whether the hosted backend can carry the live rate badge text
        /// (menu title/tooltip/info line). A backend that only renders a
        /// static icon reports `false`.
        live_rate_badge: bool,
    },
    /// The surface has no tray host. `reason` is the typed boundary reported
    /// instead of a fabricated badge.
    Unsupported { reason: &'static str },
}

impl TraySupport {
    /// Whether a real tray is hosted here.
    pub const fn is_hosted(self) -> bool {
        matches!(self, Self::Hosted { .. })
    }

    /// Whether the live rate badge may be pushed on this surface.
    pub const fn live_rate_badge(self) -> bool {
        match self {
            Self::Hosted { live_rate_badge } => live_rate_badge,
            Self::Unsupported { .. } => false,
        }
    }

    /// The typed boundary of a surface without a tray.
    pub const fn unsupported_reason(self) -> Option<&'static str> {
        match self {
            Self::Hosted { .. } => None,
            Self::Unsupported { reason } => Some(reason),
        }
    }
}

fn sanitize_bps(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};

    fn sample(up: f64, down: f64) -> TrafficSample {
        TrafficSample {
            sampled_at_epoch_ms: Some(1_700_000_000_000),
            upload_bps: up,
            download_bps: down,
        }
    }

    #[test]
    fn the_rate_badge_projects_the_newest_real_sample() {
        let snapshot = TrafficWaveformSnapshot {
            generation: 3,
            revision: 9,
            samples: vec![sample(10.0, 20.0), sample(2_048.0, 512.0)],
        };
        let badge = TrayRateBadge::from_waveform(&snapshot).expect("newest sample");
        assert_eq!(badge.up_bps, 2_048.0);
        assert_eq!(badge.down_bps, 512.0);
        assert_eq!(badge.sampled_at_epoch_ms, Some(1_700_000_000_000));
        assert_eq!(badge.up_text(), "2.0 KB/s");
        assert_eq!(badge.down_text(), "512 B/s");
        assert_eq!(badge.badge_text(), "↑ 2.0 KB/s · ↓ 512 B/s");
        assert!(!badge.is_idle());
    }

    #[test]
    fn an_empty_or_non_finite_waveform_never_reports_a_badge() {
        assert!(TrayRateBadge::from_waveform(&TrafficWaveformSnapshot::default()).is_none());
        let non_finite = TrafficWaveformSnapshot {
            generation: 1,
            revision: 1,
            samples: vec![sample(f64::NAN, 1.0)],
        };
        assert!(TrayRateBadge::from_waveform(&non_finite).is_none());
    }

    #[test]
    fn a_negative_sample_clamps_to_zero_without_faking_activity() {
        let snapshot = TrafficWaveformSnapshot {
            generation: 1,
            revision: 1,
            samples: vec![sample(-4.0, 0.0)],
        };
        let badge = TrayRateBadge::from_waveform(&snapshot).expect("sample present");
        assert_eq!(badge.up_text(), "0 B/s");
        assert!(badge.is_idle());
    }

    #[test]
    fn the_rate_ladder_is_decimal_and_bounded() {
        assert_eq!(TrayRateBadge::format_rate(0.0), "0 B/s");
        assert_eq!(TrayRateBadge::format_rate(-1.0), "0 B/s");
        assert_eq!(TrayRateBadge::format_rate(f64::INFINITY), "0 B/s");
        assert_eq!(TrayRateBadge::format_rate(999.0), "999 B/s");
        assert_eq!(TrayRateBadge::format_rate(1_000.0), "1.0 KB/s");
        assert_eq!(TrayRateBadge::format_rate(1_500_000.0), "1.5 MB/s");
        assert_eq!(TrayRateBadge::format_rate(12_000_000_000.0), "12.0 GB/s");
    }

    #[test]
    fn a_surface_without_a_tray_states_the_typed_reason() {
        let hosted = TraySupport::Hosted {
            live_rate_badge: true,
        };
        assert!(hosted.is_hosted());
        assert!(hosted.live_rate_badge());
        assert_eq!(hosted.unsupported_reason(), None);

        let capped = TraySupport::Hosted {
            live_rate_badge: false,
        };
        assert!(capped.is_hosted());
        assert!(!capped.live_rate_badge());

        let absent = TraySupport::Unsupported {
            reason: "surface-has-no-tray-host",
        };
        assert!(!absent.is_hosted());
        assert!(!absent.live_rate_badge());
        assert_eq!(
            absent.unsupported_reason(),
            Some("surface-has-no-tray-host")
        );
    }
}
