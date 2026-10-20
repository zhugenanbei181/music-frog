#![warn(missing_docs)]
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
//!
//! Every reducer in this crate is an explicit state machine: the phase is one
//! `enum` variant, never a combination of flags. A receipt is only applied
//! when it matches the pending request it belongs to, so a late answer of a
//! superseded request can never publish a wrong state.
//!
//! # Example
//!
//! ```
//! use lurker_core_contract::error::{ErrorCode, Failure};
//! use lurker_core_contract::routing_intent::{
//!     RoutingIntent, RoutingIntentSnapshot, RoutingIntentStatus,
//! };
//! use lurker_ui_presentation::routing_intent_actions::RoutingIntentActions;
//!
//! let mut actions = RoutingIntentActions::default();
//!
//! // Only a ready observation may offer the shared routing choices.
//! let failure = Failure::new(ErrorCode::NotReady, "not observed yet", true);
//! actions.observe(
//!     1,
//!     0,
//!     RoutingIntentSnapshot {
//!         current: Some(RoutingIntent::FollowRules),
//!         status: RoutingIntentStatus::Ready,
//!         failure: None,
//!     },
//! );
//!
//! let change = actions
//!     .begin(RoutingIntent::RouteAllThroughProxy)
//!     .expect("the ready observation makes the shared choice selectable");
//! assert_eq!(change.generation, 1);
//!
//! // A receipt that does not match the pending change is ignored.
//! let mut stale = change.clone();
//! stale.token += 1;
//! assert!(actions.finish(stale, Ok(RoutingIntent::RouteAllThroughProxy)).eq(&
//!     lurker_ui_presentation::routing_intent_actions::RoutingIntentFinishOutcome::IgnoredStale
//! ));
//!
//! // The matching receipt publishes the new intent.
//! actions.finish(change, Ok(RoutingIntent::RouteAllThroughProxy));
//! assert_eq!(
//!     actions.render_snapshot().current,
//!     Some(RoutingIntent::RouteAllThroughProxy)
//! );
//! # let _ = failure;
//! ```

pub mod copy;
pub mod core_status_projection;
pub mod dns_leak_actions;
pub mod failure_projection;
pub mod log_follow;
pub mod proxy_group_order_editor;
pub mod routing_intent_actions;
pub mod settings_preference_projection;
