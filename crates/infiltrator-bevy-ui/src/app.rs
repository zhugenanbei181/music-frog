//! The app shell: a left sidebar (identity, mode segment control, nav,
//! version) and a content column (title row + the page slot), composed
//! with `bsn!` over the widget layer in the iced reference product
//! language. The two shell seams are unchanged:
//!
//! - **Theme affordance** ([`ThemeToggle`] / [`ThemeMode`]): a pill in
//!   the title row whose activation flips the shell-owned mode mirror
//!   and triggers the widget layer's `ThemeSwitch` — the mounted tree is
//!   restamped in place, never remounted (charter law: observers change
//!   components). The sidebar's own fill carries the [`SidebarPanel`]
//!   marker and is repainted every frame by [`sync_sidebar_panel`] (the
//!   checkbox/slider compare-and-set idiom); the widget layer runs the
//!   same contract for its pills, nav items, icon tiles and chips.
//! - **Semantic seeds** ([`window_semantic_node`] /
//!   [`header_semantic_node`]): `AccessibilityNode` components stamped
//!   into the scene. They are pure data until a windowed composition
//!   activates the AccessKit bridge.
//!
//! **Mode segment control (BEVY-005)**: the sidebar's mode pills are wired
//! through [`on_mode_pill_activated`] — one `On<Activate>` observer that
//! dispatches on the [`OverviewModePill`] marker, posts `set_mode` through
//! the injected [`OverviewSourceHandle`] and latches
//! [`ModeCommandInFlight`] until [`drain_mode_ack`] receives the typed
//! receipt (in-flight activations are ignored, never queued). A refused
//! command projects [`OverviewState::Unavailable`] with the failure copy
//! and latches the pump's [`FailureDwell`] when one is mounted, so the
//! verdict survives the next sampling ticks; an accepted one re-reads the
//! source so the pills follow the round trip.
//! The stop button stays demo-honest: the page swaps it for a typed
//! lifecycle caption when a live core feeds the projection (see
//! `pages::overview`).
//!
//! **Honesty note**: the sidebar's 数据同步 / 系统设置 entries are real,
//! visibly-disabled items tagged 未迁移 — they route nowhere because
//! nothing behind them exists yet.
//!
//! **Copy note**: sidebar strings are zh-CN literals by design of this
//! demo slice — the 0.30 i18n milestone unifies them into locale keys.
//! (本片文案为 zh-CN 字面量，0.30 i18n 里程碑统一 locale key。)
//!
//! bsn! idiom (crate law, docs/BEVY_UI_FRONTEND.md): one `bsn!` per named
//! scene function; dynamic values ride the declarative shape; runtime
//! changes restamp components via observers and never rebuild the tree.
//! The plugin below stays headless-safe (no `DefaultPlugins` here — the
//! window launcher in `lib.rs` owns that composition), which is exactly
//! what lets `MinimalPlugins` headless tests exercise the real shell.

