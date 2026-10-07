//! Shell owner.

use crate::admin_server::{AdminServerManager, AdminSharedRuntime, SharedAdminCommandReceiver};
use crate::demo::frame::CaptureFrameGate;
use crate::tray::SharedTrayEventReceiver;
use crate::tray::spec::TrayController;
use crate::types::app::{
    ConfirmAction, Route, RouteHistory, ToastStatus, Transition, UwpLoopbackState,
};
use crate::view::theme::theme_for_skin;
use iced::Rectangle;
use iced::Theme;
use iced::window::Id;
use infiltrator_application::language_choice::LanguageChoiceState;
use infiltrator_contract::command_catalogue::CommandCatalogue;
use infiltrator_contract::ime::ImeCompositionTracker;
use infiltrator_contract::mini_hud::{MiniHudDisplay, MiniHudPlacement};
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::responsive_viewport::ResponsiveViewportSnapshot;
use infiltrator_contract::shell_readout::ShellReadoutSnapshot;
use infiltrator_contract::shortcuts::{ShortcutAction, ShortcutRegistry};
use infiltrator_contract::theme::ThemePreference;
use infiltrator_contract::toast::ToastGate;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

/// 外壳域:导航路由、语言/主题、全局错误与 Toast、托盘、Admin 管理端、
/// 任务计数与 demo 捕获标记(UI-002)。
pub struct ShellState {
    pub readout: ShellReadoutSnapshot,
    pub current_route: Route,
    pub history: RouteHistory,
    /// Shared 4-tier responsive viewport projection. Updated from
    /// [`crate::types::message::Message::WindowResized`]; both the sidebar form
    /// and page grids derive from this single source.
    pub viewport: ResponsiveViewportSnapshot,
    pub error_msg: Option<String>,
    pub transition: Transition,
    pub lang: String,
    pub language_choice: LanguageChoiceState,
    pub language_user_selected: bool,
    pub tray_controller: Option<Box<dyn TrayController>>,
    pub tray_events: Option<SharedTrayEventReceiver>,
    pub admin_enabled: bool,
    pub admin_port: u16,
    pub admin_port_input: String,
    pub admin_server: AdminServerManager,
    pub admin_shared: AdminSharedRuntime,
    pub admin_commands: Option<SharedAdminCommandReceiver>,
    pub is_admin: bool,
    /// 0.20 OS 系统通知总开关（订阅自动更新 / WebDAV 周期同步 / 内核错误），
    /// 镜像 `AppSettings.notifications_enabled`；关闭时
    /// [`crate::notify`] 零开销短路。
    pub notifications_enabled: bool,
    pub close_to_tray: bool,
    pub system_proxy_bypass: String,
    pub last_task_id: usize,
    /// Cooldown for stream-driven tray refreshes (download/sync progress)
    /// so the D-Bus menu is rebuilt at most once per shared interval.
    pub tray_refresh_cooldown: Option<Instant>,
    /// The last live rate badge text pushed to the tray (DUAL-15-02); an
    /// unchanged badge pushes nothing, so an idle shell stays quiet.
    pub tray_last_rate_text: Option<String>,
    pub toasts: Vec<(String, ToastStatus)>,
    /// Stable ids parallel to [`Self::toasts`]: dismissal is by id, so an
    /// evicted toast's expiry task can never remove a neighbour.
    pub toast_ids: Vec<u64>,
    /// Shared dedup gate (`ToastPolicy`), applied at the single toast
    /// ingestion point.
    pub toast_gate: ToastGate,
    /// Monotonic clock baseline for toast admission timestamps.
    pub toast_epoch: Instant,
    pub next_toast_id: u64,
    pub confirmation: Option<ConfirmAction>,
    pub is_factory_resetting: bool,
    pub theme: Theme,
    pub demo: bool,
    pub capture_marker: Option<PathBuf>,
    pub capture_region_bounds: Option<Rectangle>,
    pub capture_scenario: Option<FeatureId>,
    pub capture_marker_written: AtomicBool,
    pub capture_frame: CaptureFrameGate,
    pub command_palette_open: bool,
    pub command_query: String,
    pub command_selected_index: usize,
    /// Shared command-palette catalogue (DUAL-15-05). Rebuilt from the stored
    /// profile list whenever the palette opens so both surfaces list the same
    /// entries plus the same live profile rows.
    pub command_catalogue: CommandCatalogue,
    /// Persisted Mini HUD placement (shared geometry, DUAL-15-04).
    pub mini_hud_placement: MiniHudPlacement,
    /// Mini HUD drag anchor: the cursor's widget-local point and the placement
    /// captured when the drag started.
    pub mini_hud_drag_anchor: Option<MiniHudDragAnchor>,
    /// The host's monitor rectangle once resolved, used for clamping and edge
    /// snapping the HUD placement.
    pub mini_hud_display: Option<MiniHudDisplay>,
    /// The host window id once resolved (single-window desktop app), needed
    /// to move/level the Mini HUD window.
    pub window_id: Option<Id>,
    pub mini_hud_mode: bool,
    pub always_on_top: bool,
    /// Whether the OS window currently has focus (DUAL-15-08). Drives the
    /// shared render-cadence policy: 60 FPS foreground, 2 FPS background.
    pub window_focused: bool,
    /// Shared appearance preference (pinned skin or system follow).
    pub theme_preference: ThemePreference,
    /// Latest OS appearance signal (`true` = the OS prefers dark).
    pub system_prefers_dark: bool,
    /// Shared global-shortcut registry (product defaults until settings load).
    pub shortcut_registry: ShortcutRegistry,
    /// Action awaiting the next captured chord, if any.
    pub hotkey_capture: Option<ShortcutAction>,
    /// Live OS IME composition session (DUAL-15-11). The toolkit text widget
    /// owns the field text; the shell records the session so composing keys are
    /// not treated as global chords and the capability stays one shared fact.
    pub ime: ImeCompositionTracker,
    pub uwp_loopback: UwpLoopbackState,
}

/// Drag state for the Mini HUD window: the cursor's widget-local anchor and
/// the placement mirror when the drag began. The actual window move is issued
/// through the host window task; this only tracks the delta math.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MiniHudDragAnchor {
    pub origin: (f32, f32),
    pub placement: MiniHudPlacement,
}

impl ShellState {
    /// Apply an appearance preference: store it and repaint the resolved skin
    /// against the latest OS signal (shared `ThemePreference` resolution).
    pub fn apply_theme_preference(&mut self, preference: ThemePreference) {
        self.theme_preference = preference;
        self.theme = theme_for_skin(preference.resolve(self.system_prefers_dark));
    }
}
