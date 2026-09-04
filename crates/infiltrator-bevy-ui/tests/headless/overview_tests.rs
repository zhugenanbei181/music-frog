//! Headless Overview tests: routing over the real shell, the typed
//! tri-state projections, the in-place refresh seam, the injected source
//! decoupling, and the theme-flip reskin of every page surface — on
//! `MinimalPlugins` (no window, no render hardware).

use std::time::Duration;

use bevy::MinimalPlugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::asset::{AssetApp, AssetPlugin, Assets};
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::scene::ScenePlugin;
use bevy::text::TextColor;
use bevy::ui::BackgroundColor;
use bevy::ui::widget::{ImageNode, Text};
use infiltrator_bevy_ui::app::{ContentSlot, ShellPlugin, SidebarFoot};
use infiltrator_bevy_ui::history::{TrafficHistory, chart_series, demo_traffic_series};
use infiltrator_bevy_ui::pages::overview::{
    CHART_HEIGHT_PX, CHART_WIDTH_PX, OnAccentText, OverviewCardState, OverviewChip,
    OverviewChipKind, OverviewLine, OverviewLineKind, OverviewModeChip, OverviewModePill,
    OverviewProjectionUpdated, OverviewStatusCard, StatusDot, StopButton, SubscriptionQuotaCard,
    TopologyChainCard, format_memory, format_rate, subscription_quota_scene, topology_chain_scene,
};
use infiltrator_bevy_ui::pages::overview_cards::{
    ActiveExitNodeCard, SystemProxyMasterCard, TunMasterCard,
};
use infiltrator_bevy_ui::projection::{
    DemoOverviewSource, OverviewOrigin, OverviewProjection, OverviewSource, OverviewState,
    ProxyMode, SourceKind,
};
use infiltrator_bevy_ui::route::{PageRoot, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::chart::ChartPlate;
use infiltrator_bevy_widgets::icon::{IconId, IconPlate};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::stat_chip::StatChipValue;
use infiltrator_bevy_widgets::surface::SurfacePanel;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{LightDark, Theme};

/// The demo core's unavailability reason (projection.rs fixture).
const DEMO_REASON: &str = "demo: external controller unreachable (connection refused)";

/// A source that is not the demo fixture: distinct, ugly, honest values.
struct StubSource;

impl OverviewSource for StubSource {
    fn current(&self) -> OverviewProjection {
        OverviewProjection {
            state: OverviewState::Running,
            mode: ProxyMode::Direct,
            upload_bps: 250_000.0,
            download_bps: 4_047.0,
            active_connections: 3,
            memory_bytes: Some(70 * 1024 * 1024),
            sampled_at: Duration::from_secs(7),
            failure: None,
            origin: OverviewOrigin::Demo,
            core_version: None,
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
    if let Some(children) = world.get::<bevy::ecs::hierarchy::Children>(chip) {
        stack.extend(children.iter());
    }
    while let Some(entity) = stack.pop() {
        if world.get::<StatChipValue>(entity).is_some() {
            return entity;
        }
        if let Some(children) = world.get::<bevy::ecs::hierarchy::Children>(entity) {
            stack.extend(children.iter());
        }
    }
    panic!("chip {chip:?} carries no value text")
}

/// The chip's marked icon plate (the same subtree walk as the value
/// lookup — the plate sits inside the chip's icon tile).
fn chip_icon_plate(world: &mut World, chip: Entity) -> IconId {
    let mut stack: Vec<Entity> = Vec::new();
    if let Some(children) = world.get::<bevy::ecs::hierarchy::Children>(chip) {
        stack.extend(children.iter());
    }
    while let Some(entity) = stack.pop() {
        if let Some(plate) = world.get::<IconPlate>(entity) {
            return plate.0;
        }
        if let Some(children) = world.get::<bevy::ecs::hierarchy::Children>(entity) {
            stack.extend(children.iter());
        }
    }
    panic!("chip {chip:?} carries no icon plate")
}

/// (entity, fill, stored state) of the status banner.
fn card(world: &mut World) -> (Entity, Color, OverviewState) {
    let mut cards = world.query::<(Entity, &OverviewStatusCard, &BackgroundColor)>();
    let (id, _, fill) = cards.single(world).expect("one status banner");
    let state = world
        .get::<OverviewCardState>(id)
        .map(|stored| stored.0)
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
        let mut stops = world.query::<(Entity, &StopButton)>();
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

/// The route mounts the Overview page as a child of the shell's content
/// slot, exactly once; the shell's title row shows the 核心概览 heading
/// and the banner carries the running state.
#[test]
fn overview_mounts_under_the_content_slot() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let slot = content_slot(world);
    let (root, route) = page_root(world);
    assert_eq!(route, Route::Overview);
    assert_eq!(
        world.get::<ChildOf>(root).expect("page parented").0,
        slot,
        "page root is a direct child of the content slot"
    );

    let mut headings = world.query::<(&Text, &TextRole)>();
    let title = headings
        .iter(world)
        .find(|(text, role)| role.0 == Role::Heading && text.0 == "核心概览");
    assert!(
        title.is_some(),
        "the shell title row mounts the 核心概览 heading"
    );

    let (_, state_text, _) = line(world, OverviewLineKind::State);
    assert_eq!(state_text, "运行中", "the banner spells the running state");
}

/// Re-triggering the same route must never stack pages: cross-frame it
/// is a no-op (ids stable), and two triggers flushed in the same frame
/// still converge on exactly one mounted page.
#[test]
fn repeated_same_route_triggers_do_not_stack() {
    let mut app = mounted_default();
    let (root_before, _) = page_root(app.world_mut());

    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Overview));
    app.update();
    let (root_after, _) = page_root(app.world_mut());
    assert_eq!(
        root_after, root_before,
        "settled same-route trigger is a no-op"
    );

    let mut commands = app.world_mut().commands();
    commands.trigger(RouteChanged(Route::Overview));
    commands.trigger(RouteChanged(Route::Overview));
    app.update();

    let world = app.world_mut();
    let mut roots = world.query::<&PageRoot>();
    assert_eq!(
        roots.iter(world).count(),
        1,
        "same-frame duplicate triggers converge on one page"
    );
    let (root_last, _) = page_root(world);
    assert_eq!(
        root_last, root_before,
        "replacement stays under one root id set"
    );
}

// ---- the tri-state projections ----------------------------------------------

/// Running / Stopped / Unavailable each render a visible projection:
/// state word, state ink token, failure copy and banner fill — all from
/// the same injected seam.
#[test]
fn three_state_projections_are_visible() {
    let palette = UiPalette::new(&Theme::dark());
    for source in [
        DemoOverviewSource::running(),
        DemoOverviewSource::stopped(),
        DemoOverviewSource::unavailable(),
    ] {
        let state = source.current().state;
        let mut app = mounted_app_with(source);
        let world = app.world_mut();

        let (_, state_text, ink) = line(world, OverviewLineKind::State);
        assert_eq!(
            state_text,
            match state {
                OverviewState::Running => "运行中",
                OverviewState::Stopped => "已停止",
                OverviewState::Unavailable => "不可用",
            }
        );
        let expected_ink = match state {
            OverviewState::Running => palette.ink,
            OverviewState::Stopped => palette.ink_dim,
            OverviewState::Unavailable => palette.on_accent,
        };
        assert_eq!(ink.0, expected_ink, "{state:?} ink is the token one");

        let (_, failure_text, _) = line(world, OverviewLineKind::Failure);
        match state {
            OverviewState::Unavailable => {
                assert_eq!(failure_text, DEMO_REASON, "the reason is visible");
            }
            _ => assert_eq!(failure_text, "", "no fabricated status"),
        }

        let (_, fill, stored) = card(world);
        let expected_fill = if state == OverviewState::Unavailable {
            palette.danger
        } else {
            palette.accent_container
        };
        assert_eq!(
            fill, expected_fill,
            "{state:?} banner fill is the token one"
        );
        assert_eq!(stored, state, "the banner stores its projection state");

        let (_, upload, _) = line(world, OverviewLineKind::Upload);
        let (_, connections) = chip_value(world, OverviewChipKind::Connections);
        let (_, memory) = chip_value(world, OverviewChipKind::Memory);
        if state == OverviewState::Running {
            assert_eq!(upload, "↑ 1.40 MB/s");
            assert_eq!(connections, "12");
            assert_eq!(memory, "96.00 MB");
        } else {
            assert_eq!(upload, "↑ 0 B/s", "no traffic is stated as zero");
            assert_eq!(connections, "0");
            assert_eq!(memory, "—", "no memory reading is stated as absent");
        }
    }
}

// ---- the refresh seam -------------------------------------------------------

/// A projection event restamps texts, inks, pill selection, the banner's
/// stored state and the chip values in place — every restampable entity
/// keeps its id.
#[test]
fn projection_updates_restamp_in_place() {
    let palette = UiPalette::new(&Theme::dark());
    let mut app = mounted_app_with(DemoOverviewSource::running());
    let ids_before = overview_entity_ids(app.world_mut());

    let stopped = OverviewProjection {
        state: OverviewState::Stopped,
        mode: ProxyMode::Global,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
        sampled_at: Duration::from_secs(9),
        failure: None,
        origin: OverviewOrigin::Demo,
        core_version: None,
    };
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(stopped));
    app.update();

    let world = app.world_mut();
    assert_eq!(
        overview_entity_ids(world),
        ids_before,
        "refresh never remounts: ids unchanged"
    );
    let (_, state_text, ink) = line(world, OverviewLineKind::State);
    assert_eq!(state_text, "已停止");
    assert_eq!(ink.0, palette.ink_dim, "stopped ink restamped");
    let (_, upload, _) = line(world, OverviewLineKind::Upload);
    assert_eq!(upload, "↑ 0 B/s");
    let (_, mode_chip, _) = line(world, OverviewLineKind::ModeChip);
    assert_eq!(mode_chip, "全局模式", "the banner chip renames the mode");
    assert!(
        pill_selected(world, ProxyMode::Global),
        "Global pill selected"
    );
    assert!(
        !pill_selected(world, ProxyMode::Rule),
        "Rule pill deselected"
    );
    let (_, fill, stored) = card(world);
    assert_eq!(stored, OverviewState::Stopped);
    assert_eq!(
        fill, palette.accent_container,
        "stopped keeps the accent container banner"
    );
    let (_, memory) = chip_value(world, OverviewChipKind::Memory);
    assert_eq!(memory, "—");

    let unavailable = OverviewProjection {
        state: OverviewState::Unavailable,
        mode: ProxyMode::Global,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
        sampled_at: Duration::from_secs(10),
        failure: Some("refused".to_owned()),
        origin: OverviewOrigin::LiveCore,
        core_version: Some("v1.19.18".to_owned()),
    };
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(unavailable));
    app.update();

    let world = app.world_mut();
    assert_eq!(
        overview_entity_ids(world),
        ids_before,
        "still zero remounts"
    );
    let (_, failure_text, _) = line(world, OverviewLineKind::Failure);
    assert_eq!(failure_text, "refused");
    let (_, fill, _) = card(world);
    assert_eq!(
        fill, palette.danger,
        "unavailable flips the banner to danger"
    );
    let (_, state_text, ink) = line(world, OverviewLineKind::State);
    assert_eq!(state_text, "不可用");
    assert_eq!(
        ink.0, palette.on_accent,
        "state ink readable on the danger banner"
    );
}

