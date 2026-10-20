//! The one typed phase machine every page model composes.
//!
//! A page submits owner commands and renders what the owner answered. The
//! phase of one submitted operation is a single typed value: a page never
//! keeps a "saving" flag next to a "failed" string, because those two
//! together describe a stage table that no type checker can verify.
//!
//! The machine is deliberately narrow. It knows that an operation is idle,
//! running, confirmed or refused; it does not know *what* the operation was.
//! The page model owns that vocabulary and composes this phase.
//!
//! The token is what makes a late answer harmless: a surface that submits an
//! operation twice, or receives the answer of a superseded submission, ends
//! up in exactly one phase instead of two contradicting flags.

use lurker_core_contract::error::Failure;

/// The phase of one owner operation a page submitted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OperationState {
    /// No operation was submitted, or the last one was acknowledged.
    #[default]
    Idle,
    /// The operation was submitted; the owner has not answered yet.
    Running {
        /// The token that identifies this submission.
        token: u64,
    },
    /// The owner confirmed the operation.
    Succeeded {
        /// The token of the confirmed submission.
        token: u64,
    },
    /// The owner refused or failed the operation.
    Failed {
        /// The token of the refused submission.
        token: u64,
        /// Why the owner refused.
        failure: Failure,
    },
}

impl OperationState {
    /// Whether the operation is still in flight.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    /// The failure that ended the operation, when it failed.
    pub fn failure(&self) -> Option<&Failure> {
        match self {
            Self::Failed { failure, .. } => Some(failure),
            Self::Idle | Self::Running { .. } | Self::Succeeded { .. } => None,
        }
    }

    /// The token of the submission this state describes.
    pub fn token(&self) -> Option<u64> {
        match self {
            Self::Idle => None,
            Self::Running { token } | Self::Succeeded { token } | Self::Failed { token, .. } => {
                Some(*token)
            }
        }
    }
}

/// The stateful phase machine of one owner operation.
///
/// The machine owns the submission counter, so a surface never has to keep a
/// second "which request am I waiting for" number next to the phase.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OperationPhase {
    state: OperationState,
    next_token: u64,
}

impl OperationPhase {
    /// Submit the operation and return the token that identifies it.
    ///
    /// A second submission supersedes the first: the newest submission is the
    /// only one whose answer the page will accept.
    pub fn begin(&mut self) -> u64 {
        self.next_token = self.next_token.wrapping_add(1);
        let token = self.next_token;
        self.state = OperationState::Running { token };
        token
    }

    /// Confirm the submission identified by `token`.
    ///
    /// Returns `false` for an answer that belongs to a superseded submission;
    /// the phase of the newest submission is kept.
    pub fn succeed(&mut self, token: u64) -> bool {
        if !self.is_current(token) {
            return false;
        }
        self.state = OperationState::Succeeded { token };
        true
    }

    /// Refuse the submission identified by `token`.
    ///
    /// Returns `false` for an answer that belongs to a superseded submission.
    pub fn fail(&mut self, token: u64, failure: Failure) -> bool {
        if !self.is_current(token) {
            return false;
        }
        self.state = OperationState::Failed { token, failure };
        true
    }

    /// Return to the idle phase.
    pub fn reset(&mut self) {
        self.state = OperationState::Idle;
    }

    /// The current phase.
    pub fn state(&self) -> &OperationState {
        &self.state
    }

    /// Whether the operation is still in flight.
    pub fn is_running(&self) -> bool {
        self.state.is_running()
    }

    /// The failure that ended the operation, when it failed.
    pub fn failure(&self) -> Option<&Failure> {
        self.state.failure()
    }

    /// The token of the newest submission, when one was made.
    pub fn token(&self) -> Option<u64> {
        self.state.token()
    }

    fn is_current(&self, token: u64) -> bool {
        self.state.token() == Some(token)
    }
}

#[cfg(test)]
#[path = "operation_phase_tests.rs"]
mod tests;
