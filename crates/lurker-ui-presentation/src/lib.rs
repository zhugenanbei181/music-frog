//! Engine-, platform- and UI-neutral presentation kernel.
//!
//! This crate owns the parts of the cross-surface presentation layer that are
//! already free of product contract types: typed reducer phases, viewport
//! policy and the localization key port. It depends only on
//! `lurker-core-contract`, `std` and no UI toolkit, so an Iced, Bevy, Slint or
//! Compose surface (and any future non-Rust consumer) can share the same
//! transitions without pulling in a renderer or a product crate.
//!
//! Localization **keys** live here; localized **copy** stays with each product
//! catalog, which implements [`copy::TextCatalog`].

pub mod copy;
pub mod core_status_projection;
pub mod dns_leak_actions;
pub mod failure_projection;
pub mod log_follow;
pub mod proxy_group_order_editor;
pub mod routing_intent_actions;
pub mod settings_preference_projection;
