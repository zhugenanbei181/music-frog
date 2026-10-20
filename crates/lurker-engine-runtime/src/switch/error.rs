//! Typed owner-switch failure.
//!
//! The phase says how far the switch committed, and the disposition says
//! whether the previous binding was restored or the caller must resume from
//! the journal. They are deliberately separate: a failure to release an
//! unstarted candidate can still have rolled the journal back, and a
//! termination failure can leave the journal reserved.

use lurker_core_contract::engine_switch::{EngineSwitchDisposition, EngineSwitchFailurePhase};
use lurker_core_ports::error::PortError;
use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};

/// One failed switch attempt with its phase, adapter cause and disposition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineSwitchError<S> {
    /// How far the switch committed before it failed.
    pub phase: EngineSwitchFailurePhase,
    /// The adapter failure that stopped the switch.
    pub cause: PortError,
    /// Whether the previous binding was restored or the caller must resume.
    pub disposition: EngineSwitchDisposition<S>,
}

impl<S> EngineSwitchError<S> {
    /// Assembles a switch failure from its phase, cause and disposition.
    pub fn new(
        phase: EngineSwitchFailurePhase,
        cause: PortError,
        disposition: EngineSwitchDisposition<S>,
    ) -> Self {
        Self {
            phase,
            cause,
            disposition,
        }
    }

    /// A failure whose journal stage is unfinished and must be resumed.
    pub fn recovery_required(phase: EngineSwitchFailurePhase, cause: PortError) -> Self {
        Self::new(phase, cause, EngineSwitchDisposition::RecoveryRequired)
    }

    /// Whether the caller must resume the switch from the journal.
    pub fn is_recovery_required(&self) -> bool {
        matches!(self.disposition, EngineSwitchDisposition::RecoveryRequired)
    }
}

impl<S> Display for EngineSwitchError<S> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "engine switch failed during {:?}: {}",
            self.phase, self.cause
        )
    }
}

impl<S: Debug> Error for EngineSwitchError<S> {}
