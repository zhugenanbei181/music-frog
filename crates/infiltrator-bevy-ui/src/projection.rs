//! The Overview data seam: a pure projection of the mihomo core's run
//! state, plus the source trait the shell reads it through.
//!
//! [`OverviewProjection`] and the [`OverviewSource`] trait are the frontend's
//! local projection seam (BEVY-005,
//! first slice): pages render whatever a source hands them, so the UI can
//! be exercised headless against the demo fixture and, later, against the
//! real core without a single scene change.
//!
//! **Honesty note:** the demo fixture and the live mihomo pump
//! ([`crate::controller::MihomoOverviewSource`]) are both plain
//! [`OverviewSource`] implementations behind the same seam. Which one a
//! projection came from is typed data — [`OverviewOrigin`] — so pages can
//! stay honest about what feeds them (the demo banner says 演示数据, a live
//! core names its real version, and the stop button only exists where a
//! demo source supplies it).
//!
//! The typed **success / failure / unavailable tri-state** required by
//! docs/FRONTENDS.md lives in [`OverviewState`]: `Running` and `Stopped`
//! are successful reads of a reachable core, `Unavailable` carries a
//! human-readable [`OverviewProjection::failure`] reason. A projection
//! never fabricates zeros for a core it could not reach — rates and
//! connection counts are only meaningful in a successful state.

use infiltrator_contract::command::ProxyMode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

/// The typed tri-state of an Overview read (FRONTENDS matrix: success /
/// failure / unavailable must all be visible projections).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewState {
    /// The core is reachable and proxying traffic.
    #[default]
    Running,
    /// The core is reachable but not proxying (stopped / not started).
    Stopped,
    /// The core could not be read at all (unreachable, refused, auth…).
    /// The reason rides [`OverviewProjection::failure`].
    Unavailable,
}

/// Where a projection came from. Typed, not inferred: a page can only stay
/// honest about "演示数据" vs a live core when the source declares itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewOrigin {
    /// The bundled demo fixture — nothing here touched a real core.
    #[default]
    Demo,
    /// The live mihomo controller pump sampled a real core.
    LiveCore,
}

/// Which kind of [`OverviewSource`] is mounted behind the page. Known at
/// plugin-assembly time (BEVY-005: the launcher picks the demo fixture or
/// the live pump) and constant for the source's lifetime — chrome like the
/// sidebar foot reads it instead of sniffing individual projections.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceKind {
    /// The bundled demo fixture.
    #[default]
    Demo,
    /// The live mihomo controller pump.
    LiveCore,
}

/// One read-only snapshot of what the Overview page shows. Pure data —
/// zero bevy, zero transport — so it is headless-testable and portable to
/// any surface that consumes the shared seam.
#[derive(Clone, Debug, PartialEq)]
pub struct OverviewProjection {
    /// The typed run-state this snapshot encodes.
    pub state: OverviewState,
    /// The proxy mode reported by the core (last known when unavailable).
    pub mode: ProxyMode,
    /// Uplink throughput in bytes per second (0.0 when not running).
    pub upload_bps: f64,
    /// Downlink throughput in bytes per second (0.0 when not running).
    pub download_bps: f64,
    /// Number of active connections (0 when not running).
    pub active_connections: u32,
    /// Core process memory in bytes, when the core reported it (`None`
    /// means "not known right now" — the page shows an honest placeholder,
    /// never a fabricated number).
    pub memory_bytes: Option<u64>,
    /// When the source took this sample (its own monotonic clock origin).
    pub sampled_at: Duration,
    /// Why the core is unavailable — `Some` exactly when
    /// [`OverviewState::Unavailable`], `None` otherwise.
    pub failure: Option<String>,
    /// Which source produced this snapshot (BEVY-005: the demo fixture vs
    /// the live mihomo controller pump).
    pub origin: OverviewOrigin,
    /// The core's self-reported version string (`GET /version`), once the
    /// live pump has read it. The demo fixture carries `None`; the banner
    /// shows the real version only when a real core reported one.
    pub core_version: Option<String>,
}

impl OverviewProjection {
    /// The typed unavailable projection: zeroed metrics, the reason riding
    /// [`OverviewProjection::failure`]. A convenience for sources — the
    /// page renders it through the same tri-state as any other snapshot.
    pub fn unavailable(origin: OverviewOrigin, reason: impl Into<String>) -> Self {
        Self {
            state: OverviewState::Unavailable,
            mode: ProxyMode::default(),
            upload_bps: 0.0,
            download_bps: 0.0,
            active_connections: 0,
            memory_bytes: None,
            sampled_at: Duration::ZERO,
            failure: Some(reason.into()),
            origin,
            core_version: None,
        }
    }

    /// The failure text the page should show: the reason when
    /// unavailable, an empty string otherwise (the row stays mounted and
    /// simply carries no copy — never a fabricated status).
    pub fn failure_text(&self) -> &str {
        match (&self.state, self.failure.as_deref()) {
            (OverviewState::Unavailable, Some(reason)) if !reason.trim().is_empty() => reason,
            (OverviewState::Unavailable, _) => "core unavailable",
            _ => "",
        }
    }
}

