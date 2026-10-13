//! Bevy adapter for the shared 11-page surface snapshot.
//!
//! The only business-facing value crossing into this crate is
//! `infiltrator_contract::surface_snapshot::SurfaceSnapshot`. The page
//! structs remain Bevy-local render projections; these conversion functions
//! are the explicit adapter between the two worlds.

use crate::pages::app_routing::AppRoutingProjection;
use crate::pages::connections::ConnectionsProjection;
use crate::pages::dns::DnsProjection;
use crate::pages::doctor::DoctorProjection;
use crate::pages::logs::LogsProjection;
use crate::pages::profiles::ProfilesProjection;
use crate::pages::proxies::ProxiesProjection;
use crate::pages::rules::RulesProjection;
use crate::pages::settings::settings_core::SettingsProjection;
use crate::pages::sync::SyncProjection;
#[cfg(feature = "capture-tools")]
use crate::projection::OverviewOrigin;
use crate::projection::{OverviewProjection, OverviewSource, SourceKind};
use crate::route::SurfaceSourceHandle;
use bevy::app;
use bevy::app::{Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Res, ResMut};
use bevy::feathers::theme::ThemeTextColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::UiRect;
use bevy::ui::prelude::{BackgroundColor, FlexDirection, Node, Overflow, percent, px};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycleSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[path = "host_insets.rs"]
pub mod host_insets;
#[cfg(any(feature = "capture-tools", test))]
#[path = "surface_demo.rs"]
pub(crate) mod surface_demo;
#[path = "surface_projection.rs"]
mod surface_projection;
#[path = "surface_projection_defaults.rs"]
mod surface_projection_defaults;
#[path = "surface_settings.rs"]
mod surface_settings;

/// A single complete read model update delivered to the Bevy world.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct SurfaceSnapshotUpdated(pub surface_snapshot::SurfaceSnapshot);

/// The latest shared snapshot is a render input/cache, never the Core source
/// of truth. It is kept separately from page-local projection resources so
/// tests can assert revision and status propagation.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct LatestSurfaceSnapshot(pub surface_snapshot::SurfaceSnapshot);

/// Bevy's lifecycle adapter is kept separately from page projections so all
/// scenes can observe the same generation/session fencing data.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct LatestCoreLifecycle(pub CoreLifecycleSnapshot);

pub fn core_lifecycle_projection(
    snapshot: &surface_snapshot::SurfaceSnapshot,
) -> CoreLifecycleSnapshot {
    snapshot.core.lifecycle_snapshot()
}

/// Non-success status banner attached to a page root. Data pages never turn
/// an unavailable or failed source into a visually empty success state.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct SurfaceStatusBanner {
    pub page: surface_snapshot::PageId,
    pub status: surface_snapshot::PageStatus,
}

impl Default for SurfaceStatusBanner {
    fn default() -> Self {
        Self {
            page: surface_snapshot::PageId::Overview,
            status: surface_snapshot::PageStatus::Loading,
        }
    }
}

pub fn status_banner_scene(
    page: surface_snapshot::PageId,
    status: &surface_snapshot::PageStatus,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let (label, fill) = match status {
        surface_snapshot::PageStatus::Loading => (
            LocalizedText::plain("surface_loading"),
            palette.surface_elevated,
        ),
        surface_snapshot::PageStatus::Empty => (
            LocalizedText::plain("surface_empty"),
            palette.surface_elevated,
        ),
        surface_snapshot::PageStatus::Unavailable { failure } => (
            LocalizedText::new(
                "surface_host_unavailable",
                vec![("message", failure.message.clone())],
            ),
            palette.warning,
        ),
        surface_snapshot::PageStatus::Failed { failure } => (
            LocalizedText::new(
                "surface_read_failed",
                vec![("message", failure.message.clone())],
            ),
            palette.danger,
        ),
        surface_snapshot::PageStatus::Ready => (
            LocalizedText::new("common_message", vec![("message", String::new())]),
            palette.surface_elevated,
        ),
    };
    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        min_width: px(0.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(rem(space::S12)),
                        overflow: Overflow::clip(),
                    }
                    SurfaceStatusBanner {
                        page,
                        status: { status.clone() },
                    }
                    BackgroundColor({ fill })
                    Children [
                        LocalizedText { .. { label } } TextRole(Role::Caption) ThemeTextColor({ Token::Ink })
                    ]
        })],
        palette,
    )
}

