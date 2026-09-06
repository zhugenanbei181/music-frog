//! Shared contract for generated PAC scripts and their local service.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacServiceState {
    Disabled,
    Running { url: String },
    Unavailable { reason: String },
}

/// Confirmed PAC service state. The generated script itself stays inside the
/// host service and is not copied into the cross-surface snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacSnapshot {
    pub state: PacServiceState,
    pub script_bytes: usize,
    pub bypass_domains: Vec<String>,
    pub revision: u64,
}

impl Default for PacSnapshot {
    fn default() -> Self {
        Self {
            state: PacServiceState::Disabled,
            script_bytes: 0,
            bypass_domains: Vec::new(),
            revision: 0,
        }
    }
}

/// User-editable PAC application request. Rules are read from the live
/// runtime gateway by the application, keeping policy generation out of UIs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacRequest {
    pub enabled: bool,
    pub bypass_domains: Vec<String>,
    pub bypass_lan: bool,
    pub minify: bool,
}