/// Where the Overview page reads its projection from. The shell holds one
/// boxed implementation as a resource; tests inject their own. Two access
/// paths, one trait:
///
/// - [`OverviewSource::current`] — the pull path. Cheap, side-effect free:
///   the last known snapshot (the demo fixture computes it, the live pump
///   hands back what its background thread last stored).
/// - [`OverviewSource::set_mode`] — the command path (BEVY-005 mode
///   switching). Send-only: the implementation may apply the command
///   synchronously (demo fixture) or forward it to a background thread
///   (live pump); either way the typed receipt rides the caller's
///   `std::sync::mpsc` channel. The default implementation refuses, so
///   read-only sources stay one-method traits.
pub trait OverviewSource: Send + Sync {
    /// The current snapshot. Cheap and side-effect free.
    fn current(&self) -> OverviewProjection;

    /// Which kind of source this is — constant for the implementation's
    /// lifetime (the demo fixture default; the live pump overrides it).
    /// Chrome that must reflect the data source reads this, not probes.
    fn kind(&self) -> SourceKind {
        SourceKind::Demo
    }

    /// Request a proxy-mode switch. The implementation sends exactly one
    /// receipt on `ack`: `Ok(())` once the mode is applied (the next
    /// [`OverviewProjection`] read reflects it or will shortly), or
    /// `Err(reason)` when the core refused / the command could not be
    /// delivered. Dropping `ack` without sending is a bug — the UI's
    /// in-flight latch would never clear.
    fn set_mode(&self, _mode: ProxyMode, ack: Sender<Result<(), String>>) {
        let _ = ack.send(Err("此数据源不支持模式切换".to_owned()));
    }
}

/// The demo fixture source: fixed, believable numbers in the spirit of
/// the iced demo fixtures (isolated from any real transport — nothing in
/// this type can touch the network). State and mode live in atomics (Arc
/// because this toolchain's `Atomic<u8>` is not `Clone`) so `set_mode` can
/// flip the fixture through the shared `&self` seam; the state is
/// switchable so tests can exercise all three projections of the tri-state
/// through the same seam the real source plugs into.
#[derive(Clone, Debug)]
pub struct DemoOverviewSource {
    state: Arc<AtomicU8>,
    mode: Arc<AtomicU8>,
}

/// Uplink fixture: ~1.4 MiB/s.
const DEMO_UPLOAD_BPS: f64 = 1_468_006.3;
/// Downlink fixture: ~8.6 MiB/s.
const DEMO_DOWNLOAD_BPS: f64 = 9_018_431.5;
/// Connection fixture: a believable idle-plus-browsing count.
const DEMO_CONNECTIONS: u32 = 12;
/// Memory fixture: 96 MiB — a plausible mihomo RSS on a small box.
const DEMO_MEMORY_BYTES: u64 = 96 * 1024 * 1024;
/// Sample fixture: 42 s into the source's (fake) clock.
const DEMO_SAMPLE: Duration = Duration::from_secs(42);
/// Why the demo core is unavailable when it is.
const DEMO_UNAVAILABLE_REASON: &str = "demo: external controller unreachable (connection refused)";

impl DemoOverviewSource {
    /// The default fixture: a running core with live-looking traffic.
    pub fn running() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(OverviewState::Running as u8)),
            mode: Arc::new(AtomicU8::new(ProxyMode::default().to_index())),
        }
    }

    /// The stopped fixture: reachable core, no traffic.
    pub fn stopped() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(OverviewState::Stopped as u8)),
            mode: Arc::new(AtomicU8::new(ProxyMode::default().to_index())),
        }
    }

    /// The unavailable fixture: the failure projection.
    pub fn unavailable() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(OverviewState::Unavailable as u8)),
            mode: Arc::new(AtomicU8::new(ProxyMode::default().to_index())),
        }
    }

    /// Switch the fixture's state (the test seam for the tri-state).
    pub fn set_state(&mut self, state: OverviewState) {
        self.state.store(state as u8, Ordering::Relaxed);
    }

    /// The fixture's current state, decoded from its atomic.
    fn state_value(&self) -> OverviewState {
        match self.state.load(Ordering::Relaxed) {
            1 => OverviewState::Stopped,
            2 => OverviewState::Unavailable,
            _ => OverviewState::Running,
        }
    }
}

impl OverviewSource for DemoOverviewSource {
    fn current(&self) -> OverviewProjection {
        let state = self.state_value();
        let mode = ProxyMode::from_index(self.mode.load(Ordering::Relaxed));
        let live = state == OverviewState::Running;
        OverviewProjection {
            state,
            mode,
            upload_bps: if live { DEMO_UPLOAD_BPS } else { 0.0 },
            download_bps: if live { DEMO_DOWNLOAD_BPS } else { 0.0 },
            active_connections: if live { DEMO_CONNECTIONS } else { 0 },
            memory_bytes: live.then_some(DEMO_MEMORY_BYTES),
            sampled_at: DEMO_SAMPLE,
            failure: match state {
                OverviewState::Unavailable => Some(DEMO_UNAVAILABLE_REASON.to_string()),
                _ => None,
            },
            origin: OverviewOrigin::Demo,
            core_version: None,
        }
    }

    /// Flip the fixture's mode and acknowledge immediately — the demo has
    /// no controller to refuse, so the receipt is always `Ok` and the very
    /// next [`OverviewSource::current`] read reports the new mode.
    fn set_mode(&self, mode: ProxyMode, ack: Sender<Result<(), String>>) {
        self.mode.store(mode.to_index(), Ordering::Relaxed);
        let _ = ack.send(Ok(()));
    }
}