use std::sync::Mutex;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Startup, Update};
use bevy::camera::Camera2d;
use bevy::camera::ClearColor;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn, template_value};
use bevy::ui::BackgroundColor;
use bevy::ui::prelude::{
    AlignItems, FlexDirection, JustifyContent, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::button::{pill_caption_scene, pill_scene};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::nav::nav_item_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{LightDark, Theme, space};

use crate::controller::FailureDwell;
use crate::pages::overview::{OverviewModePill, OverviewProjectionUpdated, mode_label};
use crate::projection::{OverviewState, ProxyMode};
use crate::route::OverviewSourceHandle;

/// Sidebar rail width (px) — the iced reference's left column.
const SIDEBAR_WIDTH_PX: f32 = 240.0;
/// Identity tile edge (px).
const IDENTITY_TILE_PX: f32 = 40.0;

/// Marker for the content region product pages mount into. Page routing
/// (BEVY-M2) replaces bounded subtrees below this slot.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ContentSlot;

/// Marker for the shell root entity: carries the window-level semantic node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ShellRoot;

/// Marker for the title row: carries the header semantic node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ShellHeader;

/// Marker for the theme-toggle pill. The shell's `On<Activate>` observer
/// filters on this marker, so one global observer serves the affordance
/// without per-entity observer plumbing inside the scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ThemeToggle;

/// Marker on the sidebar rail; [`sync_sidebar_panel`] re-projects its
/// fill from the live palette (compare-and-set).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarPanel;

/// Marker on the sidebar's foot caption. The text is spawned with the
/// demo default (the shell mounts before any page source exists);
/// [`crate::route::sync_sidebar_foot`] re-projects it from the injected
/// source's kind — `0.30 demo` for the fixture, `实时内核 · <version>`
/// for the live pump (compare-and-set, in place).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarFoot;

/// The shell's mirror of the current appearance. The resolved `UiPalette`
/// cannot be inverted back to a mode (its colors are not bijective), so the
/// shell owns the mode the theme pill flips. Seeded by the launcher (env
/// capture skin, else the cold-start dark theme — the same theme
/// [`ShellPlugin`] hands to `WidgetsPlugin`).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeMode(pub LightDark);

/// Latch for a proxy-mode command in flight: set by
/// [`on_mode_pill_activated`], cleared by [`drain_mode_ack`] when the
/// typed receipt lands. While latched, further pill activations are
/// ignored (never queued) — duplicate clicks cannot double-submit.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModeCommandInFlight(pub bool);

/// The receipt channel of the in-flight mode command, parked here until
/// the per-frame drain reads it. The `Receiver` is not `Sync`, so the
/// resource carries it behind a mutex (the drain holds `ResMut`, i.e.
/// exclusive access, and uses `get_mut`).
#[derive(Resource, Debug, Default)]
pub struct PendingModeAck(pub Option<Mutex<Receiver<Result<(), String>>>>);

/// The AccessKit node for the shell root: a Window role carrying the app
/// title. Pure component data — headless apps carry it inertly; only a
/// windowed composition (which mounts bevy_winit's `AccessKitPlugin`, the
/// only bridge activation point in bevy 0.19) publishes it to the platform
/// tree. Scenes seed it through `template_value`, which carries the already
/// constructed component untouched.
pub fn window_semantic_node(title: &str) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Window);
    node.set_label(title);
    AccessibilityNode(node)
}

/// The AccessKit node for the title row: a Header role carrying the same
/// title, so assistive technology reads the chrome as one labeled region.
pub fn header_semantic_node(title: &str) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Header);
    node.set_label(title);
    AccessibilityNode(node)
}

/// The pill's semantic node: an explicitly named Button role. The explicit
/// node replaces the unnamed `Role::Button` default the official `Button`
/// widget requires (`bevy_ui_widgets` stamps it via `#[require]`), so the
/// control reads by its action — the theme pill ("Toggle color theme")
/// and the sidebar's mode pills alike.
pub fn toggle_semantic_node(label: &str) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Button);
    node.set_label(label);
    AccessibilityNode(node)
}

/// The AccessKit node for a sidebar nav entry: a Button role carrying the
/// entry's label. The two 未迁移 (not-migrated) entries are stamped
/// disabled — they read as buttons that cannot be activated, matching
/// their visibly-idle look and their click-through-to-nowhere honesty.
pub fn nav_semantic_node(label: &str, disabled: bool) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Button);
    node.set_label(label);
    if disabled {
        node.set_disabled();
    }
    AccessibilityNode(node)
}

/// The AccessKit node for the content region: a labeled Region role, so
/// assistive technology reads the page area as one named container. The
/// label names the page family the slot currently hosts (the Overview
/// page is the whole M2 surface).
pub fn region_semantic_node(label: &str) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Region);
    node.set_label(label);
    AccessibilityNode(node)
}

/// The theme affordance: activating the shell's theme pill flips the
/// shell-owned [`ThemeMode`] mirror and triggers the widget layer's
/// [`ThemeSwitch`], whose `apply_theme` observer re-resolves the palette and
/// restamps the mounted tree in place — zero remounts. The official `Button`
/// widget emits `Activate` for pointer and keyboard activation alike in
/// windowed runs; headless tests trigger the event directly.
fn on_theme_pill_activated(
    activate: On<Activate>,
    toggles: Query<(), With<ThemeToggle>>,
    mut mode: ResMut<ThemeMode>,
    mut commands: Commands,
) {
    if !toggles.contains(activate.entity) {
        return;
    }
    let next = match mode.0 {
        LightDark::Dark => LightDark::Light,
        LightDark::Light => LightDark::Dark,
    };
    mode.0 = next;
    commands.trigger(ThemeSwitch(next));
}

