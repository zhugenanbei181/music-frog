//! Shared tray-status contract (DUAL-15-02 / FLPARITY-006).
//!
//! The system tray is a per-surface capability, not a shared window: every
//! desktop host may run a real StatusNotifierItem / notification-area
//! backend, while a surface without an OS tray host says so with a typed
//! reason. What must not differ is the *vocabulary* all sides speak:
//!
//! * the live rate badge — projected from the newest real
//!   [`TrafficWaveformSnapshot`] sample, formatted once (bytes/s ladder) so the
//!   tray title, the tray tooltip and any future surface show identical text;
//! * the refresh cadence — a per-sample spec push would flood D-Bus, so the
//!   contract owns the maximum push rate the host may use;
//! * the capability report — a surface either hosts a tray (and says whether
//!   it can carry the live badge and which icon style it uses) or states the
//!   typed reason it does not. A missing tray is a capability difference: it
//!   must never fabricate a rate badge it cannot display;
//! * the activation vocabulary — one [`TrayEvent`] in, one typed
//!   [`TrayIntent`] out, so every desktop surface resolves a click the same
//!   way instead of re-deriving the meaning of an action id.
//!
//! The neutral menu tree itself lives in [`crate::tray_menu`]; the localized
//! rendering of that tree lives in the desktop host layer.

use crate::traffic_waveform::TrafficWaveformSnapshot;
use crate::tray_menu::{
    TRAY_ACTION_ACTIVATE_PROFILE, TRAY_ACTION_CANCEL_CORE_DOWNLOAD, TRAY_ACTION_CANCEL_SYNC,
    TRAY_ACTION_CHECK_CORE_UPDATE, TRAY_ACTION_FACTORY_RESET, TRAY_ACTION_FLUSH_FAKEIP,
    TRAY_ACTION_NAVIGATE_SYNC, TRAY_ACTION_QUIT, TRAY_ACTION_ROUTING_ALL_THROUGH_PROXY,
    TRAY_ACTION_ROUTING_BYPASS_PROXY, TRAY_ACTION_ROUTING_FOLLOW_RULES,
    TRAY_ACTION_SELECT_GLOBAL_PROXY, TRAY_ACTION_SELECT_PROXY, TRAY_ACTION_SET_DEFAULT_KERNEL,
    TRAY_ACTION_SET_PROFILE_AUTO_UPDATE, TRAY_ACTION_SHOW, TRAY_ACTION_SYNC_DOWNLOAD,
    TRAY_ACTION_SYNC_UPLOAD, TRAY_ACTION_TOGGLE_AUTOSTART, TRAY_ACTION_TOGGLE_COLOR_MODE,
    TRAY_ACTION_TOGGLE_SYSTEM_PROXY, TRAY_ACTION_TOGGLE_THEME, TRAY_ACTION_TOGGLE_TUN,
    TRAY_ACTION_UNINSTALL_KERNEL, TRAY_ACTION_UPDATE_ALL_PROFILES, TrayActionId, TrayProfileEntry,
    decode_pair_payload,
};
use lurker_core_contract::routing_intent::RoutingIntent;
use serde::{Deserialize, Serialize};

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
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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

/// The visual style of the tray icon (FlClash v0.9.2 parity).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrayIconColorMode {
    /// Full-color status indicator (green=active, gray=stopped, blue=connecting,
    /// yellow=warning).
    #[default]
    Colored,
    /// Monochrome template icon adapting to macOS menu bar or dark/light taskbars.
    Monochrome,
}

impl TrayIconColorMode {
    pub const fn is_monochrome(self) -> bool {
        matches!(self, Self::Monochrome)
    }

    pub const fn toggle(self) -> Self {
        match self {
            Self::Colored => Self::Monochrome,
            Self::Monochrome => Self::Colored,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Colored => "colored",
            Self::Monochrome => "monochrome",
        }
    }
}

/// Operational state of the backend as represented in the system tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrayServiceStatus {
    #[default]
    Stopped,
    Running,
    Connecting,
    Warning,
}

