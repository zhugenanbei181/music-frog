//! Bevy adapter for the shared 11-page surface snapshot.
//!
//! The only business-facing value crossing into this crate is
//! `infiltrator_contract::surface_snapshot::SurfaceSnapshot`. The page
//! structs remain Bevy-local render projections; these conversion functions
//! are the explicit adapter between the two worlds.

use bevy::app::{Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{BackgroundColor, FlexDirection, Node, Overflow, Val, percent, px};
use bevy::ui::widget::Text;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use crate::pages::app_routing::AppRoutingProjection;
use crate::pages::connections::ConnectionsProjection;
use crate::pages::dns::DnsProjection;
use crate::pages::doctor::DoctorProjection;
use crate::pages::logs::LogsProjection;
use crate::pages::profiles::ProfilesProjection;
use crate::pages::proxies::ProxiesProjection;
use crate::pages::rules::RulesProjection;
use crate::pages::settings::SettingsProjection;
use crate::pages::sync::SyncProjection;
use crate::projection::{OverviewOrigin, OverviewProjection, OverviewSource, SourceKind};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

#[path = "surface_demo.rs"]
mod surface_demo;
#[path = "surface_projection.rs"]
mod surface_projection;

/// A single complete read model update delivered to the Bevy world.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct SurfaceSnapshotUpdated(pub surface_snapshot::SurfaceSnapshot);

/// The latest shared snapshot is a render input/cache, never the Core source
/// of truth. It is kept separately from page-local projection resources so
/// tests can assert revision and status propagation.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct LatestSurfaceSnapshot(pub surface_snapshot::SurfaceSnapshot);

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
        surface_snapshot::PageStatus::Loading => {
            ("正在加载共享数据…".to_owned(), palette.surface_elevated)
        }
        surface_snapshot::PageStatus::Empty => {
            ("当前没有可展示的数据".to_owned(), palette.surface_elevated)
        }
        surface_snapshot::PageStatus::Unavailable { failure } => (
            format!("当前宿主不可用 · {}", failure.message),
            palette.warning,
        ),
        surface_snapshot::PageStatus::Failed { failure } => {
            (format!("读取失败 · {}", failure.message), palette.danger)
        }
        surface_snapshot::PageStatus::Ready => (String::new(), palette.surface_elevated),
    };
    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                flex_direction: FlexDirection::Column,
                padding: bevy::ui::UiRect::all(Val::Px(space::S12)),
                overflow: Overflow::clip(),
            }
            SurfaceStatusBanner {
                page,
                status: { status.clone() },
            }
            BackgroundColor({ fill })
            Children [
                ( Text({ label }) TextRole(Role::Caption) TextColor({ palette.ink }) ),
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

    fn set_mode(&self, mode: ProxyMode, ack: Sender<Result<(), String>>) {
        self.0.set_mode(mode, ack);
    }
}

/// A full source backed by the application-owned surface pump and the
/// existing Overview command bridge.
#[derive(Clone)]
pub struct ApplicationSurfaceSource {
    pump: SurfacePump,
    overview: Arc<dyn OverviewSource>,
}

impl ApplicationSurfaceSource {
    pub fn new(pump: SurfacePump, overview: Arc<dyn OverviewSource>) -> Self {
        Self { pump, overview }
    }

    pub fn pump(&self) -> &SurfacePump {
        &self.pump
    }
}

impl OverviewSource for ApplicationSurfaceSource {
    fn current(&self) -> OverviewProjection {
        surface_projection::overview_projection(&self.pump.current())
    }

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }

    fn set_mode(&self, mode: ProxyMode, ack: Sender<Result<(), String>>) {
        self.overview.set_mode(mode, ack);
    }
}

impl SurfaceSource for ApplicationSurfaceSource {
    fn surface_snapshot(&self) -> surface_snapshot::SurfaceSnapshot {
        self.pump.current()
    }
}

