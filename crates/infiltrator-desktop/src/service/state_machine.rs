//! Service Lifecycle State Machine & Command Sequence Engine.
//!
//! Implements a formal finite state machine (FSM) governing privileged background
//! services across Windows, Linux, and macOS. Validates lifecycle transitions,
//! records state transition history, and manages ordered execution of command sequences.

use std::fmt;
use std::time::Instant;

use super::{ServiceCommand, ServiceResponsePayload};

/// Represents the exhaustive lifecycle state of the background privileged service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleState {
    /// Service or capabilities are not installed on the system.
    Uninstalled,
    /// Service installation or capability provisioning is currently in progress.
    Installing,
    /// Service is installed and present, but daemon/process is stopped.
    InstalledStopped,
    /// Service is transitioning from stopped to running.
    Starting,
    /// Service is actively running and ready to handle privileged requests.
    Running {
        pid: Option<u32>,
        tun_active: bool,
        proxy_active: bool,
    },
    /// Service is shutting down or tearing down routes.
    Stopping,
    /// Service uninstallation is currently in progress.
    Uninstalling,
    /// Service is in a degraded state (e.g. proxy works but TUN routing failed).
    Degraded {
        reason: String,
        tun_active: bool,
        proxy_active: bool,
    },
    /// Service has encountered an error or crashed.
    Error {
        message: String,
        recoverable: bool,
    },
}

impl LifecycleState {
    pub fn is_running(&self) -> bool {
        matches!(self, LifecycleState::Running { .. })
    }

    pub fn is_installed(&self) -> bool {
        !matches!(self, LifecycleState::Uninstalled | LifecycleState::Installing)
    }

    pub fn is_tun_active(&self) -> bool {
        match self {
            LifecycleState::Running { tun_active, .. } => *tun_active,
            LifecycleState::Degraded { tun_active, .. } => *tun_active,
            _ => false,
        }
    }

    pub fn is_proxy_active(&self) -> bool {
        match self {
            LifecycleState::Running { proxy_active, .. } => *proxy_active,
            LifecycleState::Degraded { proxy_active, .. } => *proxy_active,
            _ => false,
        }
    }

    pub fn pid(&self) -> Option<u32> {
        match self {
            LifecycleState::Running { pid, .. } => *pid,
            _ => None,
        }
    }
}

impl fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LifecycleState::Uninstalled => write!(f, "Uninstalled"),
            LifecycleState::Installing => write!(f, "Installing"),
            LifecycleState::InstalledStopped => write!(f, "Installed (Stopped)"),
            LifecycleState::Starting => write!(f, "Starting"),
            LifecycleState::Running { pid, tun_active, proxy_active } => {
                write!(
                    f,
                    "Running (pid={:?}, tun={}, proxy={})",
                    pid, tun_active, proxy_active
                )
            }
            LifecycleState::Stopping => write!(f, "Stopping"),
            LifecycleState::Uninstalling => write!(f, "Uninstalling"),
            LifecycleState::Degraded { reason, tun_active, proxy_active } => {
                write!(
                    f,
                    "Degraded: {} (tun={}, proxy={})",
                    reason, tun_active, proxy_active
                )
            }
            LifecycleState::Error { message, recoverable } => {
                write!(
                    f,
                    "Error (recoverable={}): {}",
                    recoverable, message
                )
            }
        }
    }
}

/// Lifecycle events that trigger state transitions in the state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEvent {
    InstallStart,
    InstallSuccess,
    InstallFailure(String),
    StartRequested,
    StartSuccess {
        pid: Option<u32>,
        tun_active: bool,
    },
    StartFailure(String),
    TunStarted {
        interface_name: Option<String>,
    },
    TunStopped,
    ProxyApplied,
    ProxyCleared,
    Degrade(String),
    HeartbeatMissed,
    StopRequested,
    StopSuccess,
    StopFailure(String),
    UninstallStart,
    UninstallSuccess,
    UninstallFailure(String),
    ProcessCrashed(String),
    Recover,
    Reset,
}

/// Error returned when an invalid or illegal state transition is attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidTransitionError {
    pub from_state: LifecycleState,
    pub event: LifecycleEvent,
    pub reason: String,
}

impl fmt::Display for InvalidTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Invalid lifecycle transition from [{}] via event [{:?}]: {}",
            self.from_state, self.event, self.reason
        )
    }
}

