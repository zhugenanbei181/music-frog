//! Deterministic interaction fixtures use the production update pipeline.
//!
//! The dispatch tables live in [`activate`] and [`ready`]: every business of
//! the shared parity catalogue is opened by the real surface through the real
//! shared command, and the capture is published only when the surface state
//! proves that the business really happened.
use infiltrator_contract::parity::FeatureId;

pub(crate) mod activate;
pub(crate) mod ready;

pub(super) fn parse(raw: &str) -> FeatureId {
    FeatureId::ALL
        .iter()
        .copied()
        .find(|feature| feature.spec().id == raw)
        .unwrap_or_else(|| panic!("unknown or unimplemented interaction capture: {raw}"))
}
