//! App-shell types: navigation routes, toast severity and page transitions.

use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Route {
    #[default]
    Overview,
    Profiles,
    Proxies,
    Runtime,
    Rules,
    Dns,
    Sync,
    Editor,
    Settings,
    AppRouting,
    Doctor,
}

/// Navigation history stack tracking back/forward navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteHistory {
    pub back_stack: Vec<Route>,
    pub forward_stack: Vec<Route>,
    pub max_depth: usize,
}

impl Default for RouteHistory {
    fn default() -> Self {
        Self {
            back_stack: vec![Route::default()],
            forward_stack: Vec::new(),
            max_depth: 50,
        }
    }
}

impl RouteHistory {
    pub fn new(max_depth: usize) -> Self {
        Self {
            back_stack: vec![Route::default()],
            forward_stack: Vec::new(),
            max_depth,
        }
    }

    pub fn current(&self) -> Route {
        self.back_stack.last().copied().unwrap_or_default()
    }

    pub fn push(&mut self, route: Route) {
        if self.current() == route {
            return;
        }
        self.back_stack.push(route);
        if self.back_stack.len() > self.max_depth {
            self.back_stack.remove(0);
        }
        self.forward_stack.clear();
    }

    pub fn replace(&mut self, route: Route) {
        if let Some(top) = self.back_stack.last_mut() {
            *top = route;
        } else {
            self.back_stack.push(route);
        }
        self.forward_stack.clear();
    }

    pub fn can_go_back(&self) -> bool {
        self.back_stack.len() > 1
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    pub fn go_back(&mut self) -> Option<Route> {
        if !self.can_go_back() {
            return None;
        }
        let current = self.back_stack.pop()?;
        self.forward_stack.push(current);
        Some(self.current())
    }

    pub fn go_forward(&mut self) -> Option<Route> {
        let next = self.forward_stack.pop()?;
        self.back_stack.push(next);
        Some(next)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToastStatus {
    Info,
    Success,
    Warning,
    Error,
}

/// Destructive actions are staged behind a single confirmation surface so a
/// page cannot accidentally perform an irreversible operation on a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    FactoryReset,
    ClearProfiles,
    DeleteProfile(String),
    DeleteKernel(String),
    CloseAllConnections,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncSummary {
    pub uploaded: usize,
    pub downloaded: usize,
    pub conflicts: usize,
    pub active_profile_changed: bool,
    pub conflict_files: Vec<SyncConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncConflict {
    pub profile: String,
    pub remote_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncProgress {
    pub phase: String,
    pub current: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreDownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub previous_route: Option<Route>,
    pub start_time: Option<Instant>,
    pub duration: std::time::Duration,
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            previous_route: None,
            start_time: None,
            duration: std::time::Duration::from_millis(300),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Navigation,
    Modes,
    Actions,
    Profiles,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    Navigate(Route),
    SetMode(String),
    ToggleSystemProxy,
    ToggleTun,
    FlushFakeIp,
    SpeedTestAll,
    CloseAllConnections,
    RestartKernel,
    SwitchProfile(String),
    ToggleMiniHud,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItem {
    pub id: String,
    pub title_key: &'static str,
    pub category: CommandCategory,
    pub shortcut_hint: Option<&'static str>,
    pub action: CommandAction,
}

/// User-configured global hotkey binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub id: String,
    pub action_title_key: &'static str,
    pub combo: String,
    pub enabled: bool,
}

/// Individual UWP package metadata for Windows loopback exemption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UwpAppItem {
    pub sid: String,
    pub display_name: String,
    pub is_exempt: bool,
}

/// State for the Windows UWP Loopback Utility.
#[derive(Debug, Clone, Default)]
pub struct UwpLoopbackState {
    pub apps: Vec<UwpAppItem>,
    pub search_query: String,
    pub is_scanning: bool,
    pub status_message: Option<String>,
}

/// Configuration for the local PAC (Proxy Auto-Config) service and bypass manager.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PacManagerConfig {
    pub pac_url: String,
    pub bypass_subnets: String,
    pub is_pac_mode_active: bool,
    pub last_compile_status: Option<String>,
}

/// Configuration for LAN proxy sharing and IP access control lists (ACL).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LanSharingConfig {
    pub allow_lan: bool,
    pub mixed_port: u16,
    pub acl_whitelist_cidrs: String,
    pub active_lan_clients_count: usize,
}
