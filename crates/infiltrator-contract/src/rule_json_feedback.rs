//! Native editor feedback is a fact, never a cached translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleJsonFeedback {
    HostUnavailable,
    SourceUnavailable,
    Empty,
    InvalidDocument,
    AwaitingReadback,
}