impl std::error::Error for InvalidTransitionError {}

/// Records an entry in the state machine's transition history.
#[derive(Debug, Clone)]
pub struct TransitionRecord {
    pub from: LifecycleState,
    pub to: LifecycleState,
    pub event: LifecycleEvent,
    pub timestamp: Instant,
}

/// A formal deterministic state machine managing the lifecycle of background services.
#[derive(Debug)]
pub struct ServiceStateMachine {
    current_state: LifecycleState,
    history: Vec<TransitionRecord>,
    max_history: usize,
}

impl ServiceStateMachine {
    pub fn new(initial_state: LifecycleState) -> Self {
        Self {
            current_state: initial_state,
            history: Vec::new(),
            max_history: 100,
        }
    }

    pub fn with_max_history(mut self, max_history: usize) -> Self {
        self.max_history = max_history;
        self
    }

    pub fn current_state(&self) -> &LifecycleState {
        &self.current_state
    }

    pub fn is_running(&self) -> bool {
        self.current_state.is_running()
    }

    pub fn is_installed(&self) -> bool {
        self.current_state.is_installed()
    }

    pub fn history(&self) -> &[TransitionRecord] {
        &self.history
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Checks whether an event can be applied to the current state without error.
    pub fn can_apply(&self, event: &LifecycleEvent) -> bool {
        self.next_state(event).is_ok()
    }

    /// Calculates the next state given an event, returning an error if illegal.
    pub fn next_state(&self, event: &LifecycleEvent) -> Result<LifecycleState, InvalidTransitionError> {
        let err = |reason: &str| InvalidTransitionError {
            from_state: self.current_state.clone(),
            event: event.clone(),
            reason: reason.to_string(),
        };

        match (&self.current_state, event) {
            // Uninstalled -> Installing
            (LifecycleState::Uninstalled, LifecycleEvent::InstallStart) => Ok(LifecycleState::Installing),
            (LifecycleState::Uninstalled, LifecycleEvent::InstallSuccess) => Ok(LifecycleState::InstalledStopped),
            (LifecycleState::Uninstalled, LifecycleEvent::Reset) => Ok(LifecycleState::Uninstalled),
            (LifecycleState::Uninstalled, _) => {
                Err(err("Cannot perform operation while service is uninstalled. Install service first."))
            }

            // Installing -> InstalledStopped or Error / Uninstalled
            (LifecycleState::Installing, LifecycleEvent::InstallSuccess) => Ok(LifecycleState::InstalledStopped),
            (LifecycleState::Installing, LifecycleEvent::InstallFailure(msg)) => {
                Ok(LifecycleState::Error {
                    message: format!("Installation failed: {msg}"),
                    recoverable: true,
                })
            }
            (LifecycleState::Installing, LifecycleEvent::Reset) => Ok(LifecycleState::Uninstalled),
            (LifecycleState::Installing, _) => Err(err("Installation is currently in progress")),

            // InstalledStopped -> Starting / Uninstalling / Error
            (LifecycleState::InstalledStopped, LifecycleEvent::StartRequested) => Ok(LifecycleState::Starting),
            (LifecycleState::InstalledStopped, LifecycleEvent::StartSuccess { pid, tun_active }) => {
                Ok(LifecycleState::Running {
                    pid: *pid,
                    tun_active: *tun_active,
                    proxy_active: false,
                })
            }
            (LifecycleState::InstalledStopped, LifecycleEvent::UninstallStart) => Ok(LifecycleState::Uninstalling),
            (LifecycleState::InstalledStopped, LifecycleEvent::UninstallSuccess) => Ok(LifecycleState::Uninstalled),
            (LifecycleState::InstalledStopped, LifecycleEvent::Reset) => Ok(LifecycleState::Uninstalled),
            (LifecycleState::InstalledStopped, LifecycleEvent::InstallStart) => {
                // Reinstallation
                Ok(LifecycleState::Installing)
            }
            (LifecycleState::InstalledStopped, _) => {
                Err(err("Service is stopped. Must start service before dispatching routing commands."))
            }

            // Starting -> Running or Error
            (LifecycleState::Starting, LifecycleEvent::StartSuccess { pid, tun_active }) => {
                Ok(LifecycleState::Running {
                    pid: *pid,
                    tun_active: *tun_active,
                    proxy_active: false,
                })
            }
            (LifecycleState::Starting, LifecycleEvent::StartFailure(msg)) => {
                Ok(LifecycleState::Error {
                    message: format!("Startup failed: {msg}"),
                    recoverable: true,
                })
            }
            (LifecycleState::Starting, LifecycleEvent::ProcessCrashed(msg)) => {
                Ok(LifecycleState::Error {
                    message: format!("Crashed during startup: {msg}"),
                    recoverable: true,
                })
            }
            (LifecycleState::Starting, LifecycleEvent::Reset) => Ok(LifecycleState::InstalledStopped),
            (LifecycleState::Starting, _) => Err(err("Service is currently starting up")),

            // Running -> active operations, Stopping, Degraded, Crashed
            (
                LifecycleState::Running { pid, tun_active: _, proxy_active },
                LifecycleEvent::TunStarted { .. },
            ) => Ok(LifecycleState::Running {
                pid: *pid,
                tun_active: true,
                proxy_active: *proxy_active,
            }),
            (
                LifecycleState::Running { pid, tun_active: _, proxy_active },
                LifecycleEvent::TunStopped,
            ) => Ok(LifecycleState::Running {
                pid: *pid,
                tun_active: false,
                proxy_active: *proxy_active,
            }),
            (
                LifecycleState::Running { pid, tun_active, proxy_active: _ },
                LifecycleEvent::ProxyApplied,
            ) => Ok(LifecycleState::Running {
                pid: *pid,
                tun_active: *tun_active,
                proxy_active: true,
            }),
            (
                LifecycleState::Running { pid, tun_active, proxy_active: _ },
                LifecycleEvent::ProxyCleared,
            ) => Ok(LifecycleState::Running {
                pid: *pid,
                tun_active: *tun_active,
                proxy_active: false,
            }),
            (
                LifecycleState::Running { tun_active, proxy_active, .. },
                LifecycleEvent::Degrade(reason),
            ) => Ok(LifecycleState::Degraded {
                reason: reason.clone(),
                tun_active: *tun_active,
                proxy_active: *proxy_active,
            }),
            (LifecycleState::Running { .. }, LifecycleEvent::StopRequested) => Ok(LifecycleState::Stopping),
            (LifecycleState::Running { .. }, LifecycleEvent::StopSuccess) => Ok(LifecycleState::InstalledStopped),
            (LifecycleState::Running { .. }, LifecycleEvent::ProcessCrashed(msg)) => {
                Ok(LifecycleState::Error {
                    message: format!("Process crashed: {msg}"),
                    recoverable: true,
                })
            }
            (LifecycleState::Running { .. }, LifecycleEvent::HeartbeatMissed) => {
                Ok(LifecycleState::Degraded {
                    reason: "Heartbeat missed from daemon".to_string(),
                    tun_active: false,
                    proxy_active: false,
                })
            }
            (LifecycleState::Running { .. }, LifecycleEvent::Reset) => Ok(LifecycleState::InstalledStopped),
            (LifecycleState::Running { .. }, _) => Err(err("Invalid event while service is running")),

            // Stopping -> InstalledStopped or Error
            (LifecycleState::Stopping, LifecycleEvent::StopSuccess) => Ok(LifecycleState::InstalledStopped),
            (LifecycleState::Stopping, LifecycleEvent::StopFailure(msg)) => {
                Ok(LifecycleState::Error {
                    message: format!("Stop failed: {msg}"),
                    recoverable: true,
                })
            }
            (LifecycleState::Stopping, LifecycleEvent::Reset) => Ok(LifecycleState::InstalledStopped),
            (LifecycleState::Stopping, _) => Err(err("Service is currently stopping")),

            // Uninstalling -> Uninstalled or Error
            (LifecycleState::Uninstalling, LifecycleEvent::UninstallSuccess) => Ok(LifecycleState::Uninstalled),
            (LifecycleState::Uninstalling, LifecycleEvent::UninstallFailure(msg)) => {
                Ok(LifecycleState::Error {
                    message: format!("Uninstallation failed: {msg}"),
                    recoverable: true,
                })
            }
            (LifecycleState::Uninstalling, LifecycleEvent::Reset) => Ok(LifecycleState::Uninstalled),
            (LifecycleState::Uninstalling, _) => Err(err("Uninstallation is currently in progress")),

            // Degraded -> Running / Stopping / Error
            (
                LifecycleState::Degraded { tun_active, proxy_active, .. },
                LifecycleEvent::Recover,
            ) => Ok(LifecycleState::Running {
                pid: None,
                tun_active: *tun_active,
                proxy_active: *proxy_active,
            }),
            (LifecycleState::Degraded { .. }, LifecycleEvent::StopRequested) => Ok(LifecycleState::Stopping),
            (LifecycleState::Degraded { .. }, LifecycleEvent::StopSuccess) => Ok(LifecycleState::InstalledStopped),
            (LifecycleState::Degraded { .. }, LifecycleEvent::ProcessCrashed(msg)) => {
                Ok(LifecycleState::Error {
                    message: format!("Degraded service crashed: {msg}"),
                    recoverable: true,
                })
            }
            (LifecycleState::Degraded { .. }, LifecycleEvent::Reset) => Ok(LifecycleState::InstalledStopped),
            (LifecycleState::Degraded { .. }, _) => Err(err("Service is in degraded state; attempt recovery or stop")),

            // Error -> Recover / Reset / Install
            (LifecycleState::Error { recoverable: true, .. }, LifecycleEvent::Recover) => {
                Ok(LifecycleState::InstalledStopped)
            }
            (LifecycleState::Error { .. }, LifecycleEvent::Reset) => Ok(LifecycleState::InstalledStopped),
            (LifecycleState::Error { .. }, LifecycleEvent::InstallStart) => Ok(LifecycleState::Installing),
            (LifecycleState::Error { .. }, LifecycleEvent::UninstallStart) => Ok(LifecycleState::Uninstalling),
            (LifecycleState::Error { .. }, _) => {
                Err(err("Service is in error state. Recover or reset the state machine first."))
            }
        }
    }

    /// Applies an event to the state machine, mutating the current state and appending to history.
    pub fn apply(&mut self, event: LifecycleEvent) -> Result<&LifecycleState, InvalidTransitionError> {
        let next = self.next_state(&event)?;
        let prev = std::mem::replace(&mut self.current_state, next);

        self.history.push(TransitionRecord {
            from: prev,
            to: self.current_state.clone(),
            event,
            timestamp: Instant::now(),
        });

        if self.history.len() > self.max_history {
            self.history.remove(0);
        }

        Ok(&self.current_state)
    }
}

/// A structured sequence of commands to execute sequentially against a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSequence {
    pub name: String,
    pub commands: Vec<ServiceCommand>,
}

