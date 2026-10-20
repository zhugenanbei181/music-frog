//! Native adapters correlate terminal DNS probe feedback without completing writes on snapshots.
//!
//! The reducer owns one explicit phase. A probe is only ever completed by the
//! terminal receipt of the request it started; a snapshot observation never
//! completes a write.
use lurker_core_contract::error::{ErrorCode, Failure};

/// The exhaustive phase of the DNS leak probe reducer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DnsLeakActionPhase {
    /// No probe is running.
    #[default]
    Idle,
    /// A probe is running.
    Pending {
        /// The monotonic identity of the running probe.
        token: u64,
    },
    /// The probe failed.
    Failed {
        /// The typed failure of the probe.
        failure: Failure,
    },
}

/// The outcome of folding one terminal receipt into the reducer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DnsLeakFinish {
    /// The receipt did not match the running probe and was ignored.
    Stale,
    /// The receipt was applied.
    Applied,
}

/// The presentation reducer of the DNS leak probe.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DnsLeakActionState {
    /// The current phase of the reducer.
    pub phase: DnsLeakActionPhase,
    next_token: u64,
}

impl DnsLeakActionState {
    /// Starts one DNS leak probe and returns its request token.
    ///
    /// Fails with an `InvalidState` failure when a probe is already running.
    pub fn begin(&mut self) -> Result<u64, Failure> {
        if matches!(&self.phase, DnsLeakActionPhase::Pending { .. }) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS cross-source probe is already pending",
                true,
            ));
        }
        self.next_token = self.next_token.checked_add(1).ok_or_else(|| {
            Failure::new(ErrorCode::InvalidState, "DNS probe token exhausted", false)
        })?;
        self.phase = DnsLeakActionPhase::Pending {
            token: self.next_token,
        };
        Ok(self.next_token)
    }

    /// Starts one DNS leak probe after a retryable failure.
    ///
    /// Fails with an `InvalidState` failure when the recorded failure is not
    /// retryable.
    pub fn begin_retry(&mut self) -> Result<u64, Failure> {
        match &self.phase {
            DnsLeakActionPhase::Failed { failure }
                if failure.retryable || failure.code == ErrorCode::Permission =>
            {
                self.begin()
            }
            DnsLeakActionPhase::Idle
            | DnsLeakActionPhase::Pending { .. }
            | DnsLeakActionPhase::Failed { .. } => Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS probe has no retryable failure",
                false,
            )),
        }
    }

    /// Folds the terminal receipt of one probe into the reducer.
    ///
    /// A receipt whose token does not match the running probe is stale.
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> DnsLeakFinish {
        let DnsLeakActionPhase::Pending { token: expected } = self.phase else {
            return DnsLeakFinish::Stale;
        };
        if expected != token {
            return DnsLeakFinish::Stale;
        }
        self.phase = match result {
            Ok(()) => DnsLeakActionPhase::Idle,
            Err(failure) => DnsLeakActionPhase::Failed { failure },
        };
        DnsLeakFinish::Applied
    }

    /// Whether the recorded failure allows another attempt.
    pub fn can_retry(&self) -> bool {
        matches!(
            &self.phase,
            DnsLeakActionPhase::Failed { failure }
                if failure.retryable || failure.code == ErrorCode::Permission
        )
    }
}

#[cfg(test)]
#[path = "dns_leak_actions_tests.rs"]
mod tests;