/// The page renders whatever source the shell injects — proof that the
/// demo fixture and the real (future) source share one seam and nothing
/// on the page is welded to the fixture.
#[test]
fn injected_source_drives_the_page_not_the_demo_fixture() {
    let mut app = mounted_app_with(StubSource);
    let world = app.world_mut();
    let (_, upload, _) = line(world, OverviewLineKind::Upload);
    assert_eq!(upload, "↑ 244.14 KB/s", "stub rate, not the demo 1.40 MB/s");
    let (_, connections) = chip_value(world, OverviewChipKind::Connections);
    assert_eq!(connections, "3", "stub count, not the demo 12");
    let (_, memory) = chip_value(world, OverviewChipKind::Memory);
    assert_eq!(memory, "70.00 MB", "stub memory, not the demo 96");
    let (_, download) = chip_value(world, OverviewChipKind::Download);
    assert_eq!(download, "3.95 KB/s");
    assert!(
        pill_selected(world, ProxyMode::Direct),
        "stub mode selected"
    );
}

/// The metrics band draws semantic plates: 上传 the up arrow, 下载 the
/// down arrow (never the Plus/FileText stand-ins), connections the
/// activity pulse and memory the zap.
#[test]
fn chips_carry_their_semantic_icon_plates() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let mut chips = world.query::<(Entity, &OverviewChip)>();
    let mounted: Vec<(Entity, OverviewChipKind)> =
        chips.iter(world).map(|(id, chip)| (id, chip.0)).collect();
    let expected = [
        (OverviewChipKind::Connections, IconId::Activity),
        (OverviewChipKind::Memory, IconId::Zap),
        (OverviewChipKind::Upload, IconId::ArrowUp),
        (OverviewChipKind::Download, IconId::ArrowDown),
    ];
    for (kind, want) in expected {
        let (chip_id, _) = mounted
            .iter()
            .find(|(_, mounted_kind)| *mounted_kind == kind)
            .unwrap_or_else(|| panic!("no {kind:?} chip mounted"));
        assert_eq!(
            chip_icon_plate(world, *chip_id),
            want,
            "{kind:?} chip draws its semantic plate"
        );
    }
}