/// Surface source consumed by routing and page projection systems.
///
/// Extending `OverviewSource` keeps the small existing Overview tests/source
/// seam valid while the complete 11-page contract becomes the canonical path.
pub trait SurfaceSource: OverviewSource {
    fn surface_snapshot(&self) -> surface_snapshot::SurfaceSnapshot;

    /// Record the snapshot accepted by the rendered surface. A host may use
    /// this hook to keep its local read model aligned with the rendered revision.
    fn observe_snapshot(&self, _snapshot: &surface_snapshot::SurfaceSnapshot) {}
}

/// Trait-object adapter used when the router needs the small Overview view
/// seam while the owning resource is the complete surface source.
pub struct SurfaceOverviewAdapter(pub Arc<dyn SurfaceSource>);

impl OverviewSource for SurfaceOverviewAdapter {
    fn current(&self) -> OverviewProjection {
        surface_projection::overview_projection(&self.0.surface_snapshot())
    }

    fn kind(&self) -> SourceKind {
        self.0.kind()
    }
}

/// Periodically read a cheap, cached host surface source into Bevy's normal
/// owner-authored snapshot projection path.
pub struct SurfaceSourceRefreshPlugin {
    interval: Duration,
}

impl SurfaceSourceRefreshPlugin {
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

impl Default for SurfaceSourceRefreshPlugin {
    fn default() -> Self {
        Self::new(Duration::from_millis(500))
    }
}

#[derive(Resource)]
struct SurfaceSourceRefreshState {
    interval: Duration,
    last_read: Option<Instant>,
    last_snapshot: Option<surface_snapshot::SurfaceSnapshot>,
}

impl Plugin for SurfaceSourceRefreshPlugin {
    fn build(&self, app: &mut app::App) {
        app.insert_resource(SurfaceSourceRefreshState {
            interval: self.interval,
            last_read: None,
            last_snapshot: None,
        });
        app.add_systems(Update, refresh_surface_source);
    }
}

fn refresh_surface_source(
    source: Res<SurfaceSourceHandle>,
    latest: Option<Res<LatestSurfaceSnapshot>>,
    mut state: ResMut<SurfaceSourceRefreshState>,
    mut commands: Commands,
) {
    let now = Instant::now();
    if state
        .last_read
        .is_some_and(|last_read| now.duration_since(last_read) < state.interval)
    {
        return;
    }
    state.last_read = Some(now);
    let snapshot = source.0.surface_snapshot();
    let previous = state
        .last_snapshot
        .as_ref()
        .or_else(|| latest.as_ref().map(|latest| &latest.0));
    if previous.is_some_and(|previous| !snapshot.is_newer_than(previous)) {
        return;
    }
    state.last_snapshot = Some(snapshot.clone());
    commands.trigger(SurfaceSnapshotUpdated(snapshot));
}

/// Triggered when the host reports the UI surface is back. The shell must read
/// back one complete snapshot instead of replaying a stale backlog.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostSurfaceRestoreRequested;

/// Read back the composed source's current full snapshot and feed it through
/// the same coalesced projection path the frame drain uses.
pub fn read_back_surface_snapshot(source: &dyn SurfaceSource, commands: &mut Commands) {
    commands.trigger(SurfaceSnapshotUpdated(source.surface_snapshot()));
}

/// Host lifecycle restore hook. `SurfaceSourceHandle` is absent in a
/// pull-only composition, in which case the request is a no-op.
pub(crate) fn on_surface_restore_requested(
    _request: On<HostSurfaceRestoreRequested>,
    source: Option<Res<SurfaceSourceHandle>>,
    mut commands: Commands,
) {
    if let Some(source) = source {
        read_back_surface_snapshot(source.0.as_ref(), &mut commands);
    }
}

/// Legacy Overview-only source adapter. It is retained for existing Overview
/// tests and for hosts that have not yet composed the full surface reader. A
/// live source receives typed unavailable state for pages it did not compose;
/// only an explicitly demo source receives demo page data.
#[cfg(feature = "capture-tools")]
pub struct LegacyOverviewSurfaceSource {
    overview: Arc<dyn OverviewSource>,
}

#[cfg(feature = "capture-tools")]
impl LegacyOverviewSurfaceSource {
    pub fn new(source: impl OverviewSource + 'static) -> Self {
        Self {
            overview: Arc::new(source),
        }
    }

