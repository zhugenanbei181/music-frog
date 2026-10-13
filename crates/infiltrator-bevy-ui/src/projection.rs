//! The Overview data seam: a pure projection of the mihomo core's run
//! state, plus the source trait the shell reads it through.
//!
//! [`OverviewProjection`] and the [`OverviewSource`] trait are the frontend's
//! local projection seam (BEVY-005,
//! first slice): pages render whatever a source hands them, so the UI can
//! be exercised headless against the demo fixture and, later, against the
//! real core without a single scene change.
//!
//! **Honesty note:** the capture-only demo fixture and the owner-backed
//! product source (plus the optional capture controller source) are all plain
//! [`OverviewSource`] implementations behind the same seam. Which one a
//! projection came from is typed data — [`OverviewOrigin`] — so pages can
//! stay honest about what feeds them (the demo banner says 演示数据, the
//! product source names the real core version it observed, and fixture-only
//! controls stay out of product builds).
//!
//! The typed **success / failure / unavailable tri-state** required by
//! crates/infiltrator-contract/docs/UI_PARITY_AUDIT.md lives in [`OverviewState`]: `Running` and `Stopped`
//! are successful reads of a reachable core, `Unavailable` carries a
//! human-readable [`OverviewProjection::failure`] reason. A projection
//! never fabricates zeros for a core it could not reach — rates and
//! connection counts are only meaningful in a successful state.

use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::overview_layout::OverviewLayoutSnapshot;
use infiltrator_contract::public_ip::PublicIpProbeSnapshot;
use infiltrator_contract::reconnect_mask::ReconnectMaskSnapshot;
use infiltrator_contract::resources::ResourceAvailability;
use infiltrator_contract::responsive_viewport::ResponsiveViewportSnapshot;
use infiltrator_contract::routing_intent::RoutingIntentSnapshot;
use infiltrator_contract::shell_readout::ShellReadoutSnapshot;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::speedtest::SpeedtestSnapshot;
use infiltrator_contract::subscription_quota::SubscriptionQuotaSnapshot;
use infiltrator_contract::system_toggle::SystemToggleSnapshot;
use infiltrator_contract::traffic_scale::TrafficScaleSnapshot;
use infiltrator_contract::traffic_topology::TrafficTopologySnapshot;
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;
#[cfg(any(feature = "capture-tools", test))]
use infiltrator_presentation::shell_readout::observed_rate;
#[cfg(any(feature = "capture-tools", test))]
use std::sync::Arc;
#[cfg(any(feature = "capture-tools", test))]
use std::sync::atomic::{AtomicU8, Ordering};
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewOrigin {
    /// The bundled demo fixture — nothing here touched a real core.
    #[cfg(any(feature = "capture-tools", test))]
    Demo,
    /// The owner-backed product surface sampled a real core.
    LiveCore,
}

/// Which kind of [`OverviewSource`] is mounted behind the page. Known at
/// plugin-assembly time and constant for the source's lifetime — chrome like the
/// sidebar foot reads it instead of sniffing individual projections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    /// The bundled demo fixture.
    #[cfg(any(feature = "capture-tools", test))]
    Demo,
    /// The owner-backed product surface.
    LiveCore,
}

