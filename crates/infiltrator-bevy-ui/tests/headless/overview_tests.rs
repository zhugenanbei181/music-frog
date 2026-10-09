//! Headless Overview tests: routing over the real shell, the typed
//! tri-state projections, the in-place refresh seam, the injected source
//! decoupling, and the theme-flip reskin of every page surface — on
//! `MinimalPlugins` (no window, no render hardware).

use bevy::MinimalPlugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::asset::{AssetApp, AssetPlugin, Assets};
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::picking::hover::PickingInteraction;
use bevy::scene::ScenePlugin;
use bevy::text::TextColor;
use bevy::ui::BackgroundColor;
use bevy::ui::prelude::{Display, Node};
use bevy::ui::widget::{ImageNode, Text};
use bevy::ui_widgets::Activate;
use infiltrator_application::shell_readout_projection::observed_rate;
use infiltrator_bevy_ui::app::{ContentSlot, ShellPlugin, SidebarFoot};
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::history::{TrafficHistory, demo_traffic_series};
use infiltrator_bevy_ui::pages::overview::{
    CHART_HEIGHT_PX, CHART_WIDTH_PX, OnAccentText, OverviewCardState, OverviewChip,
    OverviewChipKind, OverviewLine, OverviewLineKind, OverviewModeChip, OverviewModePill,
    OverviewProjectionUpdated, OverviewReloadMask, OverviewReloadMaskText, OverviewStatusCard,
    StatusDot, format_memory, format_rate, subscription_quota_scene, topology_chain_scene,
};
use infiltrator_bevy_ui::pages::overview_cards::{
    ActiveExitNodeCard, SystemProxyMasterCard, TunMasterCard,
};
use infiltrator_bevy_ui::pages::overview_lifecycle::CoreControlButton;
use infiltrator_bevy_ui::pages::overview_public_ip::{
    PublicIpProbeCard, PublicIpRefreshButton, PublicIpText, PublicIpTextKind,
};
use infiltrator_bevy_ui::pages::overview_restamp::{
    ActiveExitText, ActiveExitTextKind, OverviewMasterSwitchButton, SubscriptionQuotaCard,
};
use infiltrator_bevy_ui::pages::overview_speedtest::OverviewSpeedtestUrlField;
use infiltrator_bevy_ui::pages::overview_topology::{
    TopologyChainCard, TopologyDrilldownFilter, TopologyStageButton, TopologyText, TopologyTextKind,
};
use infiltrator_bevy_ui::projection::{
    DemoOverviewSource, OverviewOrigin, OverviewProjection, OverviewSource, OverviewState,
    SourceKind,
};
use infiltrator_bevy_ui::route::{PageRoot, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::chart::interaction::compute_instant_rates;
use infiltrator_bevy_widgets::chart::topology::TopologyPlate;
use infiltrator_bevy_widgets::chart::{ChartCrosshairTracked, ChartPlate};
use infiltrator_bevy_widgets::icon::{IconId, IconPlate};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::stat_chip::StatChipValue;
use infiltrator_bevy_widgets::surface::SurfacePanel;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin};
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::proxy_mode::ProxyModeSnapshot;
use infiltrator_contract::shell_readout::ShellReadoutSnapshot;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::traffic_topology::TrafficTopologyStage;
use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};
use std::sync::Arc;
use std::time::Duration;

/// The demo core's unavailability reason (projection.rs fixture).
const DEMO_REASON: &str = "demo: external controller unreachable (connection refused)";

/// A source that is not the demo fixture: distinct, ugly, honest values.
struct StubSource;

impl OverviewSource for StubSource {
    fn current(&self) -> OverviewProjection {
        OverviewProjection {
            state: OverviewState::Running,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 250_000.0,
            download_bps: 4_047.0,
            readout: fixture_readout(250_000.0, 4_047.0),
            active_connections: 3,
            memory_bytes: Some(70 * 1024 * 1024),
            sampled_at: Duration::from_secs(7),
            failure: None,
            origin: OverviewOrigin::Demo,
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
            cpu_percent: None,
            total_traffic_bytes: None,
            proxy_mode: ProxyModeSnapshot {
                current: Some(ProxyMode::Direct),
                ..ProxyModeSnapshot::demo_fixture()
            },
            speedtest: Default::default(),
        }
    }
}

