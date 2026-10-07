//! Framework-neutral status severity, shared by DNS observation projections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DnsObservationTone {
    Neutral,
    Success,
    Warning,
    Danger,
}

#[cfg(test)]
#[path = "dns_observation_projection_test.rs"]
mod tests;
