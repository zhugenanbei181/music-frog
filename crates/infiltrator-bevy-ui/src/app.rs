//! The app shell: left sidebar/rail/bottom-nav and content column composed with `bsn!`.
//! Seams: Responsive layout breakpoints (Compact, Medium, Expanded, Ultra), theme/density toggles,
//! AccessKit semantic nodes, and mode segment control (BEVY-005).

use std::sync::{Mutex, mpsc::Receiver};
use std::time::Instant;

use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Startup, Update};
use bevy::camera::Camera2d;
use bevy::camera::ClearColor;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene};
use bevy::text::TextColor;
use bevy::ui::prelude::{BackgroundColor, BorderColor, Display, Node, UiRect, Val, px};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::icon::IconTint;
use infiltrator_bevy_widgets::nav::{NavActive, NavLabel, nav_fill, nav_label_ink};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::{Density, DensitySwitch, ResponsiveContext};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{Breakpoint, LightDark, Theme, space};

use crate::controller::FailureDwell;
use crate::pages::overview::{OverviewModePill, OverviewProjectionUpdated};
use crate::projection::OverviewState;
use crate::route::{ActiveRoute, OverviewSourceHandle, Route, RouteChanged};

/// Sidebar rail standard width (px).
pub const SIDEBAR_WIDTH_PX: f32 = 240.0;
/// Identity tile edge (px).
pub const IDENTITY_TILE_PX: f32 = 40.0;

/// Shell layout mode corresponding to responsive breakpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LayoutMode {
    /// Mobile compact layout (<600px): collapsed sidebar + bottom navigation bar.
    BottomNav,
    /// Tablet / split screen medium layout (600px - 1024px): slim rail.
    Rail,
    /// Desktop expanded layout (1024px - 1440px): standard sidebar (240px).
    #[default]
    Sidebar,
    /// Ultrawide layout (>=1440px): wide sidebar (280px).
    Wide,
}

/// Live responsive shell layout state.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ShellLayoutState {
    /// Current viewport or window width in pixels.
    pub width_px: f32,
    /// Active breakpoint category.
    pub breakpoint: Breakpoint,
    /// Active shell layout mode.
    pub mode: LayoutMode,
    /// Active layout density.
    pub density: Density,
}

impl ShellLayoutState {
    /// Construct shell layout state from a viewport / window width in pixels.
    pub fn from_width(width_px: f32) -> Self {
        let breakpoint = Breakpoint::from_width(width_px);
        let mode = match breakpoint {
            Breakpoint::Compact => LayoutMode::BottomNav,
            Breakpoint::Medium => LayoutMode::Rail,
            Breakpoint::Expanded => LayoutMode::Sidebar,
            Breakpoint::Ultra => LayoutMode::Wide,
        };
        Self {
            width_px,
            breakpoint,
            mode,
            density: Density::Comfortable,
        }
    }

    /// Update the viewport width, re-resolving breakpoint and layout mode.
    /// Returns `true` if breakpoint or layout mode changed.
    pub fn set_width(&mut self, width_px: f32) -> bool {
        let next = Self::from_width(width_px);
        if self.width_px != width_px || self.breakpoint != next.breakpoint || self.mode != next.mode
        {
            self.width_px = width_px;
            self.breakpoint = next.breakpoint;
            self.mode = next.mode;
            true
        } else {
            false
        }
    }

    pub fn is_compact(&self) -> bool {
        self.mode == LayoutMode::BottomNav
    }
    pub fn is_rail(&self) -> bool {
        self.mode == LayoutMode::Rail
    }
    pub fn is_sidebar(&self) -> bool {
        self.mode == LayoutMode::Sidebar
    }
    pub fn is_wide(&self) -> bool {
        self.mode == LayoutMode::Wide
    }
}

impl Default for ShellLayoutState {
    fn default() -> Self {
        Self::from_width(1180.0)
    }
}

/// Marker for the content region product pages mount into.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ContentSlot;

/// Marker on the content column for responsive padding scaling.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentColumn;

/// Marker for the shell root entity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ShellRoot;

/// Marker for the title row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ShellHeader;

/// Marker on the top header title text node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContentTitleLabel;

/// Marker for the theme-toggle pill.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ThemeToggle;

/// Marker for the density-toggle pill.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct DensityToggle;

/// Marker on navigation back button (<) (BEVY-GAP-011).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HistoryBackButton;

/// Marker on navigation forward button (>) (BEVY-GAP-011).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HistoryForwardButton;