/// The headless composition under test: real shell + real router over
/// `MinimalPlugins` plus the asset/scene singletons `spawn_scene`
/// resolves through, settled with one update. The image store is
/// registered here (the render-backed host does it via its render
/// plugins) so the traffic card's chart rasterizes for real and the
/// write-back path is exercisable — the widgets chart-test idiom.
fn mounted_app_with(source: impl OverviewSource + 'static) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(source));
    app.update();
    app
}

fn mounted_default() -> App {
    mounted_app_with(DemoOverviewSource::running())
}

fn page_root(world: &mut World) -> (Entity, Route) {
    let mut roots = world.query::<(Entity, &PageRoot)>();
    let (id, root) = roots.single(world).expect("exactly one mounted page");
    (id, root.0)
}

fn content_slot(world: &mut World) -> Entity {
    let mut slots = world.query::<(Entity, &ContentSlot)>();
    slots.single(world).expect("content slot").0
}

/// (entity, text content, ink) of one marked Overview line.
fn line(world: &mut World, kind: OverviewLineKind) -> (Entity, String, TextColor) {
    let mut lines = world.query::<(Entity, &OverviewLine, &Text, &TextColor)>();
    lines
        .iter(world)
        .find(|(_, marker, _, _)| marker.0 == kind)
        .map(|(id, _, text, ink)| (id, text.0.clone(), *ink))
        .unwrap_or_else(|| panic!("no {kind:?} line mounted"))
}

/// (entity, text) of one stat chip's marked value.
fn chip_value(world: &mut World, kind: OverviewChipKind) -> (Entity, String) {
    let mut chips = world.query::<(Entity, &OverviewChip)>();
    let (chip_id, _) = chips
        .iter(world)
        .find(|(_, chip)| chip.0 == kind)
        .unwrap_or_else(|| panic!("no {kind:?} chip mounted"));
    let value_id = chip_value_id(world, chip_id);
    let text = world
        .get::<Text>(value_id)
        .unwrap_or_else(|| panic!("no value text under the {kind:?} chip"));
    (value_id, text.0.clone())
}