// ---- the theme-flip reskin --------------------------------------------------

/// Triggering `ThemeSwitch` repaints every token-filled page surface
/// (banner, dot, mode chip, stop button, stat chips) from the new palette
/// and keeps every entity id.
#[test]
fn theme_flip_repaints_every_page_surface_in_place() {
    let mut app = mounted_default();
    let ids_before = overview_entity_ids(app.world_mut());

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();

    let (_, banner_fill, _) = card(world);
    assert_eq!(
        banner_fill, light.accent_container,
        "banner fill re-derived from the light tokens"
    );

    let mut dots = world.query::<(Entity, &StatusDot, &BackgroundColor)>();
    let (_, _, dot_fill) = dots.iter(world).next().expect("status dot mounted");
    assert_eq!(dot_fill.0, light.success, "dot restamped");

    let mut chip_inks = world.query::<(Entity, &OverviewModeChip, &BackgroundColor)>();
    let (_, _, chip_fill) = chip_inks.iter(world).next().expect("mode chip mounted");
    assert_eq!(chip_fill.0, light.accent, "mode chip restamped");

    let mut stops = world.query::<(Entity, &StopButton, &BackgroundColor)>();
    let (_, _, stop_fill) = stops.iter(world).next().expect("stop button mounted");
    assert_eq!(stop_fill.0, light.danger, "stop button restamped");

    let mut chips = world.query::<(Entity, &OverviewChip, &BackgroundColor)>();
    for (_, _, fill) in chips.iter(world) {
        assert_eq!(fill.0, light.surface, "stat chip fill follows the theme");
    }

    let world = app.world_mut();
    assert_eq!(
        overview_entity_ids(world),
        ids_before,
        "the reskin never remounts"
    );
}