/// Marker on global running status indicator dot (BEVY-GAP-007).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlobalStatusDot;

/// Marker on global proxy mode capsule (BEVY-GAP-007).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlobalModeCapsule;

/// Marker on the sidebar Script proxy mode pill.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarScriptModePill;

/// Marker on the sidebar system proxy toggle card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarSystemProxyCard;

/// Marker on the sidebar system proxy toggle switch.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarSystemProxyToggle;

/// Marker on the sidebar TUN mode toggle card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarTunCard;

/// Marker on the sidebar TUN mode toggle switch.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarTunToggle;

/// Marker on the sidebar active profile card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarActiveProfileCard;

/// Marker on the sidebar 2x2 shortcut matrix.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarShortcutMatrix;

/// Marker on a shortcut tile in the sidebar matrix.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarShortcutTile(pub Route);

/// Marker on the sidebar live speed footer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarSpeedFooter;

/// Marker on the sidebar rail.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarPanel;

/// Marker on an individual item in the sidebar navigation with its target route.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarNavItem(pub Route);

/// Marker on the bottom navigation bar for mobile compact mode (<600px).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BottomNavBar;

/// Marker on an individual item in the bottom navigation bar with its target route.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BottomNavItem(pub Route);

/// Active state flag for bottom navigation items.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BottomNavActive(pub bool);

/// Marker on the sidebar's foot caption.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarFoot;

/// The shell's mirror of the current appearance.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeMode(pub LightDark);

/// Latch for a proxy-mode command in flight.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModeCommandInFlight(pub bool);

/// The receipt channel of the in-flight mode command.
#[derive(Resource, Debug, Default)]
pub struct PendingModeAck(pub Option<Mutex<Receiver<Result<(), String>>>>);

pub fn window_semantic_node(title: &str) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Window);
    node.set_label(title);
    AccessibilityNode(node)
}

pub fn header_semantic_node(title: &str) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Header);
    node.set_label(title);
    AccessibilityNode(node)
}

pub fn toggle_semantic_node(label: &str) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Button);
    node.set_label(label);
    AccessibilityNode(node)
}

pub fn nav_semantic_node(label: &str, disabled: bool) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Button);
    node.set_label(label);
    if disabled {
        node.set_disabled();
    }
    AccessibilityNode(node)
}

pub fn region_semantic_node(label: &str) -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Region);
    node.set_label(label);
    AccessibilityNode(node)
}

/// Theme toggle observer.
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

/// Density toggle observer.
fn on_density_pill_activated(
    activate: On<Activate>,
    toggles: Query<(), With<DensityToggle>>,
    mut layout: ResMut<ShellLayoutState>,
    mut commands: Commands,
) {
    if !toggles.contains(activate.entity) {
        return;
    }
    let next = match layout.density {
        Density::Comfortable => Density::Compact,
        Density::Compact => Density::Comfortable,
    };
    layout.density = next;
    commands.trigger(DensitySwitch(next));
}

/// Back navigation button observer (BEVY-GAP-011).
fn on_history_back_activated(
    activate: On<Activate>,
    buttons: Query<(), With<HistoryBackButton>>,
    mut commands: Commands,
) {
    if buttons.contains(activate.entity) {
        commands.trigger(crate::route::NavigateBack);
    }
}

/// Forward navigation button observer (BEVY-GAP-011).
fn on_history_forward_activated(
    activate: On<Activate>,
    buttons: Query<(), With<HistoryForwardButton>>,
    mut commands: Commands,
) {
    if buttons.contains(activate.entity) {
        commands.trigger(crate::route::NavigateForward);
    }
}

/// Bottom navigation item activation observer.
fn on_bottom_nav_activated(
    activate: On<Activate>,
    items: Query<&BottomNavItem>,
    mut commands: Commands,
) {
    if let Ok(item) = items.get(activate.entity) {
        commands.trigger(RouteChanged(item.0));
    }
}

/// Sidebar navigation item activation observer.
fn on_sidebar_nav_activated(
    activate: On<Activate>,
    items: Query<&SidebarNavItem>,
    mut commands: Commands,
) {
    if let Ok(item) = items.get(activate.entity) {
        commands.trigger(RouteChanged(item.0));
    }
}

/// Sidebar shortcut tile activation observer.
fn on_sidebar_shortcut_tile_activated(
    activate: On<Activate>,
    tiles: Query<&SidebarShortcutTile>,
    mut commands: Commands,
) {
    if let Ok(tile) = tiles.get(activate.entity) {
        commands.trigger(RouteChanged(tile.0));
    }
}

