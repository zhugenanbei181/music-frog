//! The headless screenshot forensics seam (capture-bevy pipeline).
//!
//! Three environment knobs, read once by the windowed launcher
//! ([`crate::run`]) and nowhere else, mirror the iced demo's capture
//! contract so the nested-niri screenshot harness can drive this frontend
//! exactly like the iced one:
//!
//! - `INFILTRATOR_BEVY_SKIN=dark|light` — the cold-start appearance. The
//!   pure parser is [`parse_skin`]; the resolved mode seeds both the
//!   shell's [`crate::app::ThemeMode`] mirror and the widget layer's
//!   initial theme so the whole tree starts on one token set.
//! - `INFILTRATOR_BEVY_WINDOW_SIZE=WxH` — the requested window
//!   resolution ([`parse_window_size`]).
//! - `INFILTRATOR_CAPTURE_MARKER=path` — after ~60 rendered frames the
//!   [`CapturePlugin`] writes `CAPTURE_READY page=overview skin=<mode>`
//!   to the file, once. The capture script waits for exactly that line
//!   before binding the niri-IPC screenshot to the window; the frame
//!   count (not a sleep) is what makes the readiness honest. A live
//!   run (`waiting_for_first_snapshot`) additionally holds the marker
//!   until the trend ring holds a drawable sample count — a one-sample
//!   chart plots nothing, so firing that early would ship a screenshot
//!   indistinguishable from the pre-data placeholder.

use std::path::PathBuf;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Local, Res};
use infiltrator_bevy_widgets::theme::LightDark;

use crate::app::ThemeMode;
use crate::controller::PumpSnapshotSeen;
use crate::history::TrafficHistory;
use crate::route::Route;

/// Parse the capture skin knob. Case-insensitive, whitespace-tolerant;
/// anything else is `None` (the launcher falls back to the cold-start
/// dark theme). Pure function — unit-tested below without any env access.
pub fn parse_skin(raw: &str) -> Option<LightDark> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "dark" => Some(LightDark::Dark),
        "light" => Some(LightDark::Light),
        _ => None,
    }
}

/// The capture skin from the environment, if it parses.
pub fn skin_from_env() -> Option<LightDark> {
    std::env::var("INFILTRATOR_BEVY_SKIN")
        .ok()
        .and_then(|raw| parse_skin(&raw))
}

/// Parse `WxH` (pixels, positive). Pure function — unit-tested below.
pub fn parse_window_size(raw: &str) -> Option<(u32, u32)> {
    let (width, height) = raw.trim().split_once('x')?;
    let width: u32 = width.trim().parse().ok()?;
    let height: u32 = height.trim().parse().ok()?;
    (width > 0 && height > 0).then_some((width, height))
}

/// The capture window size from the environment, if it parses.
pub fn window_size_from_env() -> Option<(u32, u32)> {
    std::env::var("INFILTRATOR_BEVY_WINDOW_SIZE")
        .ok()
        .and_then(|raw| parse_window_size(&raw))
}

/// The capture marker path from the environment, if set and non-empty.
pub fn marker_path_from_env() -> Option<PathBuf> {
    std::env::var("INFILTRATOR_CAPTURE_MARKER")
        .ok()
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// Parse a page name into its corresponding route.
pub fn parse_page(raw: &str) -> Option<Route> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "overview" => Some(Route::Overview),
        "proxies" => Some(Route::Proxies),
        "profiles" => Some(Route::Profiles),
        "rules" => Some(Route::Rules),
        "connections" => Some(Route::Connections),
        "logs" => Some(Route::Logs),
        "dns" => Some(Route::Dns),
        "doctor" => Some(Route::Doctor),
        "app_routing" | "approuting" | "app-routing" => Some(Route::AppRouting),
        "sync" => Some(Route::Sync),
        "settings" => Some(Route::Settings),
        _ => None,
    }
}

/// The capture page from the environment, if set.
pub fn page_from_env() -> Option<Route> {
    std::env::var("INFILTRATOR_BEVY_PAGE")
        .ok()
        .and_then(|raw| parse_page(&raw))
}