// ---- pure formatting --------------------------------------------------------

/// The rate formatter: the iced reference ladder (B / KB / MB / GB labels
/// over 1024-based divisors, two decimals from KB up — see
/// `crates/infiltrator-iced/src/utils.rs:3-17`), `/s`-suffixed, with an
/// honest zero for absent traffic and non-finite input.
#[test]
fn format_rate_spans_the_unit_ladder() {
    assert_eq!(format_rate(0.0), "0 B/s");
    assert_eq!(format_rate(-3.0), "0 B/s");
    assert_eq!(format_rate(f64::NAN), "0 B/s");
    assert_eq!(format_rate(512.4), "512 B/s");
    assert_eq!(format_rate(1023.9), "1023 B/s");
    assert_eq!(format_rate(1024.0), "1.00 KB/s");
    assert_eq!(format_rate(250_000.0), "244.14 KB/s");
    assert_eq!(format_rate(999.0 * 1024.0), "999.00 KB/s");
    assert_eq!(format_rate(1024.0 * 1024.0), "1.00 MB/s");
    assert_eq!(format_rate(3.95 * 1024.0 * 1024.0), "3.95 MB/s");
    assert_eq!(format_rate(2.5 * 1024.0 * 1024.0), "2.50 MB/s");
    assert_eq!(format_rate(1.3 * 1024.0 * 1024.0 * 1024.0), "1.30 GB/s");
    assert_eq!(
        format_rate(3.0 * 1024.0 * 1024.0 * 1024.0),
        "3.00 GB/s",
        "GB is the ladder's top tier, exactly as the reference formatter"
    );
}

/// The memory formatter: the shared reference ladder with two decimals,
/// and an honest em-dash for an absent reading.
#[test]
fn format_memory_spans_the_unit_ladder() {
    assert_eq!(format_memory(None), "—");
    assert_eq!(format_memory(Some(0)), "0 B");
    assert_eq!(format_memory(Some(1023)), "1023 B");
    assert_eq!(format_memory(Some(1024)), "1.00 KB");
    assert_eq!(format_memory(Some(96 * 1024 * 1024)), "96.00 MB");
    assert_eq!(format_memory(Some(3 * 1024 * 1024 * 1024)), "3.00 GB");
}

// ---- the theme-switch state-ink replay ---------------------------------------

