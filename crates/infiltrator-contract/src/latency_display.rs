//! Measurement provenance and semantic bands shared by native renderers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LatencyReading {
    NotObserved,
    Measured(u32),
    ControllerHistory(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LatencyBand {
    NotObserved,
    UnconfirmedZero,
    Fast,
    Medium,
    Slow,
}
