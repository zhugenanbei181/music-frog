//! The globe's degradation policy: how much of the optional sphere the
//! device may afford.
//!
//! The three pressure signals — battery, thermal state and the user's motion
//! preference — are independent facts. The decision they produce is one typed
//! value: the rendering mode. The policy never invents content: it only
//! decides how much of the *known* content is drawn, and it may only degrade
//! a requested mode, never raise it.

use crate::globe::GlobeMode;

/// The battery state the degradation policy reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GlobeBattery {
    /// The battery is healthy.
    #[default]
    Healthy,
    /// The battery is low: the enhancement must spend as little as possible.
    Low,
}

/// The thermal state the degradation policy reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GlobeThermal {
    /// The device runs at its normal temperature.
    #[default]
    Normal,
    /// The device is throttling: the sphere must not run.
    Throttling,
}

/// The motion preference the degradation policy reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GlobeMotion {
    /// The user accepts the full motion.
    #[default]
    Full,
    /// The user asked for reduced motion: nothing may animate.
    Reduced,
}

/// The device pressure the globe's degradation policy reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlobeDevicePressure {
    /// The battery state.
    pub battery: GlobeBattery,
    /// The thermal state.
    pub thermal: GlobeThermal,
    /// The motion preference.
    pub motion: GlobeMotion,
}

impl GlobeDevicePressure {
    /// The most degraded mode this pressure allows.
    ///
    /// A low battery is the strongest signal (the minimal Eco view), a
    /// throttling device and a reduced-motion preference both drop the
    /// sphere for the static 2D fallback.
    pub fn allowed_mode(self) -> GlobeMode {
        if self.battery == GlobeBattery::Low {
            return GlobeMode::Eco;
        }
        if self.thermal == GlobeThermal::Throttling || self.motion == GlobeMotion::Reduced {
            return GlobeMode::Flat2d;
        }
        GlobeMode::Globe
    }

    /// The mode actually rendered: the requested mode capped by the pressure.
    ///
    /// The policy may only degrade a request, never raise it: an explicit Eco
    /// request always wins, and a requested sphere is dropped as soon as the
    /// device cannot afford it.
    pub fn effective_mode(self, requested: GlobeMode) -> GlobeMode {
        let allowed = self.allowed_mode();
        if degradation_rank(requested) <= degradation_rank(allowed) {
            requested
        } else {
            allowed
        }
    }
}

/// The degradation ladder: Eco is the most degraded, Globe the richest.
fn degradation_rank(mode: GlobeMode) -> u8 {
    match mode {
        GlobeMode::Eco => 0,
        GlobeMode::Flat2d => 1,
        GlobeMode::Globe => 2,
    }
}
