//! The app shell: left sidebar/rail/bottom-nav and content column composed with `bsn!`.
//! Seams: Responsive layout breakpoints (Compact, Medium, Expanded, Ultra), theme/density toggles,
//! AccessKit semantic nodes, and mode segment control (BEVY-005).

#[path = "app_query_access.rs"]
pub mod query_access;
use self::query_access::{
    SyncResponsiveShellBottomNavsFilter, SyncResponsiveShellSidebarsFilter,
    SyncSafeAreaInsetsBottomNavsFilter, SyncSafeAreaInsetsChromeBarsFilter,
    SyncSafeAreaInsetsShellRootsFilter,
};

use crate::appearance::{
    SystemAppearance, ThemeMode, on_theme_pill_activated, resolved_skin, sync_system_appearance,
};
use crate::chrome::WindowChromePlugin;
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_palette_shell::CommandPalettePlugin;
use crate::gesture::{GestureHostReport, ShellGesturePlugin};
use crate::host_capabilities::HostCapabilitiesPlugin;
use crate::ime::ShellImePlugin;
use crate::localization::LocalizationPlugin;
use crate::mini_hud_shell::MiniHudPlugin;
use crate::pages::profiles_editor::ProfilesEditorPlugin;
use crate::pages::profiles_editor_panes_sync::ProfilesEditorPanesPlugin;
use crate::route::{ActiveRoute, NavigateBack, NavigateForward, Route, RouteChanged};
use crate::shell_rail::{
    sync_bottom_nav_visuals, sync_rail_nav_tooltips, sync_sidebar_nav_visuals,
    sync_sidebar_rail_morphology,
};
use crate::shell_readout::{sync_core_status, sync_quota, sync_text};
use crate::shell_scene::shell_scene_with_toggles;
use crate::shell_waveform;
use crate::shortcuts::ShortcutsPlugin;
use crate::toast::ShellToastPlugin;
use crate::tray_status::TrayStatusPlugin;
use crate::{shell_mode_issue, shell_modes};
use bevy::app::{App, Plugin, Startup, Update};
use bevy::camera::{Camera2d, ClearColor};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::CommandsSceneExt;
use bevy::ui::prelude::{BackgroundColor, Display, Node, UiRect, Val, px};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, Window};
use infiltrator_application::proxy_mode_actions::{PendingModeChange, ProxyModeActions};
use infiltrator_application::system_toggle_application::SystemToggleApplication;
use infiltrator_application::system_toggle_projection::compact_label;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual, PillLabel};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::{
    Density, DensitySwitch, ResponsiveContext, SafeAreaInsets,
};
use infiltrator_bevy_widgets::theme::{Breakpoint, Theme, space};
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::Failure;
use infiltrator_contract::shell_gesture;
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleSnapshot};
use infiltrator_contract::theme::ThemePreference;
use infiltrator_contract::window_chrome::CHROME_DRAG_STRIP_HEIGHT_PX;
use std::sync::Mutex;
use std::sync::mpsc::Receiver;

/// Sidebar rail standard width (px).
pub const SIDEBAR_WIDTH_PX: f32 = 240.0;
/// Sidebar rail mode slim width (px).
pub const SIDEBAR_RAIL_WIDTH_PX: f32 = 64.0;
/// Sidebar wide mode width (px).
pub const SIDEBAR_WIDE_WIDTH_PX: f32 = 280.0;
/// Identity tile edge (px).
pub const IDENTITY_TILE_PX: f32 = 40.0;
/// Standard height of the bottom navigation bar in Compact mode (px).
pub const BOTTOM_NAV_HEIGHT_PX: f32 = 58.0;

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

/// The shell-level mirror of the shared system proxy/TUN toggle projection.
/// PagesPlugin replaces it from each accepted SurfaceSnapshot; ShellPlugin
/// alone starts in `Unknown` and therefore renders non-actionable controls.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct SidebarToggleProjection(pub SystemToggleSnapshot);

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

/// Marker on the sidebar identity text block ("MusicFrog" + version), hidden in rail mode.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarIdentityText;

/// Marker on the sidebar proxy mode segmented control, hidden in rail mode.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarModeSegment;