impl TrayServiceStatus {
    pub const fn status_color_hex(self) -> &'static str {
        match self {
            Self::Running => "#4CAF50",
            Self::Stopped => "#9E9E9E",
            Self::Connecting => "#2196F3",
            Self::Warning => "#FFC107",
        }
    }

    pub const fn status_rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Running => (76, 175, 80),
            Self::Stopped => (158, 158, 158),
            Self::Connecting => (33, 150, 243),
            Self::Warning => (255, 193, 7),
        }
    }

    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Stopped => "Stopped",
            Self::Connecting => "Connecting",
            Self::Warning => "Warning",
        }
    }
}

/// A snapshot of the tray tooltip information.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrayTooltipSnapshot {
    pub service_status: TrayServiceStatus,
    pub routing_name: String,
    pub active_profile: String,
    pub speed_display: String,
}

impl TrayTooltipSnapshot {
    pub fn format_tooltip(&self) -> String {
        format!(
            "Status: {}\nRouting: {}\nProfile: {}\nSpeed: {}",
            self.service_status.display_label(),
            self.routing_name,
            self.active_profile,
            self.speed_display
        )
    }
}

/// What one surface can honestly do with the shared tray contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraySupport {
    /// The surface hosts a real OS tray.
    Hosted {
        /// Whether the hosted backend can carry the live rate badge text
        /// (menu title/tooltip/info line). A backend that only renders a
        /// static icon reports `false`.
        live_rate_badge: bool,
        /// The visual color style of the tray icon.
        #[serde(default)]
        color_mode: TrayIconColorMode,
    },
    /// The surface has no tray host. `reason` is the typed boundary reported
    /// instead of a fabricated badge.
    Unsupported { reason: &'static str },
}

impl TraySupport {
    /// Convenience constructor for hosted tray with default colored mode.
    pub const fn hosted(live_rate_badge: bool) -> Self {
        Self::Hosted {
            live_rate_badge,
            color_mode: TrayIconColorMode::Colored,
        }
    }

    /// Convenience constructor with explicit color mode.
    pub const fn hosted_with_mode(live_rate_badge: bool, color_mode: TrayIconColorMode) -> Self {
        Self::Hosted {
            live_rate_badge,
            color_mode,
        }
    }

    /// Whether a real tray is hosted here.
    pub const fn is_hosted(self) -> bool {
        matches!(self, Self::Hosted { .. })
    }

    /// Whether the live rate badge may be pushed on this surface.
    pub const fn live_rate_badge(self) -> bool {
        match self {
            Self::Hosted {
                live_rate_badge, ..
            } => live_rate_badge,
            Self::Unsupported { .. } => false,
        }
    }

    /// The visual color style if hosted, or Colored if unsupported.
    pub const fn color_mode(self) -> TrayIconColorMode {
        match self {
            Self::Hosted { color_mode, .. } => color_mode,
            Self::Unsupported { .. } => TrayIconColorMode::Colored,
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

/// A user activation coming from any backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrayEvent {
    /// A menu entry was activated; `payload` is the entry's payload echoed
    /// back (also set for checkmarks that carry one).
    MenuActivated {
        id: TrayActionId,
        payload: Option<String>,
    },
    /// The tray icon itself was activated (left click).
    IconActivated,
}

/// Frontend intent behind a tray activation, fully resolved against the
/// current app state (checkmarks arrive already flipped).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayIntent {
    ShowWindow,
    Exit,
    ToggleTheme,
    SetRoutingIntent(RoutingIntent),
    SetSystemProxy(bool),
    SetTunEnabled(bool),
    /// Legacy GLOBAL quick-switch entry; the live event becomes typed at the
    /// app boundary.
    SelectGlobalOutbound(String),
    SelectOutbound {
        collection: String,
        outbound: String,
    },
    ActivateProfile(String),
    UpdateAllProfilesNow,
    SetProfileAutoUpdate {
        name: String,
        enabled: bool,
    },
    SetDefaultKernel(String),
    /// Stage a kernel uninstall behind the confirmation dialog.
    UninstallKernel(String),
    UpdateCoreToLatest,
    CancelCoreDownload,
    FlushFakeIp,
    SetAutostart(bool),
    SyncUpload,
    SyncDownload,
    CancelSync,
    NavigateSync,
    RequestFactoryReset,
    /// Flip the tray icon between the colored status indicator and the
    /// monochrome template icon (macOS menu bar / dark taskbars).
    ToggleIconColorMode,
}

/// App-side state snapshot needed to resolve one activation event.
#[derive(Clone, Copy, Debug)]
pub struct TrayEventContext<'a> {
    pub system_proxy: bool,
    pub tun: bool,
    pub autostart: bool,
    pub profiles: &'a [TrayProfileEntry],
}