    pub fn from_arc(source: Arc<dyn OverviewSource>) -> Self {
        Self { overview: source }
    }
}

#[cfg(feature = "capture-tools")]
impl SurfaceSource for LegacyOverviewSurfaceSource {
    fn surface_snapshot(&self) -> surface_snapshot::SurfaceSnapshot {
        let overview = self.overview.current();
        match overview.origin {
            #[cfg(feature = "capture-tools")]
            OverviewOrigin::Demo => surface_demo::demo_snapshot_from_overview(&overview),
            OverviewOrigin::LiveCore => surface_demo::snapshot_from_overview(&overview),
        }
    }
}

#[cfg(feature = "capture-tools")]
impl OverviewSource for LegacyOverviewSurfaceSource {
    fn current(&self) -> OverviewProjection {
        self.overview.current()
    }

    fn kind(&self) -> SourceKind {
        self.overview.kind()
    }
}

/// Explicit demo source for screenshot and deterministic UI tests.
#[cfg(any(feature = "capture-tools", test))]
pub struct DemoSurfaceSource {
    snapshot: surface_snapshot::SurfaceSnapshot,
}

#[cfg(any(feature = "capture-tools", test))]
impl DemoSurfaceSource {
    pub fn running() -> Self {
        Self {
            snapshot: surface_demo::demo_snapshot(),
        }
    }
}

#[cfg(any(feature = "capture-tools", test))]
impl SurfaceSource for DemoSurfaceSource {
    fn surface_snapshot(&self) -> surface_snapshot::SurfaceSnapshot {
        self.snapshot.clone()
    }
}

#[cfg(any(feature = "capture-tools", test))]
impl OverviewSource for DemoSurfaceSource {
    fn current(&self) -> OverviewProjection {
        surface_projection::overview_projection(&self.snapshot)
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Demo
    }
}

/// Default non-demo source. It is intentionally honest: a host that only
/// mounts the UI shell without a composed application sees unavailable state,
/// not fabricated proxy/profile data.
pub struct UnavailableSurfaceSource {
    snapshot: surface_snapshot::SurfaceSnapshot,
}

impl UnavailableSurfaceSource {
    pub fn new(surface: SurfaceKind, host: HostKind, failure: Failure) -> Self {
        Self {
            snapshot: surface_snapshot::SurfaceSnapshot::unavailable(
                surface,
                host,
                failure.clone(),
            ),
        }
    }
}

impl Default for UnavailableSurfaceSource {
    fn default() -> Self {
        Self::new(
            SurfaceKind::BevyDesktop,
            HostKind::Desktop,
            Failure::new(
                ErrorCode::NotReady,
                "surface application source is not composed",
                true,
            ),
        )
    }
}

impl SurfaceSource for UnavailableSurfaceSource {
    fn surface_snapshot(&self) -> surface_snapshot::SurfaceSnapshot {
        self.snapshot.clone()
    }
}

impl OverviewSource for UnavailableSurfaceSource {
    fn current(&self) -> OverviewProjection {
        surface_projection::overview_projection(&self.snapshot)
    }

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }
}

pub fn overview_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> OverviewProjection {
    surface_projection::overview_projection(snapshot)
}

pub fn proxies_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> ProxiesProjection {
    surface_projection::proxies_projection(snapshot)
}

pub fn profiles_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> ProfilesProjection {
    surface_projection::profiles_projection(snapshot)
}

pub fn rules_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> RulesProjection {
    surface_projection::rules_projection(snapshot)
}

pub fn connections_projection(
    snapshot: &surface_snapshot::SurfaceSnapshot,
) -> ConnectionsProjection {
    surface_projection::connections_projection(snapshot)
}

pub fn logs_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> LogsProjection {
    surface_projection::logs_projection(snapshot)
}

pub fn dns_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> DnsProjection {
    surface_projection::dns_projection(snapshot)
}

pub fn doctor_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> DoctorProjection {
    surface_projection::doctor_projection(snapshot)
}

pub fn app_routing_projection(
    snapshot: &surface_snapshot::SurfaceSnapshot,
) -> AppRoutingProjection {
    surface_projection::app_routing_projection(snapshot)
}

pub fn sync_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> SyncProjection {
    surface_projection::sync_projection(snapshot)
}

pub fn settings_projection(snapshot: &surface_snapshot::SurfaceSnapshot) -> SettingsProjection {
    surface_projection::settings_projection(snapshot)
}
