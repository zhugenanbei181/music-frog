//! Framework-neutral localization resources and presentation utilities shared by MusicFrog products.
//!
//! The crate owns the *resources and algorithms* the product shares: the
//! localization tables, the visual assets, the effect mathematics and the
//! character metrics. It owns no product fact: the semantic palette and the
//! motion parameters live in `infiltrator-contract`, and the page models live
//! in `infiltrator-presentation`.

pub mod assets;
pub mod autostart;
pub mod byte_units;
pub mod country_flags;
pub mod dns_observation;
pub mod effect_assets;
pub mod effect_math;
pub mod error_codes;
pub mod fuzzy_search;
pub mod i18n_interpolator;
pub mod latency_chart;
pub mod locales;
pub mod metrics_summary;
pub mod subscription_quota;
