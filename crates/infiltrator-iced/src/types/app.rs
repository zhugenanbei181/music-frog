//! App-shell types: navigation routes, toast severity and page transitions.

use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
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
