//! Bevy-side mounting of the shared accessibility grammar (DUAL-15-10).
//!
//! Bevy publishes AccessKit through the winit bridge, so the shell mounts real
//! `AccessibilityNode`s instead of describing them. The role and the label of
//! every node come from `infiltrator_contract::a11y` — the same rows the Iced
//! surface resolves into localized labels — so the two surfaces cannot drift
//! into two vocabularies. Switches additionally carry their live on/off state.

use bevy::a11y::AccessibilityNode;
use infiltrator_contract::a11y::{A11yRole, ShellA11yNode};

/// Map a shared grammar role onto the AccessKit role Bevy publishes.
pub const fn accesskit_role(role: A11yRole) -> accesskit::Role {
    match role {
        A11yRole::Window => accesskit::Role::Window,
        A11yRole::Header => accesskit::Role::Header,
        A11yRole::Region => accesskit::Role::Region,
        A11yRole::Navigation => accesskit::Role::Navigation,
        A11yRole::Button => accesskit::Role::Button,
        A11yRole::Switch => accesskit::Role::Switch,
        A11yRole::Status => accesskit::Role::Status,
        A11yRole::Dialog => accesskit::Role::Dialog,
        A11yRole::LiveRegion => accesskit::Role::Log,
        A11yRole::Text => accesskit::Role::Label,
    }
}

/// The mounted semantic node for one grammar row (role + bare-Chinese label).
pub fn semantic_node(node: ShellA11yNode) -> AccessibilityNode {
    let mut a11y = accesskit::Node::new(accesskit_role(node.role()));
    a11y.set_label(node.label_zh());
    AccessibilityNode(a11y)
}

/// A switch row with its live on/off state; the label stays the shared one.
pub fn switch_node(node: ShellA11yNode, enabled: bool) -> AccessibilityNode {
    let mut a11y = accesskit::Node::new(accesskit_role(A11yRole::Switch));
    a11y.set_label(node.label_zh());
    a11y.set_toggled(accesskit::Toggled::from(enabled));
    AccessibilityNode(a11y)
}

/// A read-only status/text row: the label plus the live value the shell shows.
pub fn value_node(node: ShellA11yNode, value: &str) -> AccessibilityNode {
    let mut a11y = accesskit::Node::new(accesskit_role(node.role()));
    a11y.set_label(format!("{}: {value}", node.label_zh()));
    AccessibilityNode(a11y)
}