/// One read-only snapshot of what the Overview page shows. Pure data —
/// zero bevy, zero transport — so it is headless-testable and portable to
/// any surface that consumes the shared seam.
#[derive(Clone, Debug, PartialEq)]
pub struct OverviewProjection {
    pub lifecycle: CoreLifecycle,
    pub readout: ShellReadoutSnapshot,
    /// The typed run-state this snapshot encodes.
    pub state: OverviewState,
    /// Uplink throughput in bytes per second (0.0 when not running).
    pub upload_bps: f64,
    /// Downlink throughput in bytes per second (0.0 when not running).
    pub download_bps: f64,
    /// Number of active connections (0 when not running).
    pub active_connections: u32,
    /// Core process memory in bytes with support and failure kept explicit.
    pub memory_bytes: ResourceAvailability<u64>,
    /// When the source took this sample (its own monotonic clock origin).
    pub sampled_at: Duration,
    /// Why the core is unavailable — `Some` exactly when
    /// [`OverviewState::Unavailable`], `None` otherwise.
    pub failure: Option<String>,
    /// Which source produced this snapshot (capture fixture vs owner-backed
    /// product surface).
    pub origin: OverviewOrigin,
    /// The core's self-reported version string (`GET /version`), once the
    /// product owner has read it. The demo fixture carries `None`; the banner
    /// shows the real version only when a real core reported one.
    pub core_version: Option<String>,
    /// Application-owned live samples used by the shared waveform renderer.
    pub traffic_waveform: TrafficWaveformSnapshot,
    /// Shared dynamic max/unit/tick scale for the waveform.
    pub traffic_scale: TrafficScaleSnapshot,
    /// Shared live routing chain for the Overview topology card.
    pub traffic_topology: TrafficTopologySnapshot,
    /// Shared selected outbound node for the Overview exit card.
    pub active_exit: ActiveExitSnapshot,
    pub public_ip: PublicIpProbeSnapshot,
    pub layout: OverviewLayoutSnapshot,
    pub reconnect_mask: ReconnectMaskSnapshot,
    pub viewport: ResponsiveViewportSnapshot,
    /// Shared active subscription quota dashboard.
    pub subscription_quota: SubscriptionQuotaSnapshot,
    /// Shared system proxy/TUN state used by Overview master controls.
    pub system_toggles: SystemToggleSnapshot,
    /// Shared routing intent snapshot used by Overview policy controls.
    pub routing_intent: RoutingIntentSnapshot,
    /// Core-owned speedtest snapshot driving the Overview speedtest button.
    pub speedtest: SpeedtestSnapshot,
    /// CPU utilization percentage, if observed.
    pub cpu_percent: ResourceAvailability<f32>,
    /// Cumulative session traffic in bytes, if observed.
    pub total_traffic_bytes: Option<u64>,
}

impl OverviewProjection {
    /// The typed unavailable projection: zeroed metrics, the reason riding
    /// [`OverviewProjection::failure`]. A convenience for sources — the
    /// page renders it through the same tri-state as any other snapshot.
    pub fn unavailable(origin: OverviewOrigin, reason: impl Into<String>) -> Self {
        Self {
            state: OverviewState::Unavailable,
            lifecycle: CoreLifecycle::Failed,
            readout: Default::default(),
            upload_bps: 0.0,
            download_bps: 0.0,
            active_connections: 0,
            memory_bytes: ResourceAvailability::NotObserved,
            sampled_at: Duration::ZERO,
            failure: Some(reason.into()),
            origin,
            core_version: None,
            traffic_waveform: Default::default(),
            traffic_scale: Default::default(),
            traffic_topology: Default::default(),
            active_exit: Default::default(),
            public_ip: Default::default(),
            layout: Default::default(),
            reconnect_mask: Default::default(),
            viewport: Default::default(),
            subscription_quota: Default::default(),
            system_toggles: Default::default(),
            routing_intent: Default::default(),
            speedtest: Default::default(),
            cpu_percent: ResourceAvailability::NotObserved,
            total_traffic_bytes: None,
        }
    }
}

/// Where the Overview page reads its projection from. Commands use the
/// shared command sink; this trait only supplies a read projection.
pub trait OverviewSource: Send + Sync {
    /// The current snapshot. Cheap and side-effect free.
    fn current(&self) -> OverviewProjection;

    /// Which kind of source this is — constant for the implementation's
    /// lifetime. Product sources report LiveCore; capture sources report Demo.
    /// Chrome that must reflect the data source reads this, not probes.
    fn kind(&self) -> SourceKind;
}

/// The capture-only demo fixture source: fixed, believable numbers in the spirit of
/// the iced demo fixtures (isolated from any real transport — nothing in
/// this type can touch the network). The state is switchable so tests can
/// exercise all three projections of the tri-state through the same seam the
/// real source plugs into.
#[cfg(any(feature = "capture-tools", test))]
#[derive(Clone, Debug)]
pub struct DemoOverviewSource {
    state: Arc<AtomicU8>,
}