pub fn route_slug(route: Route) -> &'static str {
    match route {
        Route::Overview => "overview",
        Route::Proxies => "proxies",
        Route::Profiles => "profiles",
        Route::Rules => "rules",
        Route::Connections => "connections",
        Route::Logs => "logs",
        Route::Dns => "dns",
        Route::Doctor => "doctor",
        Route::AppRouting => "app_routing",
        Route::Sync => "sync",
        Route::Settings => "settings",
    }
}

/// The readiness line the capture script waits on. Pure function.
pub fn capture_marker_line(route: Route, skin: LightDark) -> String {
    let skin_str = match skin {
        LightDark::Dark => "dark",
        LightDark::Light => "light",
    };
    let page_str = route_slug(route);
    format!("CAPTURE_READY page={page_str} skin={skin_str}\n")
}

/// Where the marker goes. Injected by the launcher; a resource so the
/// writer system stays a plain function.
#[derive(Resource, Clone, Debug)]
struct CaptureMarkerPath(PathBuf);

/// Whether the readiness marker must wait for the live pump's data: a
/// delivered snapshot plus a drawable trend ring. Demo captures mount no
/// pump, so the gate is a plain bool resource the writer reads.
#[derive(Resource, Clone, Copy, Debug, Default)]
struct WaitForLiveSnapshot(bool);

/// Adds the frame-counted marker writer. The launcher installs this only
/// when `INFILTRATOR_CAPTURE_MARKER` is set — a normal windowed run never
/// touches the filesystem.
///
/// `waiting_for_first_snapshot` (live-controller runs only) hardens the
/// marker's honesty: readiness then requires the page to have received the
/// pump's real data AND a drawable trend (the rate ring's sample count,
/// [`live_trend_ready`]), not just rendered 60 frames of the pre-sample
/// placeholder.
pub struct CapturePlugin {
    marker_path: PathBuf,
    waiting_for_first_snapshot: bool,
}

impl CapturePlugin {
    pub fn new(marker_path: PathBuf) -> Self {
        Self {
            marker_path,
            waiting_for_first_snapshot: false,
        }
    }

    /// Gate `CAPTURE_READY` on [`PumpSnapshotSeen`] (live-controller runs).
    pub fn waiting_for_first_snapshot(mut self) -> Self {
        self.waiting_for_first_snapshot = true;
        self
    }
}

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CaptureMarkerPath(self.marker_path.clone()));
        app.insert_resource(WaitForLiveSnapshot(self.waiting_for_first_snapshot));
        app.add_systems(Update, write_capture_marker);
    }
}

/// Rendered frames to wait before declaring readiness: enough for the
/// scene to mount, fonts to shape and several presented frames to land,
/// still fast enough that the capture budget stays seconds, not minutes.
const CAPTURE_READY_FRAME: u32 = 60;

/// The minimum recorded samples before a LIVE capture may fire. The
/// trend chart draws nothing for a single-sample series (one point has no
/// segment — the widget pins it to the newest edge, chart.rs), so a live
/// screenshot taken one sample in would show an indistinguishable-from-
/// empty plot. Two recorded samples project the first drawable segment;
/// three make the readiness robust against one anomalous tick.
const LIVE_MIN_TREND_SAMPLES: usize = 3;

/// The live run's readiness decision: the pump must have delivered at
/// least one snapshot (never the pre-sample placeholder) *and* the trend
/// ring must hold a drawable sample count. A run without a ring (no pump
/// mounted) passes on the delivered-snapshot latch alone. Pure function.
fn live_trend_ready(snapshot_seen: bool, ring_len: Option<usize>) -> bool {
    if !snapshot_seen {
        return false;
    }
    ring_len.is_none_or(|len| len >= LIVE_MIN_TREND_SAMPLES)
}