/// A `ThemeSwitch` must not paint role ink over state semantics: after the
/// switch and the page's same-frame replay of its last projection, the
/// unavailable banner keeps the danger fill, the state word its readable
/// `on_accent` ink (not the `Display` role's plain ink a bare `apply_theme`
/// restamp would leave behind) and the uplink its success ink — with every
/// entity id intact.
#[test]
fn theme_switch_keeps_the_unavailable_state_inks_and_ids() {
    let dark = UiPalette::new(&Theme::dark());
    let mut app = mounted_app_with(DemoOverviewSource::unavailable());
    let ids_before = overview_entity_ids(app.world_mut());
    let (state_id, state_text, ink) = line(app.world_mut(), OverviewLineKind::State);
    assert_eq!(state_text, "不可用");
    assert_eq!(ink.0, dark.on_accent, "precondition: the danger ink is on");

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    assert_eq!(
        overview_entity_ids(world),
        ids_before,
        "the replay is a restamp, never a remount"
    );
    let (state_id_after, state_text, ink) = line(world, OverviewLineKind::State);
    assert_eq!(state_id_after, state_id, "the state line keeps its id");
    assert_eq!(state_text, "不可用", "the verdict survives the switch");
    assert_eq!(
        ink.0, light.on_accent,
        "the state ink stays semantic after the switch"
    );
    assert_ne!(ink.0, light.ink, "role ink must not win over state ink");

    let (_, upload, upload_ink) = line(world, OverviewLineKind::Upload);
    assert_eq!(upload, "↑ 0 B/s");
    assert_eq!(
        upload_ink.0, light.success,
        "the uplink ink stays the success token"
    );

    let mut accents = world.query::<(&OnAccentText, &TextColor)>();
    for (_, accent) in accents.iter(world) {
        assert_eq!(
            accent.0, light.on_accent,
            "on-accent copy stays readable on its accent fill"
        );
    }

    let (_, fill, stored) = card(world);
    assert_eq!(stored, OverviewState::Unavailable);
    assert_eq!(
        fill, light.danger,
        "the banner fill re-derives from the new palette"
    );
}

/// Same contract for the stopped state: the state word's dim ink survives
/// the switch instead of snapping to the role's full ink.
#[test]
fn theme_switch_keeps_the_stopped_state_ink_dim() {
    let mut app = mounted_app_with(DemoOverviewSource::stopped());
    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let (_, state_text, ink) = line(world, OverviewLineKind::State);
    assert_eq!(state_text, "已停止");
    assert_eq!(ink.0, light.ink_dim);
    assert_ne!(ink.0, light.ink);
}

// ---- the traffic card's trend chart ------------------------------------------

