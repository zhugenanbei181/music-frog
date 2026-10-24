//! App-shell types: navigation routes, toast severity and page transitions.

use infiltrator_contract::command_catalogue::ShellPage;
use infiltrator_contract::rule_tracer::TracerRuleOverride;
use std::path::PathBuf;
use std::time;

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

impl Route {
    /// Map a shared command-catalogue page onto this surface's route. Iced has
    /// no dedicated logs page (the runtime page hosts the logs section) and
    /// keeps the config editor as a local page, so those two are the only
    /// non-identical arms.
    pub fn from_shell_page(page: ShellPage) -> Self {
        match page {
            ShellPage::Overview => Self::Overview,
            ShellPage::Proxies => Self::Proxies,
            ShellPage::Profiles => Self::Profiles,
            ShellPage::Rules => Self::Rules,
            ShellPage::Connections | ShellPage::Logs => Self::Runtime,
            ShellPage::Dns => Self::Dns,
            ShellPage::Doctor => Self::Doctor,
            ShellPage::AppRouting => Self::AppRouting,
            ShellPage::Sync => Self::Sync,
            ShellPage::Settings => Self::Settings,
        }
    }
}

/// DUAL-09-08: how the snapshot diff modal lays the comparison out. The
/// shared `YamlAstDiffSnapshot` carries both the unified and the split rows,
/// so this is a pure presentation choice on the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapshotDiffMode {
    #[default]
    Inline,
    Split,
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
    RuleStatisticsCleanup,
    FactoryReset,
    ClearProfiles,
    DeleteProfile(String),
    DeleteKernel(String),
    /// Roll the active core back to a previously runnable version.
    ///
    /// The version is the owner's rollback target, so the confirmation names
    /// the exact core the rollback would select.
    RollbackCore(String),
    CloseAllConnections,
    TracerOverride(TracerRuleOverride),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncSummary {
    pub uploaded: usize,
    pub downloaded: usize,
    pub conflicts: usize,
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

// The command-palette catalogue, its categories and its typed targets live in
// the shared contract (`infiltrator_contract::command_catalogue`); the Iced
// types module keeps no second local command model.

// User-configured global hotkeys live in the shared contract
// (`infiltrator_contract::shortcuts::ShortcutRegistry`); the shell keeps no
// second local binding model.

// The LAN sharing, LAN security and PAC drafts, the UWP loopback state and
// their baselines live in the shared owner layer
// (`infiltrator_presentation::settings_page_model` and
// `infiltrator_presentation::app_routing_page_model`); the Iced types module
// keeps no second local copy of them.
