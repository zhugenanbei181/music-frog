//! App-shell types: navigation routes, toast severity and page transitions.

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