/// A live-origin projection with concrete rates (the chart-seam input).
fn live_projection(upload_bps: f64, download_bps: f64) -> OverviewProjection {
    OverviewProjection {
        state: OverviewState::Running,
        mode: ProxyMode::Rule,
        upload_bps,
        download_bps,
        active_connections: 1,
        memory_bytes: None,
        sampled_at: Duration::from_secs(3),
        failure: None,
        origin: OverviewOrigin::LiveCore,
        core_version: Some("v1.19.18".to_owned()),
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

/// The traffic card mounts a real chart node: the plate carries the fixed
/// token box, the demo fixture's synthetic series, lives inside the card
/// whose caption is 实时流量, and has actually been rasterized (the
/// widget's sync_charts stamped its ImageNode).
#[test]
fn traffic_card_mounts_the_trend_chart() {
    let mut app = mounted_default();
    app.update(); // the frame sync_charts stamps the raster on
    let world = app.world_mut();
    let (plate_id, plate) = chart_plate(world);

    assert_eq!(
        plate.0.width,
        CHART_WIDTH_PX.round() as u32,
        "full-card box"
    );
    assert_eq!(plate.0.height, CHART_HEIGHT_PX.round() as u32);
    let (demo_up, demo_down) = demo_traffic_series();
    assert_eq!(plate.0.up, demo_up, "demo origin draws the fixture waves");
    assert_eq!(plate.0.down, demo_down);

    let mut cards = world.query::<(Entity, &SurfacePanel)>();
    let card_ids: Vec<Entity> = cards.iter(world).map(|(id, _)| id).collect();
    let traffic_card = card_ids
        .iter()
        .copied()
        .find(|id| {
            descendants(world, *id)
                .iter()
                .any(|e| world.get::<Text>(*e).is_some_and(|t| t.0 == "实时流量"))
        })
        .expect("the traffic card is mounted");
    assert!(
        descendants(world, traffic_card).contains(&plate_id),
        "the chart node lives inside the traffic card"
    );
    assert!(
        world.get::<ImageNode>(plate_id).is_some(),
        "sync_charts rasterized the plate on mount"
    );
}

/// A projection update re-derives the chart series (demo waves → the live
/// ring) and restamps the plate in place — same entity, same handle
/// contract with sync_charts.
#[test]
fn projection_updates_refresh_the_chart_series() {
    let mut app = mounted_default();
    let (plate_id, before) = chart_plate(app.world_mut());
    let (demo_up, _) = demo_traffic_series();
    assert_eq!(before.0.up, demo_up, "precondition: the demo trend mounted");

    // A live projection plus a recorded ring (in production the drain
    // appends at the drain site; here the ring is injected directly).
    let mut history = TrafficHistory::default();
    history.push(1.0, 2.0);
    history.push(3.0, 4.0);
    {
        let world = app.world_mut();
        world.insert_resource(history);
        world
            .commands()
            .trigger(OverviewProjectionUpdated(live_projection(5.0, 6.0)));
    }
    app.update();

    let world = app.world_mut();
    let (plate_id_after, after) = chart_plate(world);
    assert_eq!(plate_id_after, plate_id, "the chart never remounts");
    assert_eq!(after.0.up, vec![1.0, 3.0], "the series follows the ring");
    assert_eq!(after.0.down, vec![2.0, 4.0]);
    let (want_up, want_down) =
        chart_series(OverviewOrigin::LiveCore, world.resource::<TrafficHistory>());
    assert_eq!((after.0.up, after.0.down), (want_up, want_down));
}

/// A `ThemeSwitch` re-rasterizes the chart under the SAME image handle
/// (chart.rs's write-back contract): entity id stable, asset id stable,
/// pixels re-derived from the new palette.
#[test]
fn theme_flip_rerasterizes_the_chart_in_place() {
    let mut app = mounted_default();
    app.update(); // the frame sync_charts stamps the raster on
    let (plate_id, _) = chart_plate(app.world_mut());
    let handle = app
        .world()
        .get::<ImageNode>(plate_id)
        .expect("chart rasterized on mount")
        .image
        .clone();
    let data_before = app
        .world()
        .resource::<Assets<Image>>()
        .get(&handle)
        .expect("chart asset")
        .data
        .clone();

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let world = app.world_mut();
    assert!(
        world.get::<ChartPlate>(plate_id).is_some(),
        "the chart keeps its entity id"
    );
    let node = world.get::<ImageNode>(plate_id).expect("the node survives");
    assert_eq!(
        node.image.id(),
        handle.id(),
        "same handle — write-back, never a swap"
    );
    let data_after = world
        .resource::<Assets<Image>>()
        .get(&node.image)
        .expect("chart asset")
        .data
        .clone();
    assert_ne!(data_before, data_after, "the chart inks follow the theme");
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
            mode: ProxyMode::Rule,
            upload_bps: 0.0,
            download_bps: 0.0,
            active_connections: 0,
            memory_bytes: None,
            sampled_at: Duration::from_secs(1),
            failure: None,
            origin: OverviewOrigin::LiveCore,
            core_version: self.version.map(str::to_owned),
        }
    }

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }
}

/// The foot names the data source: the demo milestone caption under the
/// fixture, the real core version under the live pump (and an honest
/// placeholder while the version has not been read yet).
#[test]
fn sidebar_foot_follows_the_source_kind() {
    let mut app = mounted_app_with(DemoOverviewSource::running());
    assert_eq!(
        foot_text(app.world_mut()),
        "0.30 demo",
        "demo keeps its caption"
    );

    let mut app = mounted_app_with(LiveFootStub {
        version: Some("v1.19.18"),
    });
    assert_eq!(
        foot_text(app.world_mut()),
        "实时内核 · v1.19.18",
        "a live core names the version it reported"
    );

    let mut app = mounted_app_with(LiveFootStub { version: None });
    assert_eq!(
        foot_text(app.world_mut()),
        "实时内核 · 版本读取中",
        "an unread version stays honest"
    );
}

// ---- accessibility semantics --------------------------------------------------