/// The mode segment's affordance: activating a pill (matched by the
/// [`OverviewModePill`] marker, the same global-observer idiom as the
/// theme pill) posts `set_mode` through the injected source and latches
/// [`ModeCommandInFlight`] until [`drain_mode_ack`] reads the receipt.
/// Sources without mode support refuse through the receipt channel, so
/// the failure projection is the honest visible outcome. A pill already
/// in flight is ignored — the projection refresh (accepted) or the
/// failure copy (refused) is the only thing that un-latches.
fn on_mode_pill_activated(
    activate: On<Activate>,
    pills: Query<&OverviewModePill>,
    handle: Option<Res<OverviewSourceHandle>>,
    mut in_flight: ResMut<ModeCommandInFlight>,
    mut pending: ResMut<PendingModeAck>,
) {
    if in_flight.0 {
        return;
    }
    let Ok(wanted) = pills.get(activate.entity) else {
        return;
    };
    let Some(handle) = handle else {
        return;
    };
    let (ack_tx, ack_rx) = std::sync::mpsc::channel();
    handle.0.set_mode(wanted.0, ack_tx);
    in_flight.0 = true;
    pending.0 = Some(Mutex::new(ack_rx));
}

/// The receipt drain: one typed outcome per accepted pill activation.
///
/// - `Ok(())` — re-read the source and fire [`OverviewProjectionUpdated`]:
///   the success path is a projection round trip, never a local override.
/// - `Err(reason)` — project [`OverviewState::Unavailable`] with the
///   failure copy (the failure line becomes visible) and latch the pump's
///   [`FailureDwell`] when one is mounted: on a live source the next
///   sampling ticks would otherwise wash the verdict away within seconds
///   (see `controller::FailureDwell` — the verdict holds until the dwell
///   elapses *and* a successful sample arrives). The demo fixture keeps
///   the verdict until the next refresh either way — both honest.
/// - A disconnected channel — the source is gone; same failure path.
/// - An empty but connected channel — the command is still executing
///   (the live PATCH can take up to the client's HTTP timeout); the latch
///   stays and a later frame re-checks.
fn drain_mode_ack(
    mut pending: ResMut<PendingModeAck>,
    mut in_flight: ResMut<ModeCommandInFlight>,
    handle: Option<Res<OverviewSourceHandle>>,
    mut dwell: Option<ResMut<FailureDwell>>,
    mut commands: Commands,
) {
    let Some(slot) = pending.0.as_mut() else {
        return;
    };
    // Exclusive access: get_mut cannot race; a poisoned mutex still hands
    // us the (valid) receiver.
    let outcome = match slot
        .get_mut()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .try_recv()
    {
        Ok(receipt) => Some(receipt),
        Err(std::sync::mpsc::TryRecvError::Empty) => None, // still executing
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            Some(Err("模式命令通道已断开".to_owned()))
        }
    };
    let Some(receipt) = outcome else {
        return;
    };
    pending.0 = None;
    in_flight.0 = false;
    let Some(handle) = handle else {
        return; // no source mounted; the latch is already cleared
    };
    let mut projection = handle.0.current();
    match receipt {
        Ok(()) => commands.trigger(OverviewProjectionUpdated(projection)),
        Err(reason) => {
            projection.state = OverviewState::Unavailable;
            projection.failure = Some(format!("模式切换失败：{reason}"));
            if let Some(dwell) = dwell.as_deref_mut() {
                dwell.latch(Instant::now());
            }
            commands.trigger(OverviewProjectionUpdated(projection));
        }
    }
}

/// Installs the widget layer (seeded with this plugin's cold-start mode),
/// the theme-mode mirror, the affordance observer, the sidebar reskin and
/// mounts the shell. No windowing infrastructure here — the launcher in
/// `lib.rs` owns `DefaultPlugins`. Headless tests run this plugin under
/// `MinimalPlugins` plus the asset/scene singleton plugins that
/// `spawn_scene` resolves through; the AccessKit bridge itself is mounted
/// only by the windowed `WinitPlugin`, so headless compositions carry the
/// semantic nodes as inert components (the honest M1 boundary).
pub struct ShellPlugin {
    mode: LightDark,
}

