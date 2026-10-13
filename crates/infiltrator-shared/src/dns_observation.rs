//! Shared visual tone for cross-surface DNS observation readouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DnsObservationTone {
    Neutral,
    Success,
    Warning,
    Danger,
}