/// Uplink fixture: ~1.4 MiB/s.
#[cfg(any(feature = "capture-tools", test))]
const DEMO_UPLOAD_BPS: f64 = 1_468_006.3;
/// Downlink fixture: ~8.6 MiB/s.
#[cfg(any(feature = "capture-tools", test))]
const DEMO_DOWNLOAD_BPS: f64 = 9_018_431.5;
/// Connection fixture: a believable idle-plus-browsing count.
#[cfg(any(feature = "capture-tools", test))]
const DEMO_CONNECTIONS: u32 = 12;
/// Memory fixture: 96 MiB — a plausible mihomo RSS on a small box.
#[cfg(any(feature = "capture-tools", test))]
const DEMO_MEMORY_BYTES: u64 = 96 * 1024 * 1024;
/// Sample fixture: 42 s into the source's (fake) clock.
#[cfg(any(feature = "capture-tools", test))]
const DEMO_SAMPLE: Duration = Duration::from_secs(42);
/// Why the demo core is unavailable when it is.
#[cfg(any(feature = "capture-tools", test))]
const DEMO_UNAVAILABLE_REASON: &str = "demo: external controller unreachable (connection refused)";

#[cfg(any(feature = "capture-tools", test))]
impl DemoOverviewSource {
    /// The default fixture: a running core with live-looking traffic.
    pub fn running() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(OverviewState::Running as u8)),
        }
    }

    /// The stopped fixture: reachable core, no traffic.
    pub fn stopped() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(OverviewState::Stopped as u8)),
        }
    }

    /// The unavailable fixture: the failure projection.
    pub fn unavailable() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(OverviewState::Unavailable as u8)),
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

#[cfg(any(feature = "capture-tools", test))]
impl OverviewSource for DemoOverviewSource {
    fn current(&self) -> OverviewProjection {
        let state = self.state_value();
        let (
            upload_bps,
            download_bps,
            active_connections,
            memory_bytes,
            cpu_percent,
            total_traffic_bytes,
        ) = match state {
            OverviewState::Running => (
                DEMO_UPLOAD_BPS,
                DEMO_DOWNLOAD_BPS,
                DEMO_CONNECTIONS,
                ResourceAvailability::Available(DEMO_MEMORY_BYTES),
                ResourceAvailability::Available(2.4),
                Some(1024 * 1024 * 1024 * 12 + 1024 * 1024 * 512),
            ),
            OverviewState::Stopped | OverviewState::Unavailable => (
                0.0,
                0.0,
                0,
                ResourceAvailability::NotObserved,
                ResourceAvailability::NotObserved,
                None,
            ),
        };
        let (upload_observation, download_observation) = match state {
            OverviewState::Running => (
                observed_rate(Some(upload_bps)),
                observed_rate(Some(download_bps)),
            ),
            OverviewState::Stopped | OverviewState::Unavailable => {
                (observed_rate(None), observed_rate(None))
            }
        };
        OverviewProjection {
            lifecycle: match state {
                OverviewState::Running => CoreLifecycle::Running,
                OverviewState::Stopped => CoreLifecycle::Stopped,
                OverviewState::Unavailable => CoreLifecycle::Failed,
            },
            readout: ShellReadoutSnapshot {
                upload_bps: upload_observation,
                download_bps: download_observation,
                ..Default::default()
            },
            state,
            upload_bps,
            download_bps,
            active_connections,
            memory_bytes,
            sampled_at: DEMO_SAMPLE,
            failure: match state {
                OverviewState::Unavailable => Some(DEMO_UNAVAILABLE_REASON.to_string()),
                _ => None,
            },
            origin: OverviewOrigin::Demo,
            core_version: None,
            traffic_waveform: Default::default(),
            traffic_scale: Default::default(),
            traffic_topology: TrafficTopologySnapshot::demo_fixture(),
            active_exit: ActiveExitSnapshot::demo_fixture(),
            public_ip: PublicIpProbeSnapshot::demo_fixture(),
            layout: Default::default(),
            reconnect_mask: Default::default(),
            viewport: Default::default(),
            subscription_quota: SubscriptionQuotaSnapshot::demo_fixture(),
            system_toggles: SystemToggleSnapshot::from_legacy(true, Some(false), 1),
            routing_intent: RoutingIntentSnapshot::demo_fixture(),
            speedtest: SpeedtestSnapshot::demo_fixture(),
            cpu_percent,
            total_traffic_bytes,
        }
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Demo
    }
}