impl ShellPlugin {
    /// Cold-start with an explicit appearance (the capture seam's skin).
    pub fn new(mode: LightDark) -> Self {
        Self { mode }
    }
}

impl Default for ShellPlugin {
    fn default() -> Self {
        Self::new(LightDark::Dark)
    }
}

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WidgetsPlugin::new(&Theme::for_mode(self.mode)));
        app.insert_resource(ThemeMode(self.mode));
        app.init_resource::<ModeCommandInFlight>();
        app.init_resource::<PendingModeAck>();
        app.add_observer(on_theme_pill_activated);
        app.add_observer(on_mode_pill_activated);
        app.init_resource::<ClearColor>();
        app.add_systems(Startup, (spawn_camera, spawn_shell));
        app.add_systems(
            Update,
            (sync_sidebar_panel, sync_window_clear, drain_mode_ack),
        );
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn spawn_shell(mut commands: Commands, palette: Res<UiPalette>) {
    commands.spawn_scene(shell_scene("MusicFrog Infiltrator".to_string(), &palette));
}

/// Repaint the sidebar rail from the live palette. Compare-and-set:
/// unchanged frames cost nothing, and a theme switch repaints the rail
/// with no switch-specific hook.
fn sync_sidebar_panel(
    palette: Res<UiPalette>,
    mut panels: Query<&mut BackgroundColor, With<SidebarPanel>>,
) {
    for mut fill in &mut panels {
        if fill.0 != palette.sidebar {
            fill.0 = palette.sidebar;
        }
    }
}

/// Repaint the window canvas (the camera's global clear color) from the
/// `window_clear` token. Compare-and-set: unchanged frames cost nothing,
/// and a theme switch repaints the canvas with no switch-specific hook.
/// `Option` because headless `MinimalPlugins` compositions carry no core
/// pipeline — there is simply nothing to clear there.
fn sync_window_clear(palette: Res<UiPalette>, mut clear: Option<ResMut<ClearColor>>) {
    let Some(clear) = clear.as_deref_mut() else {
        return;
    };
    if clear.0 != palette.window_clear {
        clear.0 = palette.window_clear;
    }
}

/// The shell: a full-bleed sidebar (fixed 240px rail) beside the content
/// column (title row over the content slot product pages mount into).
/// The theme pill is the widget layer's `pill_scene` re-skinned by one
/// shell marker and one semantic node — interaction wiring belongs to the
/// shell.
pub fn shell_scene(title: String, palette: &UiPalette) -> impl Scene + use<> {
    let window_node = window_semantic_node(&title);
    let region_node = region_semantic_node("核心概览");
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
        }
        ShellRoot
        template_value(window_node)
        Children [
            (
                Node {
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Row,
                }
                Children [
                    ( { sidebar_scene(palette) } ),
                    (
                        Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(space::S16)),
                            row_gap: Val::Px(space::S16),
                        }
                        Children [
                            ( { content_title_row(&title, palette) } ),
                            (
                                Node { flex_grow: 1.0 }
                                ContentSlot
                                template_value(region_node)
                            ),
                        ]
                    ),
                ]
            ),
        ]
    }
}

/// The title row: the page-family heading, a flex spacer, then the theme
/// pill. Carries the header semantic node.
fn content_title_row(title: &str, palette: &UiPalette) -> impl Scene + use<> {
    let header_node = header_semantic_node(title);
    let pill_node = toggle_semantic_node("Toggle color theme");
    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S16),
        }
        ShellHeader
        template_value(header_node)
        Children [
            ( Text({ "核心概览".to_owned() }) TextRole(Role::Heading) ),
            ( Node { flex_grow: 1.0 } ),
            (
                { pill_scene("Theme".to_owned(), false, palette) }
                ThemeToggle
                template_value(pill_node)
            ),
        ]
    }
}