/// The four stat chips carry labeled Group semantics ("name value") that
/// the refresh observer restamps, and the banner's state word carries a
/// Status semantic that follows the run state.
#[test]
fn stat_chips_and_banner_status_carry_accesskit_semantics() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut chips = world.query::<(&OverviewChip, &AccessibilityNode)>();
    let mut labels: Vec<(OverviewChipKind, String)> = Vec::new();
    for (chip, node) in chips.iter(world) {
        assert_eq!(node.role(), accesskit::Role::Group);
        labels.push((chip.0, node.label().expect("chip group label").to_owned()));
    }
    assert_eq!(labels.len(), 4, "every chip carries one group node");
    assert!(labels.contains(&(OverviewChipKind::Connections, "连接数 12".to_owned())));
    assert!(labels.contains(&(OverviewChipKind::Memory, "内存 96.00 MB".to_owned())));
    assert!(labels.contains(&(OverviewChipKind::Upload, "上传 1.40 MB/s".to_owned())));
    assert!(labels.contains(&(OverviewChipKind::Download, "下载 8.60 MB/s".to_owned())));

    let mut lines = world.query::<(&OverviewLine, &AccessibilityNode)>();
    let (_, status) = lines
        .iter(world)
        .find(|(line, _)| line.0 == OverviewLineKind::State)
        .expect("the state line carries semantics");
    assert_eq!(status.role(), accesskit::Role::Status);
    assert_eq!(status.label(), Some("运行中"));

    // A refresh restamps the semantics alongside the visible texts.
    let stopped = OverviewProjection {
        state: OverviewState::Stopped,
        mode: ProxyMode::Rule,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
        sampled_at: Duration::from_secs(9),
        failure: None,
        origin: OverviewOrigin::Demo,
        core_version: None,
    };
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(stopped));
    app.update();

    let world = app.world_mut();
    let mut chips = world.query::<(&OverviewChip, &AccessibilityNode)>();
    let mut labels: Vec<(OverviewChipKind, String)> = Vec::new();
    for (chip, node) in chips.iter(world) {
        labels.push((chip.0, node.label().expect("chip group label").to_owned()));
    }
    assert!(labels.contains(&(OverviewChipKind::Connections, "连接数 0".to_owned())));
    assert!(labels.contains(&(OverviewChipKind::Memory, "内存 —".to_owned())));
    let mut lines = world.query::<(&OverviewLine, &AccessibilityNode)>();
    let (_, status) = lines
        .iter(world)
        .find(|(line, _)| line.0 == OverviewLineKind::State)
        .expect("the state line carries semantics");
    assert_eq!(status.label(), Some("已停止"), "the status word follows");
}

/// Navigating across multiple routes replaces the bounded subtree below ContentSlot
/// and stamps the corresponding PageRoot marker.
#[test]
fn route_switching_mounts_target_page_scene_idempotently() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut roots = world.query::<&PageRoot>();
    assert_eq!(
        roots.iter(world).next().expect("overview page root").0,
        Route::Overview
    );

    // Navigate to Proxies route.
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();

    let world = app.world_mut();
    let mut roots = world.query::<&PageRoot>();
    assert_eq!(
        roots.iter(world).next().expect("proxies page root").0,
        Route::Proxies
    );

    // Navigate to Rules route.
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Rules));
    app.update();

    let world = app.world_mut();
    let mut roots = world.query::<&PageRoot>();
    assert_eq!(
        roots.iter(world).next().expect("rules page root").0,
        Route::Rules
    );

    // Navigate back to Overview.
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Overview));
    app.update();

    let world = app.world_mut();
    let mut roots = world.query::<&PageRoot>();
    assert_eq!(
        roots.iter(world).next().expect("back to overview").0,
        Route::Overview
    );
}

/// The Overview page uses responsive wrapping for stat chips and scrollable viewport,
/// ensuring 4 chips wrap into a clean 2x2 grid on compact mobile screens (<600px).
#[test]
fn overview_page_chips_and_container_responsive_wrapping() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut chips = world.query::<(&OverviewChip, &bevy::ui::Node)>();
    let count = chips.iter(world).count();
    assert_eq!(count, 4, "exactly four stat chips mounted");

    for (_, node) in chips.iter(world) {
        assert_eq!(
            node.flex_grow, 1.0,
            "chips share width evenly via flex_grow"
        );
        assert_eq!(
            node.flex_basis,
            bevy::ui::Val::Px(140.0),
            "chips carry 140px flex_basis for responsive 2x2 wrapping on mobile"
        );
    }
}