/// Pump-to-Bevy bridge. The source owns the worker; this plugin only drains
/// bounded snapshots on the Bevy update schedule and emits one coalesced
/// contract event for the page projection layer.
pub struct SurfaceDrainPlugin {
    bridge: infiltrator_application::surface_application::SurfacePumpBridge,
}

impl SurfaceDrainPlugin {
    pub fn new(source: &ApplicationSurfaceSource) -> Self {
        Self {
            bridge: source.pump().bridge(),
        }
    }
}

#[derive(Resource, Clone)]
struct SurfaceBridge(pub infiltrator_application::surface_application::SurfacePumpBridge);

impl Plugin for SurfaceDrainPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_resource(SurfaceBridge(self.bridge.clone()));
        app.add_systems(Update, drain_surface);
    }
}

fn drain_surface(bridge: Res<SurfaceBridge>, mut commands: Commands) {
    if let Some(snapshot) = bridge
        .0
        .drain_events()
        .into_iter()
        .map(|event| match event {
            surface_snapshot::SurfaceEvent::SnapshotUpdated(snapshot) => snapshot,
        })
        .next_back()
    {
        commands.trigger(SurfaceSnapshotUpdated(snapshot));
    }
}

/// Legacy Overview-only source adapter. It is retained for existing Overview
/// tests and for hosts that have not yet composed the full surface reader. A
/// live source receives typed unavailable state for pages it did not compose;
/// only an explicitly demo source receives demo page data.
pub struct LegacyOverviewSurfaceSource {
    overview: Arc<dyn OverviewSource>,
}

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

impl SurfaceSource for LegacyOverviewSurfaceSource {
    fn surface_snapshot(&self) -> surface_snapshot::SurfaceSnapshot {
        let overview = self.overview.current();
        surface_demo::snapshot_from_overview(&overview, overview.origin == OverviewOrigin::Demo)
    }
}

impl OverviewSource for LegacyOverviewSurfaceSource {
    fn current(&self) -> OverviewProjection {
        self.overview.current()
    }

    fn kind(&self) -> SourceKind {
        self.overview.kind()
    }

    fn set_mode(&self, mode: ProxyMode, ack: Sender<Result<(), String>>) {
        self.overview.set_mode(mode, ack);
    }
}

/// Explicit demo source for screenshot and deterministic UI tests.
pub struct DemoSurfaceSource {
    snapshot: surface_snapshot::SurfaceSnapshot,
}

impl DemoSurfaceSource {
    pub fn running() -> Self {
        Self {
            snapshot: surface_demo::demo_snapshot(),
        }
    }
}

impl SurfaceSource for DemoSurfaceSource {
    fn surface_snapshot(&self) -> surface_snapshot::SurfaceSnapshot {
        self.snapshot.clone()
    }
}

impl OverviewSource for DemoSurfaceSource {
    fn current(&self) -> OverviewProjection {
        surface_projection::overview_projection(&self.snapshot)
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Demo
    }

    fn set_mode(&self, _mode: ProxyMode, ack: Sender<Result<(), String>>) {
        let _ = ack.send(Err("演示数据源不支持共享核心命令".to_owned()));
    }
}

/// Default non-demo source. It is intentionally honest: a host that only
/// mounts the UI shell without a composed application sees unavailable state,
/// not fabricated proxy/profile data.
pub struct UnavailableSurfaceSource {
    snapshot: surface_snapshot::SurfaceSnapshot,
}

impl Default for UnavailableSurfaceSource {
    fn default() -> Self {
        Self {
            snapshot: surface_snapshot::SurfaceSnapshot::unavailable(
                SurfaceKind::BevyDesktop,
                HostKind::Desktop,
                infiltrator_contract::error::Failure::new(
                    infiltrator_contract::error::ErrorCode::NotReady,
                    "surface application source is not composed",
                    true,
                ),
            ),
        }
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

    fn set_mode(&self, _mode: ProxyMode, ack: Sender<Result<(), String>>) {
        let _ = ack.send(Err("surface application source is not composed".to_owned()));
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
