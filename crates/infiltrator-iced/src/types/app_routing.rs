//! Per-App Split Tunneling and Process Routing types for the Iced desktop client.

use infiltrator_desktop::process_enumerator::{ExtendedProcessInfo, ProcessCategory};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AppRoutingMode {
    #[default]
    Global,
    Whitelist,
    Blacklist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AppRouteRule {
    #[default]
    Proxy,
    Direct,
    Block,
}

impl AppRouteRule {
    pub fn next(self) -> Self {
        match self {
            Self::Proxy => Self::Direct,
            Self::Direct => Self::Block,
            Self::Block => Self::Proxy,
        }
    }
}

/// State of the interactive App Routing grid.
#[derive(Debug, Clone, Default)]
pub struct AppRoutingState {
    pub processes: Vec<ExtendedProcessInfo>,
    pub filter_query: String,
    pub mode: AppRoutingMode,
    pub custom_rules: HashMap<String, AppRouteRule>,
    pub is_refreshing: bool,
    pub selected_category: Option<ProcessCategory>,
}
