//! Native adapters correlate terminal DNS probe feedback without completing writes on snapshots.
use infiltrator_contract::error::{ErrorCode, Failure};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DnsLeakActionPhase {
    #[default]
    Idle,
    Pending {
        token: u64,
    },
    Failed {
        failure: Failure,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DnsLeakFinish {
    Stale,
    Applied,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DnsLeakActionState {
    pub phase: DnsLeakActionPhase,
    next_token: u64,
}
impl DnsLeakActionState {
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