/// Mode segment pill observer.
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

/// Drain receipt channel for proxy mode switch.
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
    let outcome = match slot
        .get_mut()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .try_recv()
    {
        Ok(receipt) => Some(receipt),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
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
        return;
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

/// App shell plugin.
pub struct ShellPlugin {
    mode: LightDark,
    initial_width_px: Option<f32>,
}

impl ShellPlugin {
    pub fn new(mode: LightDark) -> Self {
        Self {
            mode,
            initial_width_px: None,
        }
    }

    pub fn new_with_width(mode: LightDark, width_px: f32) -> Self {
        Self {
            mode,
            initial_width_px: Some(width_px),
        }
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

        let width = self.initial_width_px.unwrap_or(1180.0);
        let initial_layout = ShellLayoutState::from_width(width);
        app.insert_resource(initial_layout);
        app.insert_resource(ResponsiveContext::new(width, 760.0));

        app.init_resource::<ModeCommandInFlight>();
        app.init_resource::<PendingModeAck>();
        app.add_observer(on_theme_pill_activated);
        app.add_observer(on_density_pill_activated);
        app.add_observer(on_mode_pill_activated);
        app.add_observer(on_bottom_nav_activated);
        app.add_observer(on_sidebar_nav_activated);
        app.add_observer(on_sidebar_shortcut_tile_activated);
        app.add_observer(on_history_back_activated);
        app.add_observer(on_history_forward_activated);
        app.init_resource::<ClearColor>();
        app.add_systems(Startup, (spawn_camera, spawn_shell));
        app.add_systems(
            Update,
            (
                sync_content_title,
                sync_sidebar_panel,
                sync_sidebar_nav_visuals,
                sync_bottom_nav_visuals,
                sync_responsive_shell,
                sync_window_clear,
                drain_mode_ack,
            ),
        );
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn spawn_shell(mut commands: Commands, palette: Res<UiPalette>) {
    commands.spawn_scene(shell_scene("MusicFrog Infiltrator".to_string(), &palette));
}

/// Repaint the sidebar rail from the live palette.
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

/// Sync top content title with the active route.
fn sync_content_title(
    active_route: Option<Res<ActiveRoute>>,
    mut titles: Query<&mut Text, With<ContentTitleLabel>>,
) {
    let target = active_route
        .as_ref()
        .and_then(|r| r.0)
        .unwrap_or(Route::Overview)
        .label();
    for mut text in &mut titles {
        if text.0 != target {
            text.0 = target.to_owned();
        }
    }
}

/// Sync sidebar navigation items with the active route and live palette.
fn sync_sidebar_nav_visuals(
    palette: Res<UiPalette>,
    active_route: Option<Res<ActiveRoute>>,
    mut items: Query<(Entity, &SidebarNavItem, &mut NavActive, &Children)>,
    mut bgs: Query<&mut BackgroundColor>,
    mut texts: Query<(&NavLabel, &mut TextColor, Option<&mut TextRole>)>,
) {
    let current = active_route
        .as_ref()
        .and_then(|r| r.0)
        .unwrap_or(Route::Overview);
    for (entity, item, mut active_marker, children) in &mut items {
        let is_active = item.0 == current;
        if active_marker.0 != is_active {
            active_marker.0 = is_active;
        }
        let target_fill = nav_fill(is_active, &palette);
        if let Ok(mut bg) = bgs.get_mut(entity)
            && bg.0 != target_fill
        {
            bg.0 = target_fill;
        }
        let target_ink = nav_label_ink(is_active, &palette);
        let target_role = if is_active {
            Role::BodyStrong
        } else {
            Role::Body
        };
        for child in children.iter() {
            if let Ok((_, mut text_color, text_role)) = texts.get_mut(*child) {
                if text_color.0 != target_ink {
                    text_color.0 = target_ink;
                }
                if let Some(mut role) = text_role
                    && role.0 != target_role
                {
                    role.0 = target_role;
                }
            }
        }
    }
}

/// Repaint bottom navigation bar and sync active states with ActiveRoute.
fn sync_bottom_nav_visuals(
    palette: Res<UiPalette>,
    active_route: Option<Res<ActiveRoute>>,
    mut bars: Query<(&mut BackgroundColor, &mut BorderColor), With<BottomNavBar>>,
    mut items: Query<(&BottomNavItem, &mut BottomNavActive, &Children)>,
    mut icons: Query<&mut IconTint>,
) {
    let edge = palette.border;
    for (mut fill, mut border) in &mut bars {
        if fill.0 != palette.sidebar {
            fill.0 = palette.sidebar;
        }
        if border.top != edge {
            border.top = edge;
        }
    }

    let current = active_route
        .as_ref()
        .and_then(|r| r.0)
        .unwrap_or(Route::Overview);
    for (item, mut active_marker, children) in &mut items {
        let is_active = item.0 == current;
        if active_marker.0 != is_active {
            active_marker.0 = is_active;
        }
        let target_ink = if is_active {
            palette.accent
        } else {
            palette.ink_dim
        };
        for child in children.iter() {
            if let Ok(mut tint) = icons.get_mut(*child)
                && tint.0 != target_ink
            {
                tint.0 = target_ink;
            }
        }
    }
}

type ContentColFilter = (
    With<ContentColumn>,
    Without<SidebarPanel>,
    Without<BottomNavBar>,
    Without<DensityToggle>,
);
type DensityPillFilter = (
    With<DensityToggle>,
    Without<ContentColumn>,
    Without<SidebarPanel>,
    Without<BottomNavBar>,
);

type ContentColQuery<'w, 's> = Query<'w, 's, &'static mut Node, ContentColFilter>;
type DensityPillQuery<'w, 's> = Query<'w, 's, &'static mut Node, DensityPillFilter>;

/// Update layout mode and toggle sidebar vs bottom navigation bar display based on window width.
fn sync_responsive_shell(
    windows: Option<Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>>,
    mut layout: ResMut<ShellLayoutState>,
    mut responsive_ctx: Option<ResMut<ResponsiveContext>>,
    mut sidebars: Query<&mut Node, (With<SidebarPanel>, Without<BottomNavBar>)>,
    mut bottom_navs: Query<&mut Node, (With<BottomNavBar>, Without<SidebarPanel>)>,
    mut content_cols: ContentColQuery,
    mut density_pills: DensityPillQuery,
) {
    if let Some(windows) = windows
        && let Ok(primary) = windows.single()
    {
        let w = primary.width();
        let h = primary.height();
        if (w - layout.width_px).abs() > 0.5 {
            layout.set_width(w);
            if let Some(ref mut ctx) = responsive_ctx {
                ctx.set_dimensions(w, h);
            }
        }
    }

    let is_compact = layout.mode == LayoutMode::BottomNav;
    for mut node in &mut sidebars {
        if is_compact {
            if node.display != Display::None {
                node.display = Display::None;
            }
        } else {
            if node.display != Display::Flex {
                node.display = Display::Flex;
            }
            let target_w = match layout.mode {
                LayoutMode::Rail => px(72.0),
                LayoutMode::Sidebar => px(SIDEBAR_WIDTH_PX),
                LayoutMode::Wide => px(280.0),
                LayoutMode::BottomNav => px(0.0),
            };
            if node.width != target_w {
                node.width = target_w;
            }
        }
    }

    for mut node in &mut bottom_navs {
        let target = if is_compact {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != target {
            node.display = target;
        }
    }

    let col_pad = if is_compact {
        UiRect::all(Val::Px(space::S12))
    } else {
        UiRect::all(Val::Px(space::S16))
    };
    let col_gap = if is_compact {
        Val::Px(space::S12)
    } else {
        Val::Px(space::S16)
    };
    for mut node in &mut content_cols {
        if node.padding != col_pad {
            node.padding = col_pad;
        }
        if node.row_gap != col_gap {
            node.row_gap = col_gap;
        }
    }

    let density_display = if is_compact {
        Display::None
    } else {
        Display::Flex
    };
    for mut node in &mut density_pills {
        if node.display != density_display {
            node.display = density_display;
        }
    }
}

/// Repaint window canvas clear color.
fn sync_window_clear(palette: Res<UiPalette>, mut clear: Option<ResMut<ClearColor>>) {
    let Some(clear) = clear.as_deref_mut() else {
        return;
    };
    if clear.0 != palette.window_clear {
        clear.0 = palette.window_clear;
    }
}

/// The root shell scene.
pub fn shell_scene(title: String, palette: &UiPalette) -> impl Scene + use<> {
    crate::shell_scene::shell_scene(title, palette)
}

/// The bottom navigation bar for Compact mode (<600px).
pub fn bottom_nav_scene(palette: &UiPalette) -> Box<dyn Scene> {
    crate::shell_scene::bottom_nav_scene(palette)
}