/// The chip's marked value-text descendant (the checkbox box-lookup
/// idiom, one subtree walk deep because the value sits in the chip's
/// info column).
fn chip_value_id(world: &mut World, chip: Entity) -> Entity {
    let mut stack: Vec<Entity> = Vec::new();
    if let Some(children) = world.get::<Children>(chip) {
        stack.extend(children.iter());
    }
    while let Some(entity) = stack.pop() {
        if world.get::<StatChipValue>(entity).is_some() {
            return entity;
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    panic!("chip {chip:?} carries no value text")
}

/// The chip's marked icon plate (the same subtree walk as the value
/// lookup — the plate sits inside the chip's icon tile).
fn chip_icon_plate(world: &mut World, chip: Entity) -> IconId {
    let mut stack: Vec<Entity> = Vec::new();
    if let Some(children) = world.get::<Children>(chip) {
        stack.extend(children.iter());
    }
    while let Some(entity) = stack.pop() {
        if let Some(plate) = world.get::<IconPlate>(entity) {
            return plate.0;
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    panic!("chip {chip:?} carries no icon plate")
}

/// (entity, fill, stored state) of the status banner.
fn card(world: &mut World) -> (Entity, Color, CoreLifecycle) {
    let mut cards = world.query::<(Entity, &OverviewStatusCard, &BackgroundColor)>();
    let (id, _, fill) = cards.single(world).expect("one status banner");
    let state = world
        .get::<OverviewCardState>(id)
        .and_then(|stored| stored.0.clone())
        .expect("banner stores its state");
    (id, fill.0, state)
}

/// Whether the pill for one proxy mode carries the selected bit.
fn pill_selected(world: &mut World, mode: ProxyMode) -> bool {
    let mut pills = world.query::<(&OverviewModePill, &ControlVisual)>();
    pills
        .iter(world)
        .find(|(pill, _)| pill.0 == mode)
        .expect("mode pill mounted")
        .1
        .0
}

/// Every restampable Overview entity (lines, pills, banner, dot, mode
/// chip, stop button, chips) — the set the refresh seam and the reskin
/// must keep stable.
fn overview_entity_ids(world: &mut World) -> Vec<Entity> {
    let mut ids: Vec<Entity> = Vec::new();
    {
        let mut lines = world.query::<(Entity, &OverviewLine)>();
        ids.extend(lines.iter(world).map(|(id, _)| id));
    }
    {
        let mut pills = world.query::<(Entity, &OverviewModePill)>();
        ids.extend(pills.iter(world).map(|(id, _)| id));
    }
    {
        let mut cards = world.query::<(Entity, &OverviewStatusCard)>();
        ids.extend(cards.iter(world).map(|(id, _)| id));
    }
    {
        let mut dots = world.query::<(Entity, &StatusDot)>();
        ids.extend(dots.iter(world).map(|(id, _)| id));
    }
    {
        let mut chips = world.query::<(Entity, &OverviewChip)>();
        ids.extend(chips.iter(world).map(|(id, _)| id));
    }
    {
        let mut chip_inks = world.query::<(Entity, &OverviewModeChip)>();
        ids.extend(chip_inks.iter(world).map(|(id, _)| id));
    }
    {
        let mut stops = world.query::<(Entity, &CoreControlButton)>();
        ids.extend(stops.iter(world).map(|(id, _)| id));
    }
    {
        let mut charts = world.query::<(Entity, &ChartPlate)>();
        ids.extend(charts.iter(world).map(|(id, _)| id));
    }
    ids.sort();
    ids
}

// ---- routing ----------------------------------------------------------------

// ---- the tri-state projections ----------------------------------------------

// ---- the refresh seam -------------------------------------------------------

// ---- the theme-flip reskin --------------------------------------------------

// ---- pure formatting --------------------------------------------------------

// ---- the theme-switch state-ink replay ---------------------------------------

// ---- the traffic card's trend chart ------------------------------------------

/// A live-origin projection with concrete rates (the chart-seam input).
fn live_projection(upload_bps: f64, download_bps: f64) -> OverviewProjection {
    OverviewProjection {
        state: OverviewState::Running,
        lifecycle: CoreLifecycle::Running,
        upload_bps,
        download_bps,
        readout: fixture_readout(upload_bps, download_bps),
        active_connections: 1,
        memory_bytes: None,
        sampled_at: Duration::from_secs(3),
        failure: None,
        origin: OverviewOrigin::LiveCore,
        core_version: Some("v1.19.18".to_owned()),
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
        cpu_percent: None,
        total_traffic_bytes: None,
        proxy_mode: ProxyModeSnapshot {
            current: Some(ProxyMode::Rule),
            ..ProxyModeSnapshot::demo_fixture()
        },
        speedtest: Default::default(),
    }
}

/// (entity, plate) of the page's one trend chart.
fn chart_plate(world: &mut World) -> (Entity, ChartPlate) {
    let mut plates = world.query::<(Entity, &ChartPlate)>();
    let (id, plate) = plates.single(world).expect("exactly one trend chart");
    (id, plate.clone())
}

/// Every descendant of `root` (the chip value lookup's walk, collect form).
fn descendants(world: &World, root: Entity) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut stack: Vec<Entity> = Vec::new();
    if let Some(children) = world.get::<Children>(root) {
        stack.extend(children.iter());
    }
    while let Some(entity) = stack.pop() {
        out.push(entity);
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    out
}

// ---- the sidebar foot (source kind) ------------------------------------------

/// The sidebar foot caption text.
fn foot_text(world: &mut World) -> String {
    world
        .query::<(&SidebarFoot, &Text)>()
        .iter(world)
        .next()
        .map(|(_, text)| text.0.clone())
        .expect("the sidebar foot is mounted")
}

/// A live-core stub with a configurable self-reported version.
struct LiveFootStub {
    version: Option<&'static str>,
}

impl OverviewSource for LiveFootStub {
    fn current(&self) -> OverviewProjection {
        OverviewProjection {
            state: OverviewState::Running,
            lifecycle: CoreLifecycle::Running,
            upload_bps: 0.0,
            download_bps: 0.0,
            readout: fixture_readout(0.0, 0.0),
            active_connections: 0,
            memory_bytes: None,
            sampled_at: Duration::from_secs(1),
            failure: None,
            origin: OverviewOrigin::LiveCore,
            core_version: self.version.map(str::to_owned),
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
            cpu_percent: None,
            total_traffic_bytes: None,
            proxy_mode: ProxyModeSnapshot {
                current: Some(ProxyMode::Rule),
                ..ProxyModeSnapshot::demo_fixture()
            },
            speedtest: Default::default(),
        }
    }

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }
}

// ---- accessibility semantics --------------------------------------------------

fn native_url_field(world: &mut World) -> Entity {
    let parent = world
        .query::<(Entity, &OverviewSpeedtestUrlField)>()
        .single(world)
        .expect("one native speedtest URL owner")
        .0;
    let field = world
        .get::<Children>(parent)
        .unwrap()
        .iter()
        .copied()
        .find(|entity| world.get::<TextField>(*entity).is_some())
        .expect("the actual controlled URL field");
    assert!(
        world.get::<NativeTextField>(field).is_some(),
        "URL field has a native input owner"
    );
    field
}

#[path = "overview_tests/active.rs"]
mod active;
#[path = "overview_tests/chips.rs"]
mod chips;
#[path = "overview_tests/format.rs"]
mod format;
#[path = "overview_tests/injected.rs"]
mod injected;
#[path = "overview_tests/live.rs"]
mod live;
#[path = "overview_tests/overview_card.rs"]
mod overview_card;
#[path = "overview_tests/overview_master.rs"]
mod overview_master;
#[path = "overview_tests/overview_mode.rs"]
mod overview_mode;
#[path = "overview_tests/overview_mounts.rs"]
mod overview_mounts;
#[path = "overview_tests/overview_page.rs"]
mod overview_page;
#[path = "overview_tests/overview_public.rs"]
mod overview_public;
#[path = "overview_tests/overview_reload.rs"]
mod overview_reload;
#[path = "overview_tests/overview_responsive.rs"]
mod overview_responsive;
#[path = "overview_tests/overview_six.rs"]
mod overview_six;
#[path = "overview_tests/overview_speedtest.rs"]
mod overview_speedtest;
#[path = "overview_tests/overview_topology.rs"]
mod overview_topology;
#[path = "overview_tests/overview_traffic.rs"]
mod overview_traffic;
#[path = "overview_tests/projection.rs"]
mod projection;
#[path = "overview_tests/repeated.rs"]
mod repeated;
#[path = "overview_tests/route.rs"]
mod route;
#[path = "overview_tests/sidebar.rs"]
mod sidebar;
#[path = "overview_tests/stat.rs"]
mod stat;
#[path = "overview_tests/subscription.rs"]
mod subscription;
#[path = "overview_tests/theme.rs"]
mod theme;
#[path = "overview_tests/three.rs"]
mod three;
#[path = "overview_tests/topology.rs"]
mod topology;
#[path = "overview_tests/traffic.rs"]
mod traffic;

#[path = "overview_tests/speedtest_details.rs"]
mod speedtest_details;

fn fixture_readout(upload: f64, download: f64) -> ShellReadoutSnapshot {
    ShellReadoutSnapshot {
        upload_bps: observed_rate(Some(upload)),
        download_bps: observed_rate(Some(download)),
        ..Default::default()
    }
}

#[path = "overview_tests/copy.rs"]
mod copy;

#[test]
fn mounted_mode_toggle_emits_mapped_feedback_and_mute_blocks_it() {
    use bevy::ui_widgets::Activate;
    use infiltrator_bevy_ui::host_capabilities::{HapticsHost, HapticsPort};
    use infiltrator_bevy_ui::pages::overview_cards::OverviewModeSegmentPill;
    use infiltrator_bevy_widgets::haptics::{
        AudioHapticOutput, AudioHapticsSettings, HapticPattern, PcmBuffer,
    };
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recorder {
        haptics: Mutex<Vec<HapticPattern>>,
        tones: Mutex<usize>,
    }
    impl HapticsPort for Recorder {
        fn vibrate(&self, pattern: HapticPattern) {
            self.haptics.lock().expect("haptics").push(pattern);
        }
    }
    impl AudioHapticOutput for Recorder {
        fn play_tone(&self, _pcm: &PcmBuffer) {
            *self.tones.lock().expect("tones") += 1;
        }
    }

    let recorder = Arc::new(Recorder::default());
    let mut app = mounted_default();
    app.insert_resource(HapticsHost::new(recorder.clone(), true).with_audio(recorder.clone()));
    let pill = app
        .world_mut()
        .query::<(Entity, &OverviewModeSegmentPill)>()
        .iter(app.world())
        .find(|(_, pill)| pill.0 == ProxyMode::Global)
        .map(|(entity, _)| entity)
        .expect("the Global mode pill is mounted");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: pill });
    app.update();
    assert_eq!(
        recorder.haptics.lock().expect("haptics").as_slice(),
        &[HapticPattern::LightTick]
    );
    assert_eq!(*recorder.tones.lock().expect("tones"), 1);

    app.world_mut()
        .resource_mut::<AudioHapticsSettings>()
        .set_muted(true);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: pill });
    app.update();
    assert_eq!(recorder.haptics.lock().expect("haptics").len(), 1);
    assert_eq!(*recorder.tones.lock().expect("tones"), 1);
}

