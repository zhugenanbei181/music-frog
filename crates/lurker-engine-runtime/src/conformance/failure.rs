//! Typed conformance failures.

use lurker_core_ports::error::PortError;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Stable identity of the conformance check that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConformanceCheck {
    Case,
    EngineBinding,
    CodecScope,
    LifecycleExpectation,
    LifecycleInitial,
    LifecycleStart,
    LifecycleReadiness,
    LifecycleVersion,
    LifecycleRunning,
    LifecycleStop,
    LifecycleTerminal,
    DocumentValid,
    DocumentInvalid,
    DocumentNativeInvalid,
    OutboundScope,
    OutboundInventory,
    OutboundSelection,
    OutboundDetails,
    OutboundProbe,
    OutboundObservation,
    OutboundRestore,
    RoutingObservation,
    RoutingReadback,
    RoutingUnsupported,
    ErrorStaleRevision,
    ErrorStaleGeneration,
    ErrorStaleVersion,
    ErrorStaleSession,
    ErrorForeignEngine,
    RunningObserver,
    SwitchTarget,
    SwitchCoordinator,
    SwitchJournal,
    SwitchOrdering,
    SwitchRecovery,
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
        check: ConformanceCheck,
        expected: String,
        actual: String,
        message: String,
    },
    /// A port returned an error while the check required a successful value.
    AdapterError {
        check: ConformanceCheck,
        expected: String,
        actual: String,
        message: String,
    },
    /// The adapter or coordinator left a state the protocol forbids.
    StateViolation {
        check: ConformanceCheck,
        expected: String,
        actual: String,
        message: String,
    },
}

impl ConformanceFailure {
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

    pub fn check(&self) -> ConformanceCheck {
        match self {
            Self::Mismatch { check, .. }
            | Self::AdapterError { check, .. }
            | Self::StateViolation { check, .. } => *check,
        }
    }

    pub fn expected(&self) -> &str {
        match self {
            Self::Mismatch { expected, .. }
            | Self::AdapterError { expected, .. }
            | Self::StateViolation { expected, .. } => expected,
        }
    }

    pub fn actual(&self) -> &str {
        match self {
            Self::Mismatch { actual, .. }
            | Self::AdapterError { actual, .. }
            | Self::StateViolation { actual, .. } => actual,
        }
    }

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
