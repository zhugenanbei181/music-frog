//! Per-App Split Tunneling and Process Routing types for the Iced desktop client.

use infiltrator_desktop::process_enumerator::{ExtendedProcessInfo, ProcessCategory};
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use std::collections::HashMap;

/// State of the interactive App Routing grid.
#[derive(Debug, Clone, Default)]
pub struct AppRoutingState {
    pub processes: Vec<ExtendedProcessInfo>,
    pub filter_query: String,
    pub mode: AppRoutingMode,
    pub custom_rules: HashMap<String, AppRoutingRule>,
    pub is_refreshing: bool,
    pub selected_category: Option<ProcessCategory>,
}
