//! Cross-surface service-mode privilege state.
//!
//! The platform-specific command and helper details stay in host adapters;
//! this contract only describes what a UI may truthfully render.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceModePlatform {
    WindowsService,
    LinuxPolkit,
    MacosLaunchd,
    #[default]
    Unsupported,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceModeState {
    Ready,
    InstalledStopped,
    NotInstalled,
    MissingPrivilege,
    #[default]
    Unavailable,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceModeSnapshot {
    pub platform: ServiceModePlatform,
    pub state: ServiceModeState,
}