/// One-shot readiness writer: count frames with `Local` state, write the
/// marker once past the threshold (and, when gated, once the live pump has
/// delivered enough of a trend), then retire. A failed write retries on
/// the next frame; a successful one never writes again.
#[allow(clippy::too_many_arguments)]
fn write_capture_marker(
    mut frames: Local<u32>,
    mut done: Local<bool>,
    mode: Option<Res<ThemeMode>>,
    seen: Option<Res<PumpSnapshotSeen>>,
    history: Option<Res<TrafficHistory>>,
    active_route: Option<Res<crate::route::ActiveRoute>>,
    gate: Res<WaitForLiveSnapshot>,
    path: Res<CaptureMarkerPath>,
) {
    if *done {
        return;
    }
    *frames += 1;
    if *frames < CAPTURE_READY_FRAME {
        return;
    }
    if gate.0
        && !live_trend_ready(
            seen.is_some_and(|seen| seen.0),
            history.as_ref().map(|history| history.len()),
        )
    {
        return;
    }
    let skin = mode.map(|mode| mode.0).unwrap_or(LightDark::Dark);
    let route = active_route
        .and_then(|r| r.0)
        .or_else(page_from_env)
        .unwrap_or_default();
    if std::fs::write(&path.0, capture_marker_line(route, skin)).is_ok() {
        *done = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_parses_both_modes_and_rejects_junk() {
        assert_eq!(parse_skin("dark"), Some(LightDark::Dark));
        assert_eq!(parse_skin("light"), Some(LightDark::Light));
        assert_eq!(parse_skin(" LIGHT "), Some(LightDark::Light));
        assert_eq!(parse_skin("Dark"), Some(LightDark::Dark));
        assert_eq!(parse_skin("blue"), None);
        assert_eq!(parse_skin(""), None);
    }

    #[test]
    fn window_size_parses_wxh_and_rejects_junk() {
        assert_eq!(parse_window_size("1180x760"), Some((1180, 760)));
        assert_eq!(parse_window_size(" 800x600 "), Some((800, 600)));
        assert_eq!(parse_window_size("1180"), None);
        assert_eq!(parse_window_size("1180x"), None);
        assert_eq!(parse_window_size("x760"), None);
        assert_eq!(parse_window_size("0x100"), None);
        assert_eq!(parse_window_size("-5x100"), None);
        assert_eq!(parse_window_size("axb"), None);
    }

    #[test]
    fn page_parses_all_eleven_routes_and_rejects_junk() {
        assert_eq!(parse_page("overview"), Some(Route::Overview));
        assert_eq!(parse_page("proxies"), Some(Route::Proxies));
        assert_eq!(parse_page("PROFILES"), Some(Route::Profiles));
        assert_eq!(parse_page("rules"), Some(Route::Rules));
        assert_eq!(parse_page("connections"), Some(Route::Connections));
        assert_eq!(parse_page("logs"), Some(Route::Logs));
        assert_eq!(parse_page("dns"), Some(Route::Dns));
        assert_eq!(parse_page("doctor"), Some(Route::Doctor));
        assert_eq!(parse_page("app_routing"), Some(Route::AppRouting));
        assert_eq!(parse_page("app-routing"), Some(Route::AppRouting));
        assert_eq!(parse_page("sync"), Some(Route::Sync));
        assert_eq!(parse_page("settings"), Some(Route::Settings));
        assert_eq!(parse_page("invalid_page"), None);
    }

    #[test]
    fn marker_line_names_the_route_and_skin() {
        assert_eq!(
            capture_marker_line(Route::Overview, LightDark::Dark),
            "CAPTURE_READY page=overview skin=dark\n"
        );
        assert_eq!(
            capture_marker_line(Route::Proxies, LightDark::Light),
            "CAPTURE_READY page=proxies skin=light\n"
        );
    }

    /// The live readiness gate: the delivered-snapshot latch alone is not
    /// enough — a drawable trend (≥ `LIVE_MIN_TREND_SAMPLES` recorded
    /// samples) is required; a run without a ring falls back to the latch.
    #[test]
    fn live_trend_ready_needs_a_drawable_ring() {
        assert!(!live_trend_ready(false, Some(5)), "nothing delivered yet");
        assert!(!live_trend_ready(true, Some(0)), "no samples recorded");
        assert!(!live_trend_ready(true, Some(1)), "one point draws nothing");
        assert!(!live_trend_ready(true, Some(2)));
        assert!(
            live_trend_ready(true, Some(LIVE_MIN_TREND_SAMPLES)),
            "the trend is drawable"
        );
        assert!(
            live_trend_ready(true, None),
            "no ring mounted: the latch decides"
        );
    }
}