/// The Overview page mounts the traffic topology chain card (BEVY-GAP-018)
/// with 4 linked stage chips and connecting arrows (">").
#[test]
fn test_topology_chain_card_mounts_with_four_stages_and_arrows() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut query = world.query::<(Entity, &TopologyChainCard)>();
    let (card_entity, _) = query
        .iter(world)
        .next()
        .expect("TopologyChainCard must be mounted in overview page");

    let all_descendants = descendants(world, card_entity);
    let texts: Vec<String> = all_descendants
        .iter()
        .filter_map(|e| world.get::<Text>(*e).map(|t| t.0.clone()))
        .collect();

    // Card title & badge
    assert!(
        texts.iter().any(|t| t == "分流网络拓扑 (Traffic Topology)"),
        "card contains title"
    );
    assert!(
        texts.iter().any(|t| t == "12 连接"),
        "card contains connection count badge"
    );

    // Stage 1: Client / Inbound
    assert!(texts.iter().any(|t| t == "Client / Inbound"));
    assert!(texts.iter().any(|t| t == "Mixed: 7890"));
    assert!(texts.iter().any(|t| t == "12 conns"));

    // Stage 2: RuleSet
    assert!(texts.iter().any(|t| t == "RuleSet"));
    assert!(texts.iter().any(|t| t == "MRS / GeoIP"));
    assert!(texts.iter().any(|t| t == "Active"));

    // Stage 3: Proxy Group
    assert!(texts.iter().any(|t| t == "Proxy Group"));
    assert!(texts.iter().any(|t| t == "GLOBAL / PROXIES"));
    assert!(texts.iter().any(|t| t == "Selector"));

    // Stage 4: Outbound Node
    assert!(texts.iter().any(|t| t == "Outbound Node"));
    assert!(texts.iter().any(|t| t == "香港 01 · BGP 专线"));
    assert!(texts.iter().any(|t| t == "38 ms"));

    // Connecting arrows
    let arrow_count = texts.iter().filter(|t| t.as_str() == ">").count();
    assert_eq!(
        arrow_count, 3,
        "must have exactly 3 connecting arrows between the 4 chips"
    );

    // Standalone scene creation test
    let palette = UiPalette::new(&Theme::dark());
    let _scene = topology_chain_scene(&palette);
}

/// The Overview page mounts the subscription quota card (BEVY-GAP-020)
/// with title, header, subtitle/stats and visual progress bar.
#[test]
fn test_subscription_quota_card_mounts_with_progress_bar() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut query = world.query::<(Entity, &SubscriptionQuotaCard)>();
    let (card_entity, _) = query
        .iter(world)
        .next()
        .expect("SubscriptionQuotaCard must be mounted in overview page");

    let all_descendants = descendants(world, card_entity);
    let texts: Vec<String> = all_descendants
        .iter()
        .filter_map(|e| world.get::<Text>(*e).map(|t| t.0.clone()))
        .collect();

    // Card title
    assert!(texts.iter().any(|t| t == "订阅配额"), "contains 订阅配额");

    // Header: "主力高速订阅 (Primary VIP)" and "2026-10-01 到期"
    assert!(
        texts.iter().any(|t| t == "主力高速订阅 (Primary VIP)"),
        "contains 主力高速订阅 (Primary VIP)"
    );
    assert!(
        texts.iter().any(|t| t == "2026-10-01 到期"),
        "contains 2026-10-01 到期"
    );

    // Subtitle/stats: "已用: 46.43 GB / 总计: 186.26 GB (24.9%)"
    assert!(
        texts
            .iter()
            .any(|t| t == "已用: 46.43 GB / 总计: 186.26 GB (24.9%)"),
        "contains stats"
    );

    // Visual progress bar: height ~8px, inner fill width 25% with palette.accent
    let mut found_bar = false;
    for e in &all_descendants {
        if let Some(node) = world.get::<bevy::ui::Node>(*e)
            && node.height == bevy::ui::Val::Px(8.0)
        {
            let bar_descendants = descendants(world, *e);
            for child in bar_descendants {
                if let Some(inner_node) = world.get::<bevy::ui::Node>(child)
                    && inner_node.width == bevy::ui::Val::Percent(25.0)
                {
                    found_bar = true;
                    break;
                }
            }
        }
    }
    assert!(
        found_bar,
        "visual progress bar with 8px height and 25% fill mounted"
    );

    // Standalone scene creation test
    let palette = UiPalette::new(&Theme::dark());
    let _scene = subscription_quota_scene(&palette);
}

#[test]
fn test_overview_master_switches_and_exit_node_cards() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut exit_query = world.query::<(Entity, &ActiveExitNodeCard)>();
    let (exit_entity, _) = exit_query
        .iter(world)
        .next()
        .expect("ActiveExitNodeCard must be mounted");
    let exit_descendants = descendants(world, exit_entity);
    let exit_texts: Vec<String> = exit_descendants
        .iter()
        .filter_map(|e| world.get::<Text>(*e).map(|t| t.0.clone()))
        .collect();
    assert!(exit_texts.iter().any(|t| t.contains("当前主出口节点")));
    assert!(exit_texts.iter().any(|t| t.contains("🇭🇰")));

    let mut proxy_query = world.query::<(Entity, &SystemProxyMasterCard)>();
    assert!(
        proxy_query.iter(world).next().is_some(),
        "SystemProxyMasterCard must be mounted"
    );

    let mut tun_query = world.query::<(Entity, &TunMasterCard)>();
    assert!(
        tun_query.iter(world).next().is_some(),
        "TunMasterCard must be mounted"
    );
}