/// Marker on the sidebar bottom footer row, hidden in rail mode.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarFooterRow;

/// Marker on the sidebar nav item spacer between icon and label, hidden in rail mode.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavSpacer;

/// Marker on floating rail navigation tooltip bubble.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RailNavTooltip(pub Route);

/// Marker on sidebar elements that are only visible in Expanded/Wide mode and collapsed in Rail mode.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidebarExpandedOnly;

/// The receipt channel of the in-flight mode command.
#[derive(Resource, Debug, Default)]
pub struct PendingModeAck(pub Option<ModeAckSlot>);

#[derive(Debug)]
pub struct ModeAckSlot {
    pub request: PendingModeChange,
    pub receiver: Mutex<Receiver<Result<ProxyMode, Failure>>>,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct ModeActionState(pub ProxyModeActions);

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
        commands.trigger(NavigateBack);
    }
}

/// Forward navigation button observer (BEVY-GAP-011).
fn on_history_forward_activated(
    activate: On<Activate>,
    buttons: Query<(), With<HistoryForwardButton>>,
    mut commands: Commands,
) {
    if buttons.contains(activate.entity) {
        commands.trigger(NavigateForward);
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

/// Sidebar system-proxy toggle: resolve the desired state through the shared
/// application policy before sending the UI command.
fn on_sidebar_system_proxy_activated(
    activate: On<Activate>,
    toggles: Query<(), With<SidebarSystemProxyToggle>>,
    projection: Res<SidebarToggleProjection>,
    handle: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
) {
    if toggles.get(activate.entity).is_err() {
        return;
    }
    let desired = !projection.0.state(SystemToggle::SystemProxy).is_enabled();
    if SystemToggleApplication::intent(&projection.0, SystemToggle::SystemProxy, desired).is_err() {
        return;
    }
    let Some(handle) = handle else {
        return;
    };
    handle.submit(UiCommand::SetSystemProxy { enabled: desired });
    commands.insert_resource(SidebarToggleProjection(
        projection
            .0
            .clone()
            .with_pending(SystemToggle::SystemProxy, desired),
    ));
}

/// Sidebar TUN toggle, using the same application policy as the Iced switch.
fn on_sidebar_tun_activated(
    activate: On<Activate>,
    toggles: Query<(), With<SidebarTunToggle>>,
    projection: Res<SidebarToggleProjection>,
    handle: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
) {
    if toggles.get(activate.entity).is_err() {
        return;
    }
    let desired = !projection.0.state(SystemToggle::Tun).is_enabled();
    if SystemToggleApplication::intent(&projection.0, SystemToggle::Tun, desired).is_err() {
        return;
    }
    let Some(handle) = handle else {
        return;
    };
    handle.submit(UiCommand::ToggleTun { enabled: desired });
    commands.insert_resource(SidebarToggleProjection(
        projection
            .0
            .clone()
            .with_pending(SystemToggle::Tun, desired),
    ));
}

/// App shell plugin.
pub struct ShellPlugin {
    preference: ThemePreference,
    initial_width_px: Option<f32>,
}

impl ShellPlugin {
    pub fn new(preference: ThemePreference) -> Self {
        Self {
            preference,
            initial_width_px: None,
        }
    }

    pub fn new_with_width(preference: ThemePreference, width_px: f32) -> Self {
        Self {
            preference,
            initial_width_px: Some(width_px),
        }
    }
}

impl Default for ShellPlugin {
    fn default() -> Self {
        Self::new(ThemePreference::default())
    }
}

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LocalizationPlugin);
        app.add_plugins(WidgetsPlugin::new(&Theme::for_mode(resolved_skin(
            self.preference,
            None,
        ))));
        app.add_plugins(ShortcutsPlugin);
        app.add_plugins(ShellToastPlugin);
        app.add_plugins(CommandPalettePlugin);
        app.add_plugins(ProfilesEditorPlugin);
        app.add_plugins(ProfilesEditorPanesPlugin);
        app.add_plugins(MiniHudPlugin);
        // DUAL-15-13/02: the frameless chrome wiring and the honest tray
        // capability report are part of the shell, so headless compositions
        // get the same facts as the windowed launcher.
        app.add_plugins(WindowChromePlugin);
        app.add_plugins(TrayStatusPlugin);
        // DUAL-15-11: the OS IME path (enable + caret area + composition) is
        // part of the shell, so headless compositions see the same plan.
        app.add_plugins(ShellImePlugin);
        // DUAL-15-07: the real touch-gesture consumer (Bevy `TouchInput` →
        // widget recognizer → shared semantic snapshot) is part of the shell,
        // so headless compositions exercise the same recognition path.
        app.add_plugins(ShellGesturePlugin);
        // BANDROID-014: the typed host seams (haptics capability gate and the
        // reduce-motion / energy preference input) are part of the shell, so a
        // headless composition and the native host share the same defaults.
        app.add_plugins(HostCapabilitiesPlugin::default());
        app.insert_resource(ThemeMode(self.preference));
        app.init_resource::<SystemAppearance>();

        let width = self.initial_width_px.unwrap_or(1180.0);
        let initial_layout = ShellLayoutState::from_width(width);
        app.insert_resource(initial_layout);
        app.insert_resource(ResponsiveContext::new(width, 760.0));

        app.init_resource::<PendingModeAck>();
        app.init_resource::<ModeActionState>();
        app.init_resource::<SidebarToggleProjection>();
        app.add_systems(Update, (sync_text, sync_quota, sync_core_status));
        app.add_systems(Update, shell_waveform::sync);
        app.add_systems(
            Update,
            (
                shell_modes::sync,
                shell_modes::sync_segments,
                shell_mode_issue::sync,
            )
                .chain()
                .after(shell_modes::drain_ack),
        );
        app.init_resource::<SafeAreaInsets>();
        app.add_observer(on_theme_pill_activated);
        app.add_observer(on_density_pill_activated);
        app.add_observer(shell_modes::activate);
        app.add_observer(shell_modes::request);
        app.add_observer(shell_mode_issue::retry);
        app.add_observer(shell_mode_issue::dismiss);
        app.add_observer(shell_mode_issue::settings);
        app.add_observer(on_bottom_nav_activated);
        app.add_observer(on_sidebar_nav_activated);
        app.add_observer(on_sidebar_shortcut_tile_activated);
        app.add_observer(on_sidebar_system_proxy_activated);
        app.add_observer(on_sidebar_tun_activated);
        app.add_observer(on_history_back_activated);
        app.add_observer(on_history_forward_activated);
        app.init_resource::<ClearColor>();
        app.add_systems(Startup, (spawn_camera, spawn_shell));
        app.add_systems(
            Update,
            (
                sync_content_title,
                sync_sidebar_panel,
                sync_sidebar_toggle_visuals,
                sync_sidebar_nav_visuals,
                sync_bottom_nav_visuals,
                sync_responsive_shell,
                sync_sidebar_rail_morphology,
                sync_rail_nav_tooltips,
                sync_safe_area_insets.after(sync_responsive_shell),
                sync_window_clear,
                sync_system_appearance,
                shell_modes::drain_ack,
            ),
        );
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn spawn_shell(
    mut commands: Commands,
    palette: Res<UiPalette>,
    toggles: Res<SidebarToggleProjection>,
) {
    commands.spawn_scene(shell_scene_with_toggles(
        "MusicFrog Infiltrator".to_string(),
        &toggles.0,
        &palette,
    ));
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

/// Restamp the two compact sidebar controls from the shared projection. A
/// pending/unknown/unsupported control is visibly non-actionable and cannot
/// be toggled by the observers above.
type SidebarToggleQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut ControlVisual,
        &'static Children,
        Option<&'static SidebarSystemProxyToggle>,
        Option<&'static SidebarTunToggle>,
    ),
>;

fn sync_sidebar_toggle_visuals(
    projection: Res<SidebarToggleProjection>,
    locale: Res<UiLocale>,
    mut toggles: SidebarToggleQuery,
    mut labels: Query<&mut Text, With<PillLabel>>,
    mut commands: Commands,
) {
    for (entity, mut visual, children, proxy_marker, tun_marker) in &mut toggles {
        let Some(state) = proxy_marker
            .map(|_| &projection.0.system_proxy)
            .or_else(|| tun_marker.map(|_| &projection.0.tun))
        else {
            continue;
        };
        visual.0 = state.is_enabled();
        for child in children.iter() {
            if let Ok(mut label) = labels.get_mut(*child) {
                label.0 = compact_label(state, locale.code());
            }
        }
        if state.can_toggle() {
            commands.entity(entity).remove::<ButtonDisabled>();
        } else {
            commands.entity(entity).insert(ButtonDisabled(true));
        }
    }
}

/// Sync top content title with the active route.
fn sync_content_title(
    active_route: Option<Res<ActiveRoute>>,
    locale: Res<UiLocale>,
    mut titles: Query<(&mut Text, &mut LocalizedText), With<ContentTitleLabel>>,
) {
    let target = active_route
        .as_ref()
        .and_then(|r| r.0)
        .unwrap_or(Route::Overview)
        .label_key();
    for (mut text, mut copy) in &mut titles {
        if copy.key != target {
            *copy = LocalizedText::plain(target);
        }
        text.0 = copy.render(&locale);
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
    windows: Option<Query<&Window, With<PrimaryWindow>>>,
    mut layout: ResMut<ShellLayoutState>,
    mut responsive_ctx: Option<ResMut<ResponsiveContext>>,
    mut sidebars: Query<&mut Node, SyncResponsiveShellSidebarsFilter>,
    mut bottom_navs: Query<&mut Node, SyncResponsiveShellBottomNavsFilter>,
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
    let is_rail = layout.mode == LayoutMode::Rail;
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
                LayoutMode::Rail => px(SIDEBAR_RAIL_WIDTH_PX),
                LayoutMode::Sidebar => px(SIDEBAR_WIDTH_PX),
                LayoutMode::Wide => px(SIDEBAR_WIDE_WIDTH_PX),
                LayoutMode::BottomNav => px(0.0),
            };
            if node.width != target_w {
                node.width = target_w;
            }
            let target_padding = if is_rail {
                UiRect::all(Val::Px(space::S8))
            } else {
                UiRect::all(Val::Px(space::S12))
            };
            if node.padding != target_padding {
                node.padding = target_padding;
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

/// Synchronize safe-area insets (status bar, gesture navigation bar, camera cutouts)
/// across ShellRoot, ChromeDragBar, and BottomNavBar.
pub fn sync_safe_area_insets(
    mut safe_insets: Option<ResMut<SafeAreaInsets>>,
    mut host_report: Option<ResMut<GestureHostReport>>,
    layout: Res<ShellLayoutState>,
    mut shell_roots: Query<&mut Node, SyncSafeAreaInsetsShellRootsFilter>,
    mut bottom_navs: Query<&mut Node, SyncSafeAreaInsetsBottomNavsFilter>,
    mut chrome_bars: Query<&mut Node, SyncSafeAreaInsetsChromeBarsFilter>,
) {
    let host_changed = host_report.as_ref().is_some_and(|r| r.is_changed());
    let widget_changed = safe_insets.as_ref().is_some_and(|w| w.is_changed());

    let (insets_top, insets_bottom, insets_left, insets_right) = if widget_changed && !host_changed
    {
        let w = safe_insets.as_ref().unwrap();
        (w.top_px, w.bottom_px, w.left_px, w.right_px)
    } else if host_changed && !widget_changed {
        let r = host_report.as_ref().unwrap();
        (r.insets.top, r.insets.bottom, r.insets.left, r.insets.right)
    } else {
        let r_top = host_report.as_ref().map_or(0.0, |r| r.insets.top);
        let r_bottom = host_report.as_ref().map_or(0.0, |r| r.insets.bottom);
        let r_left = host_report.as_ref().map_or(0.0, |r| r.insets.left);
        let r_right = host_report.as_ref().map_or(0.0, |r| r.insets.right);

        let w_top = safe_insets.as_ref().map_or(0.0, |w| w.top_px);
        let w_bottom = safe_insets.as_ref().map_or(0.0, |w| w.bottom_px);
        let w_left = safe_insets.as_ref().map_or(0.0, |w| w.left_px);
        let w_right = safe_insets.as_ref().map_or(0.0, |w| w.right_px);

        if w_top > 0.0 || w_bottom > 0.0 || w_left > 0.0 || w_right > 0.0 {
            (w_top, w_bottom, w_left, w_right)
        } else {
            (r_top, r_bottom, r_left, r_right)
        }
    };

    // Bidirectional sync between widget SafeAreaInsets and contract GestureHostReport
    if let Some(ref mut widget_insets) = safe_insets
        && ((widget_insets.top_px - insets_top).abs() > 0.001
            || (widget_insets.bottom_px - insets_bottom).abs() > 0.001
            || (widget_insets.left_px - insets_left).abs() > 0.001
            || (widget_insets.right_px - insets_right).abs() > 0.001)
    {
        widget_insets.top_px = insets_top;
        widget_insets.bottom_px = insets_bottom;
        widget_insets.left_px = insets_left;
        widget_insets.right_px = insets_right;
    }
    if let Some(ref mut report) = host_report
        && ((report.insets.top - insets_top).abs() > 0.001
            || (report.insets.bottom - insets_bottom).abs() > 0.001
            || (report.insets.left - insets_left).abs() > 0.001
            || (report.insets.right - insets_right).abs() > 0.001)
    {
        report.insets = shell_gesture::SafeAreaInsets::new(
            insets_top,
            insets_right,
            insets_bottom,
            insets_left,
        );
    }

    // 1. Bottom Navigation Bar safe area avoidance (bottom gesture bar)
    let bottom_nav_height = Val::Px(BOTTOM_NAV_HEIGHT_PX + insets_bottom);
    let bottom_nav_padding = if insets_bottom > 0.0 || insets_left > 0.0 || insets_right > 0.0 {
        UiRect::new(
            Val::Px(insets_left),
            Val::Px(insets_right),
            Val::Px(0.0),
            Val::Px(space::S6 + insets_bottom),
        )
    } else {
        UiRect::bottom(Val::Px(space::S6))
    };

    for mut node in &mut bottom_navs {
        if node.height != bottom_nav_height {
            node.height = bottom_nav_height;
        }
        if node.min_height != bottom_nav_height {
            node.min_height = bottom_nav_height;
        }
        if node.padding != bottom_nav_padding {
            node.padding = bottom_nav_padding;
        }
    }

    // 2. Chrome Drag Bar safe area avoidance (status bar / notch)
    let chrome_height = Val::Px(CHROME_DRAG_STRIP_HEIGHT_PX as f32 + insets_top);
    let chrome_padding = if insets_top > 0.0 || insets_left > 0.0 || insets_right > 0.0 {
        UiRect::new(
            Val::Px(space::S12 + insets_left),
            Val::Px(space::S12 + insets_right),
            Val::Px(insets_top),
            Val::Px(0.0),
        )
    } else {
        UiRect::horizontal(Val::Px(space::S12))
    };

    let chrome_bar_count = chrome_bars.iter().count();
    for mut node in &mut chrome_bars {
        if node.height != chrome_height {
            node.height = chrome_height;
        }
        if node.min_height != chrome_height {
            node.min_height = chrome_height;
        }
        if node.padding != chrome_padding {
            node.padding = chrome_padding;
        }
    }

    // 3. Shell Root horizontal & fallback bottom/top safe area avoidance
    let is_compact = layout.mode == LayoutMode::BottomNav;
    let root_bottom_pad = if !is_compact && insets_bottom > 0.0 {
        insets_bottom
    } else {
        0.0
    };
    let root_top_pad = if chrome_bar_count == 0 && insets_top > 0.0 {
        insets_top
    } else {
        0.0
    };
    let root_padding = UiRect::new(
        Val::Px(insets_left),
        Val::Px(insets_right),
        Val::Px(root_top_pad),
        Val::Px(root_bottom_pad),
    );

    for mut node in &mut shell_roots {
        if node.padding != root_padding {
            node.padding = root_padding;
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
