//! Bevy-side mounting of the shared accessibility grammar (DUAL-15-10).
//!
//! Bevy publishes AccessKit through the winit bridge, so the shell mounts real
//! `AccessibilityNode`s instead of describing them. The role and the label of
//! every node come from `infiltrator_contract::a11y` — the same rows the Iced
//! surface resolves into localized labels — so the two surfaces cannot drift
//! into two vocabularies. Switches additionally carry their live on/off state.

use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::message::{Message, MessageReader, MessageWriter};
use bevy::ecs::system::Res;
use infiltrator_bevy_widgets::text_input::native::{
    ScreenReaderAction, ScreenReaderGate, ScreenReaderOutcome,
};
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

pub fn window_semantic_node(title: &str) -> AccessibilityNode {
    let mut a11y = accesskit::Node::new(accesskit::Role::Window);
    a11y.set_label(title);
    AccessibilityNode(a11y)
}

pub fn header_semantic_node(title: &str) -> AccessibilityNode {
    let mut a11y = accesskit::Node::new(accesskit::Role::Header);
    a11y.set_label(title);
    AccessibilityNode(a11y)
}

pub fn toggle_semantic_node(label: &str) -> AccessibilityNode {
    let mut a11y = accesskit::Node::new(accesskit::Role::Switch);
    a11y.set_label(label);
    AccessibilityNode(a11y)
}

pub fn button_semantic_node(label: &str) -> AccessibilityNode {
    let mut a11y = accesskit::Node::new(accesskit::Role::Button);
    a11y.set_label(label);
    AccessibilityNode(a11y)
}

pub fn nav_semantic_node(label: &str, disabled: bool) -> AccessibilityNode {
    let mut a11y = accesskit::Node::new(accesskit::Role::Button);
    a11y.set_label(label);
    if disabled {
        a11y.set_disabled();
    }
    AccessibilityNode(a11y)
}

pub fn region_semantic_node(label: &str) -> AccessibilityNode {
    let mut a11y = accesskit::Node::new(accesskit::Role::Region);
    a11y.set_label(label);
    AccessibilityNode(a11y)
}

/// A semantic action the shell routes through the installed screen-reader gate.
///
/// The widget layer owns the bridge trait; the shell only carries the request
/// across the seam, so no ECS type ever crosses into a platform host.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct SemanticActionRequest(pub ScreenReaderAction);

/// The typed outcome for each routed request, in arrival order.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticActionOutcome(pub ScreenReaderOutcome);

/// Route every requested semantic action through the host gate. With no bridge
/// installed this is a no-op that reports the typed `Unsupported` outcome —
/// never a silent success.
pub fn route_semantic_actions(
    mut requests: MessageReader<SemanticActionRequest>,
    gate: Res<ScreenReaderGate>,
    mut outcomes: MessageWriter<SemanticActionOutcome>,
) {
    for request in requests.read() {
        outcomes.write(SemanticActionOutcome(gate.dispatch(request.0.clone())));
    }
}

/// Install the screen-reader gate seam and its routing system.
pub struct ShellA11yPlugin;

impl Plugin for ShellA11yPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenReaderGate>();
        app.add_message::<SemanticActionRequest>();
        app.add_message::<SemanticActionOutcome>();
        app.add_systems(Update, route_semantic_actions);
    }
}