impl CommandSequence {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            commands: Vec::new(),
        }
    }

    pub fn then(mut self, command: ServiceCommand) -> Self {
        self.commands.push(command);
        self
    }

    /// Generates a sequence for starting full TUN mode.
    pub fn tun_startup_sequence(
        tun_interface: Option<String>,
        config_path: Option<String>,
    ) -> Self {
        Self::new("TunStartup")
            .then(ServiceCommand::QueryStatus)
            .then(ServiceCommand::StartTun {
                tun_interface,
                config_path,
            })
            .then(ServiceCommand::QueryStatus)
    }

    /// Generates a sequence for configuring system proxy.
    pub fn system_proxy_sequence(
        endpoint: impl Into<String>,
        bypass: Option<String>,
    ) -> Self {
        Self::new("SystemProxyConfigure")
            .then(ServiceCommand::QueryStatus)
            .then(ServiceCommand::SetSystemProxy {
                endpoint: endpoint.into(),
                bypass,
            })
            .then(ServiceCommand::QueryStatus)
    }

    /// Generates a sequence for full teardown.
    pub fn teardown_sequence() -> Self {
        Self::new("Teardown")
            .then(ServiceCommand::ClearSystemProxy)
            .then(ServiceCommand::StopTun)
            .then(ServiceCommand::QueryStatus)
    }
}

/// Result of executing a command sequence.
#[derive(Debug, Clone)]
pub struct SequenceExecutionResult {
    pub sequence_name: String,
    pub step_results: Vec<(ServiceCommand, Result<ServiceResponsePayload, String>)>,
    pub success: bool,
}

impl SequenceExecutionResult {
    pub fn all_successful(&self) -> bool {
        self.success && self.step_results.iter().all(|(_, r)| r.is_ok())
    }
}