/// Pure mapping from a neutral event to an intent. Toggle entries are
/// resolved against the current app-side states carried in `ctx`
/// (checkmark targets arrive already flipped).
pub fn resolve_tray_event_in(event: &TrayEvent, ctx: &TrayEventContext<'_>) -> Option<TrayIntent> {
    let TrayEvent::MenuActivated { id, payload } = event else {
        return Some(TrayIntent::ShowWindow);
    };
    let payload = payload.as_deref();
    let intent = match *id {
        TRAY_ACTION_SHOW => TrayIntent::ShowWindow,
        TRAY_ACTION_QUIT => TrayIntent::Exit,
        TRAY_ACTION_TOGGLE_THEME => TrayIntent::ToggleTheme,
        TRAY_ACTION_TOGGLE_COLOR_MODE => TrayIntent::ToggleIconColorMode,
        TRAY_ACTION_ROUTING_FOLLOW_RULES => {
            TrayIntent::SetRoutingIntent(RoutingIntent::FollowRules)
        }
        TRAY_ACTION_ROUTING_ALL_THROUGH_PROXY => {
            TrayIntent::SetRoutingIntent(RoutingIntent::RouteAllThroughProxy)
        }
        TRAY_ACTION_ROUTING_BYPASS_PROXY => {
            TrayIntent::SetRoutingIntent(RoutingIntent::BypassProxy)
        }
        TRAY_ACTION_TOGGLE_SYSTEM_PROXY => TrayIntent::SetSystemProxy(!ctx.system_proxy),
        TRAY_ACTION_TOGGLE_TUN => TrayIntent::SetTunEnabled(!ctx.tun),
        TRAY_ACTION_TOGGLE_AUTOSTART => TrayIntent::SetAutostart(!ctx.autostart),
        TRAY_ACTION_SELECT_GLOBAL_PROXY => TrayIntent::SelectGlobalOutbound(payload?.to_string()),
        TRAY_ACTION_SELECT_PROXY => {
            let (group, node) = decode_pair_payload(payload?)?;
            TrayIntent::SelectOutbound {
                collection: group.to_string(),
                outbound: node.to_string(),
            }
        }
        TRAY_ACTION_ACTIVATE_PROFILE => TrayIntent::ActivateProfile(payload?.to_string()),
        TRAY_ACTION_UPDATE_ALL_PROFILES => TrayIntent::UpdateAllProfilesNow,
        TRAY_ACTION_SET_PROFILE_AUTO_UPDATE => {
            let name = payload?;
            let current = ctx.profiles.iter().find(|profile| profile.name == name)?;
            TrayIntent::SetProfileAutoUpdate {
                name: name.to_string(),
                enabled: !current.auto_update_enabled,
            }
        }
        TRAY_ACTION_SET_DEFAULT_KERNEL => TrayIntent::SetDefaultKernel(payload?.to_string()),
        TRAY_ACTION_UNINSTALL_KERNEL => TrayIntent::UninstallKernel(payload?.to_string()),
        TRAY_ACTION_CHECK_CORE_UPDATE => TrayIntent::UpdateCoreToLatest,
        TRAY_ACTION_CANCEL_CORE_DOWNLOAD => TrayIntent::CancelCoreDownload,
        TRAY_ACTION_FLUSH_FAKEIP => TrayIntent::FlushFakeIp,
        TRAY_ACTION_SYNC_UPLOAD => TrayIntent::SyncUpload,
        TRAY_ACTION_SYNC_DOWNLOAD => TrayIntent::SyncDownload,
        TRAY_ACTION_CANCEL_SYNC => TrayIntent::CancelSync,
        TRAY_ACTION_NAVIGATE_SYNC => TrayIntent::NavigateSync,
        TRAY_ACTION_FACTORY_RESET => TrayIntent::RequestFactoryReset,
        _ => return None,
    };
    Some(intent)
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
    use crate::tray_menu::TRAY_ACTION_INFO_STATUS;

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
        let hosted = TraySupport::hosted(true);
        assert!(hosted.is_hosted());
        assert!(hosted.live_rate_badge());
        assert_eq!(hosted.color_mode(), TrayIconColorMode::Colored);
        assert_eq!(hosted.unsupported_reason(), None);

        let capped = TraySupport::hosted_with_mode(false, TrayIconColorMode::Monochrome);
        assert!(capped.is_hosted());
        assert!(!capped.live_rate_badge());
        assert_eq!(capped.color_mode(), TrayIconColorMode::Monochrome);

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

    #[test]
    fn tray_color_modes_and_status_colors_are_sound() {
        assert_eq!(TrayIconColorMode::Colored.as_str(), "colored");
        assert_eq!(TrayIconColorMode::Monochrome.as_str(), "monochrome");
        assert_eq!(
            TrayIconColorMode::Colored.toggle(),
            TrayIconColorMode::Monochrome
        );
        assert_eq!(
            TrayIconColorMode::Monochrome.toggle(),
            TrayIconColorMode::Colored
        );

        assert_eq!(TrayServiceStatus::Running.status_color_hex(), "#4CAF50");
        assert_eq!(TrayServiceStatus::Stopped.status_color_hex(), "#9E9E9E");
        assert_eq!(TrayServiceStatus::Connecting.status_color_hex(), "#2196F3");
        assert_eq!(TrayServiceStatus::Warning.status_color_hex(), "#FFC107");
        assert_eq!(TrayServiceStatus::Running.status_rgb(), (76, 175, 80));

        let tooltip = TrayTooltipSnapshot {
            service_status: TrayServiceStatus::Running,
            routing_name: "Follow rules".to_string(),
            active_profile: "US-Proxy".to_string(),
            speed_display: "2.5 MB/s".to_string(),
        };
        assert_eq!(
            tooltip.format_tooltip(),
            "Status: Running\nRouting: Follow rules\nProfile: US-Proxy\nSpeed: 2.5 MB/s"
        );
    }

    #[test]
    fn an_activation_resolves_to_the_same_typed_intent_on_every_surface() {
        let profiles = vec![TrayProfileEntry {
            name: "Paid".to_string(),
            active: true,
            auto_update_enabled: false,
        }];
        let ctx = TrayEventContext {
            system_proxy: false,
            tun: true,
            autostart: false,
            profiles: &profiles,
        };

        assert_eq!(
            resolve_tray_event_in(&TrayEvent::IconActivated, &ctx),
            Some(TrayIntent::ShowWindow)
        );
        assert_eq!(
            resolve_tray_event_in(
                &TrayEvent::MenuActivated {
                    id: TRAY_ACTION_TOGGLE_SYSTEM_PROXY,
                    payload: None,
                },
                &ctx
            ),
            Some(TrayIntent::SetSystemProxy(true))
        );
        assert_eq!(
            resolve_tray_event_in(
                &TrayEvent::MenuActivated {
                    id: TRAY_ACTION_TOGGLE_TUN,
                    payload: None,
                },
                &ctx
            ),
            Some(TrayIntent::SetTunEnabled(false))
        );
        assert_eq!(
            resolve_tray_event_in(
                &TrayEvent::MenuActivated {
                    id: TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
                    payload: Some("Paid".to_string()),
                },
                &ctx
            ),
            Some(TrayIntent::SetProfileAutoUpdate {
                name: "Paid".to_string(),
                enabled: true,
            })
        );
        assert_eq!(
            resolve_tray_event_in(
                &TrayEvent::MenuActivated {
                    id: TRAY_ACTION_TOGGLE_COLOR_MODE,
                    payload: None,
                },
                &ctx
            ),
            Some(TrayIntent::ToggleIconColorMode)
        );
        // Informational lines never resolve to an intent.
        assert_eq!(
            resolve_tray_event_in(
                &TrayEvent::MenuActivated {
                    id: TRAY_ACTION_INFO_STATUS,
                    payload: None,
                },
                &ctx
            ),
            None
        );
    }
}
