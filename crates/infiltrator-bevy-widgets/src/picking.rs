//! The picking policy of the product's UI layers.
//!
//! Bevy's UI picking backend stops at the first layer that claims the
//! pointer: a node **without** a [`Pickable`] component blocks every hit
//! below it, and a node whose `should_block_lower` is set owns the pointer
//! for everything underneath. A decorative layer — an overlay container
//! that only positions its children — therefore swallows every pointer
//! event under it unless it states that it does not take part in picking.
//!
//! This module is the single owner of the two policies the product uses. A
//! layer names one of them instead of repeating a literal, so the question
//! "does this layer take the pointer?" has exactly one answer per layer.

use bevy::picking::Pickable;

/// A layer that never takes part in pointer picking.
///
/// The layer's children keep their own policy: a button inside a
/// non-interactive overlay is still a button, and the controls under the
/// layer still receive the pointer.
pub const NON_INTERACTIVE: Pickable = Pickable {
    should_block_lower: false,
    is_hoverable: false,
};

/// A layer that observes the pointer without taking it from the controls
/// underneath it.
///
/// The layer is hovered like any other node, but the pointer also reaches
/// everything below it.
pub const HOVER_PASSTHROUGH: Pickable = Pickable {
    should_block_lower: false,
    is_hoverable: true,
};