/// The sidebar rail: identity block, mode segment control, the nav group
/// in the content flow (the reference keeps nav just below the segment
/// control, one S16 step down — the rail's own `row_gap`), a flex spacer
/// and the version caption at the foot.
fn sidebar_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            width: px(SIDEBAR_WIDTH_PX),
            height: percent(100),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(space::S16)),
            row_gap: Val::Px(space::S16),
        }
        BackgroundColor({ palette.sidebar })
        SidebarPanel
        Children [
            ( { identity_scene(palette) } ),
            ( { mode_segment_scene(ProxyMode::default(), palette) } ),
            ( { nav_column_scene(palette) } ),
            ( Node { flex_grow: 1.0 } ),
            (
                Node {
                    width: percent(100),
                    justify_content: JustifyContent::Center,
                }
                Children [
                    (
                        Text({ "0.30 demo".to_owned() }) TextRole(Role::Caption)
                        SidebarFoot
                    ),
                ]
            ),
        ]
    }
}

/// The identity block: the app icon tile, "MusicFrog" (body strong) over
/// "Infiltrator" (caption).
fn identity_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S12),
        }
        Children [
            ( { icon_tile_scene(IconId::Network, IDENTITY_TILE_PX, palette) } ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                }
                Children [
                    ( Text({ "MusicFrog".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ "Infiltrator".to_owned() }) TextRole(Role::Caption) ),
                ]
            ),
        ]
    }
}

/// The proxy-mode segment control: three compact pills in a row, the
/// selection mirroring the Overview projection's mode (restamped by the
/// page's refresh observer through [`OverviewModePill`]; spawned on the
/// default mode, corrected by the router's first paint).
fn mode_segment_scene(mode: ProxyMode, palette: &UiPalette) -> impl Scene + use<> {
    let rule_node = toggle_semantic_node(mode_label(ProxyMode::Rule));
    let global_node = toggle_semantic_node(mode_label(ProxyMode::Global));
    let direct_node = toggle_semantic_node(mode_label(ProxyMode::Direct));
    bsn! {
        Node {
            align_items: AlignItems::Center,
            // S4: three 64px caption pills (48px of CJK + S8 padding each)
            // fit the 208px rail interior only with the tighter gap — at S8
            // the row hits the boundary exactly and flex shrink wraps the
            // labels (capture round 3).
            column_gap: Val::Px(space::S4),
        }
        Children [
            (
                { pill_caption_scene(mode_label(ProxyMode::Rule).to_owned(), mode == ProxyMode::Rule, palette) }
                OverviewModePill(ProxyMode::Rule)
                template_value(rule_node)
            ),
            (
                { pill_caption_scene(mode_label(ProxyMode::Global).to_owned(), mode == ProxyMode::Global, palette) }
                OverviewModePill(ProxyMode::Global)
                template_value(global_node)
            ),
            (
                { pill_caption_scene(mode_label(ProxyMode::Direct).to_owned(), mode == ProxyMode::Direct, palette) }
                OverviewModePill(ProxyMode::Direct)
                template_value(direct_node)
            ),
        ]
    }
}

/// The nav column: the active 核心概览 item, then the two honest,
/// visibly-disabled entries (each tagged 未迁移 — nothing routes yet).
/// Every item carries a labeled Button semantic node (the disabled ones
/// stamped disabled).
fn nav_column_scene(palette: &UiPalette) -> impl Scene + use<> {
    let overview_node = nav_semantic_node("核心概览", false);
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S8),
        }
        Children [
            (
                { nav_item_scene("核心概览".to_owned(), true, palette) }
                template_value(overview_node)
            ),
            ( { disabled_nav_row("数据同步", palette) } ),
            ( { disabled_nav_row("系统设置", palette) } ),
        ]
    }
}

/// One disabled nav entry: the idle nav item beside its 未迁移 caption.
/// The item is a plain node (never the official `Button`) — clicking it
/// must not even look pressable; its semantic node reads as a disabled
/// Button labeled with the entry name.
fn disabled_nav_row(label: &str, palette: &UiPalette) -> impl Scene + use<> {
    let nav_node = nav_semantic_node(label, true);
    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
        }
        Children [
            (
                Node { flex_grow: 1.0 }
                Children [
                    (
                        { nav_item_scene(label.to_owned(), false, palette) }
                        template_value(nav_node)
                    ),
                ]
            ),
            ( Text({ "未迁移".to_owned() }) TextRole(Role::Caption) ),
        ]
    }
}
