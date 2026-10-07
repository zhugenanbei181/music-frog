//! Toolkit-neutral lifecycle controls. Labels and actions are projected from actual host facts.
use crate::command::CommandIntent;
use crate::error::Failure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreControlAction {
    Start,
    Stop,
}
impl CoreControlAction {
    pub fn intent(self) -> CommandIntent {
        match self {
            Self::Start => CommandIntent::StartCore,
            Self::Stop => CommandIntent::StopCore,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreControlLabel {
    Start,
    Stop,
    Retry,
    Starting,
    Stopping,
    Pending,
    Unsupported,
    Unavailable,
}
impl CoreControlLabel {
    pub fn key(self) -> &'static str {
        match self {
            Self::Start => "start_proxy",
            Self::Stop => "stop_proxy",
            Self::Retry => "core_control_retry",
            Self::Starting => "core_control_starting",
            Self::Stopping => "core_control_stopping",
            Self::Pending => "core_control_pending",
            Self::Unsupported => "core_control_unsupported",
            Self::Unavailable => "core_control_unavailable_hint",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreControlProjection {
    pub action: Option<CoreControlAction>,
    pub label: CoreControlLabel,
    pub issue: Option<Failure>,
}
