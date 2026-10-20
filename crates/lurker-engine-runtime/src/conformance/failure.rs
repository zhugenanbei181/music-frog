//! Typed conformance failures.

use lurker_core_ports::error::PortError;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Stable identity of the conformance check that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConformanceCheck {
    /// The conformance case itself could not be prepared.
    Case,
    /// The backend did not report the engine it was requested for.
    EngineBinding,
    /// The document codec did not report the expected scope.
    CodecScope,
    /// The observed lifecycle did not match the expectation of the case.
    LifecycleExpectation,
    /// The adapter did not start in the expected initial lifecycle state.
    LifecycleInitial,
    /// Starting the core failed.
    LifecycleStart,
    /// The readiness probe failed.
    LifecycleReadiness,
    /// The reported core version was missing or wrong.
    LifecycleVersion,
    /// The core did not reach the running lifecycle state.
    LifecycleRunning,
    /// Stopping the core failed.
    LifecycleStop,
    /// The terminal readback after the stop was not exact.
    LifecycleTerminal,
    /// A valid document was not accepted.
    DocumentValid,
    /// An invalid document was not rejected.
    DocumentInvalid,
    /// A document that is invalid for the native format was not rejected.
    DocumentNativeInvalid,
    /// An outbound operation was refused for a foreign scope.
    OutboundScope,
    /// The outbound inventory did not match the expectation.
    OutboundInventory,
    /// The outbound selection did not take effect.
    OutboundSelection,
    /// The outbound details did not match the expectation.
    OutboundDetails,
    /// The outbound probe did not produce the expected result.
    OutboundProbe,
    /// The combined outbound observation cycle was inconsistent.
    OutboundObservation,
    /// The outbound state could not be restored after a check.
    OutboundRestore,
    /// The routing intent could not be observed.
    RoutingObservation,
    /// The routing intent readback did not match the applied value.
    RoutingReadback,
    /// An unsupported routing intent was not rejected in the typed way.
    RoutingUnsupported,
    /// A stale revision was not rejected in the typed way.
    ErrorStaleRevision,
    /// A stale generation was not rejected in the typed way.
    ErrorStaleGeneration,
    /// A stale version was not rejected in the typed way.
    ErrorStaleVersion,
    /// A stale session token was not rejected in the typed way.
    ErrorStaleSession,
    /// A foreign engine scope was not rejected in the typed way.
    ErrorForeignEngine,
    /// The running-state observer did not report the expected lifecycle.
    RunningObserver,
    /// The owner-switch target was rejected or mismatched.
    SwitchTarget,
    /// The owner-switch coordinator did not complete as expected.
    SwitchCoordinator,
    /// The owner-switch journal did not expose the expected stage.
    SwitchJournal,
    /// The owner-switch stages were committed out of order.
    SwitchOrdering,
    /// The owner-switch recovery path did not behave as expected.
    SwitchRecovery,
    /// The owner-switch cleanup did not restore the previous state.
    SwitchCleanup,
}

impl Display for ConformanceCheck {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

/// One failed conformance check with the exact observed evidence.
///
/// The failure kind is a closed enum and every variant carries the typed check
/// identity, the expectation, the observation and a diagnostic message. The
/// kit never reports a bare `Err(String)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConformanceFailure {
    /// The adapter produced a different value than the case requires.
    Mismatch {
        /// The check that failed.
        check: ConformanceCheck,
        /// The value the case required.
        expected: String,
        /// The value the adapter produced.
        actual: String,
        /// Diagnostic detail of the failure.
        message: String,
    },
    /// A port returned an error while the check required a successful value.
    AdapterError {
        /// The check that failed.
        check: ConformanceCheck,
        /// The value the case required.
        expected: String,
        /// The value the adapter produced.
        actual: String,
        /// Diagnostic detail of the failure.
        message: String,
    },
    /// The adapter or coordinator left a state the protocol forbids.
    StateViolation {
        /// The check that failed.
        check: ConformanceCheck,
        /// The state the case required.
        expected: String,
        /// The state the adapter produced.
        actual: String,
        /// Diagnostic detail of the failure.
        message: String,
    },
}

impl ConformanceFailure {
    /// Builds a `Mismatch` failure.
    pub fn mismatch(
        check: ConformanceCheck,
        expected: impl Into<String>,
        actual: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Mismatch {
            check,
            expected: expected.into(),
            actual: actual.into(),
            message: message.into(),
        }
    }

    /// Builds an `AdapterError` failure.
    pub fn adapter_error(
        check: ConformanceCheck,
        expected: impl Into<String>,
        actual: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::AdapterError {
            check,
            expected: expected.into(),
            actual: actual.into(),
            message: message.into(),
        }
    }

    /// Builds a `StateViolation` failure.
    pub fn state(
        check: ConformanceCheck,
        expected: impl Into<String>,
        actual: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::StateViolation {
            check,
            expected: expected.into(),
            actual: actual.into(),
            message: message.into(),
        }
    }

    /// Builds an `AdapterError` failure from a port error.
    pub fn port_error(
        check: ConformanceCheck,
        expected: impl Into<String>,
        message: impl Into<String>,
        error: &PortError,
    ) -> Self {
        Self::AdapterError {
            check,
            expected: expected.into(),
            actual: error.to_string(),
            message: message.into(),
        }
    }

    /// Returns the check that failed.
    pub fn check(&self) -> ConformanceCheck {
        match self {
            Self::Mismatch { check, .. }
            | Self::AdapterError { check, .. }
            | Self::StateViolation { check, .. } => *check,
        }
    }

    /// Returns the value the case required.
    pub fn expected(&self) -> &str {
        match self {
            Self::Mismatch { expected, .. }
            | Self::AdapterError { expected, .. }
            | Self::StateViolation { expected, .. } => expected,
        }
    }

    /// Returns the value the adapter produced.
    pub fn actual(&self) -> &str {
        match self {
            Self::Mismatch { actual, .. }
            | Self::AdapterError { actual, .. }
            | Self::StateViolation { actual, .. } => actual,
        }
    }

    /// Returns the diagnostic detail of the failure.
    pub fn message(&self) -> &str {
        match self {
            Self::Mismatch { message, .. }
            | Self::AdapterError { message, .. }
            | Self::StateViolation { message, .. } => message,
        }
    }

    /// Attach secondary evidence (for example a cleanup failure) without
    /// losing the primary check identity.
    pub fn with_message_suffix(self, suffix: &str) -> Self {
        match self {
            Self::Mismatch {
                check,
                expected,
                actual,
                message,
            } => Self::Mismatch {
                check,
                expected,
                actual,
                message: format!("{message}; {suffix}"),
            },
            Self::AdapterError {
                check,
                expected,
                actual,
                message,
            } => Self::AdapterError {
                check,
                expected,
                actual,
                message: format!("{message}; {suffix}"),
            },
            Self::StateViolation {
                check,
                expected,
                actual,
                message,
            } => Self::StateViolation {
                check,
                expected,
                actual,
                message: format!("{message}; {suffix}"),
            },
        }
    }
}

impl Display for ConformanceFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} conformance check failed: expected {}, observed {} ({})",
            self.check(),
            self.expected(),
            self.actual(),
            self.message()
        )
    }
}

impl Error for ConformanceFailure {}