#[test]
fn unsupported_host_turns_interaction_feedback_into_a_noop() {
    use bevy::ui_widgets::Activate;
    use infiltrator_bevy_ui::host_capabilities::{HapticsHost, HapticsPort};
    use infiltrator_bevy_ui::pages::overview_cards::OverviewModeSegmentPill;
    use infiltrator_bevy_widgets::haptics::{AudioHapticOutput, HapticPattern, PcmBuffer};
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recorder {
        haptics: Mutex<Vec<HapticPattern>>,
    }
    impl HapticsPort for Recorder {
        fn vibrate(&self, pattern: HapticPattern) {
            self.haptics.lock().expect("haptics").push(pattern);
        }
    }
    impl AudioHapticOutput for Recorder {
        fn play_tone(&self, _pcm: &PcmBuffer) {}
    }

    let recorder = Arc::new(Recorder::default());
    let mut app = mounted_default();
    app.insert_resource(HapticsHost::new(recorder.clone(), false).with_audio(recorder.clone()));
    let pill = app
        .world_mut()
        .query::<(Entity, &OverviewModeSegmentPill)>()
        .iter(app.world())
        .find(|(_, pill)| pill.0 == ProxyMode::Global)
        .map(|(entity, _)| entity)
        .expect("the Global mode pill is mounted");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: pill });
    app.update();
    assert!(recorder.haptics.lock().expect("haptics").is_empty());
}

#[test]
fn reactive_overview_summary_recomputes_only_on_metric_change() {
    use infiltrator_bevy_ui::pages::overview::overview_reactive::SIGNAL_LOAD_SCORE;
    use infiltrator_bevy_widgets::reactive::ReactiveEvaluationStats;

    let mut app = mounted_default();
    app.update();
    app.update();
    let before = app
        .world()
        .resource::<ReactiveEvaluationStats>()
        .total_recomputed;

    let mut projection = DemoOverviewSource::running().current();
    projection.active_connections += 1;
    app.world_mut()
        .trigger(OverviewProjectionUpdated(projection.clone()));
    app.update();

    let stats = app.world().resource::<ReactiveEvaluationStats>();
    assert_eq!(stats.last_recomputed, vec![SIGNAL_LOAD_SCORE]);
    let after_change = stats.total_recomputed;
    assert!(after_change > before);

    app.world_mut()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();
    let stats = app.world().resource::<ReactiveEvaluationStats>();
    assert_eq!(stats.total_recomputed, after_change);
    assert!(stats.last_recomputed.is_empty());
}
