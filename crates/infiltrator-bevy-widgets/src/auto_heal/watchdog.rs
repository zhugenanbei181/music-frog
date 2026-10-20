//! Watchdog behaviour: the typed repair stage and its timeout-driven abort.

use super::{HealingWatchdog, WatchdogStage, WatchdogTransitionError};
use std::time::Duration;

impl Default for HealingWatchdog {
    fn default() -> Self {
        Self {
            stage: WatchdogStage::Idle,
            elapsed_time: Duration::ZERO,
            deadlock_limit: Duration::from_secs(10),
        }
    }
}

impl HealingWatchdog {
    pub fn new(limit: Duration) -> Self {
        Self {
            deadlock_limit: limit,
            ..Default::default()
        }
    }

    /// The stage the watchdog is in.
    pub fn stage(&self) -> &WatchdogStage {
        &self.stage
    }

    /// Whether a repair action is currently under the watchdog.
    pub fn is_repairing(&self) -> bool {
        matches!(self.stage, WatchdogStage::Repairing { .. })
    }

    /// Whether the watchdog tripped and aborted the repair.
    pub fn has_tripped(&self) -> bool {
        matches!(self.stage, WatchdogStage::Tripped { .. })
    }

    /// Name of the action under the watchdog, if any.
    pub fn active_action_name(&self) -> Option<&str> {
        match &self.stage {
            WatchdogStage::Idle => None,
            WatchdogStage::Repairing { action_name }
            | WatchdogStage::Tripped { action_name } => Some(action_name),
        }
    }

    /// Arm watchdog for a new repair action.
    ///
    /// Arming while a repair is already running is refused: the running stage
    /// comes back in [`WatchdogTransitionError::from`], so one watchdog never
    /// tracks two actions at once. Arming after a trip restarts the watchdog
    /// for the next action.
    pub fn arm(&mut self, action_name: impl Into<String>) -> Result<(), WatchdogTransitionError> {
        if let WatchdogStage::Repairing { .. } = &self.stage {
            return Err(WatchdogTransitionError {
                from: self.stage.clone(),
            });
        }
        self.stage = WatchdogStage::Repairing {
            action_name: action_name.into(),
        };
        self.elapsed_time = Duration::ZERO;
        Ok(())
    }

    /// Disarm watchdog upon clean action completion.
    pub fn disarm(&mut self) {
        self.stage = WatchdogStage::Idle;
        self.elapsed_time = Duration::ZERO;
    }

    /// Step watchdog timer. Returns true if deadlock limit reached, forcibly aborting repair.
    ///
    /// Only the [`WatchdogStage::Repairing`] stage has a running timer; every
    /// other stage refuses the tick.
    pub fn tick(&mut self, dt: Duration) -> bool {
        let WatchdogStage::Repairing { action_name } = &self.stage else {
            return false;
        };
        self.elapsed_time += dt;
        if self.elapsed_time >= self.deadlock_limit {
            self.stage = WatchdogStage::Tripped {
                action_name: action_name.clone(),
            };
            true
        } else {
            false
        }
    }
}
