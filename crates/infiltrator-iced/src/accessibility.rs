//! Iced-side accessibility vocabulary (DUAL-15-10).
//!
//! Iced 0.14 has no AccessKit integration: the toolkit cannot publish roles
//! or screen-reader labels to the OS, and this crate must not pretend it can
//! (the group guard forbids any `accesskit` reference on the Iced side). What
//! Iced *can* do honestly is speak the shared grammar
//! ([`infiltrator_contract::a11y`]) out loud:
//!
//! * every shell node resolves its label from the shared key through the same
//!   localizer the rest of the UI uses ([`AppState::a11y_label`]);
//! * controls whose meaning is not already visible text carry that label as a
//!   real hover tooltip ([`labelled`]) — the visible affordance this toolkit
//!   offers in place of a semantic tree.
//!
//! The roles themselves remain a grammar the host cannot publish; Bevy takes
//! the same rows and mounts them as real `AccessibilityNode`s.

use crate::state::AppState;
use iced::Element;
use iced::widget::{container, text, tooltip};
use infiltrator_contract::a11y::{A11yRole, ShellA11yNode};
use infiltrator_shared::locales::{Lang, Localizer};

impl AppState {
    /// The localized label of a shared shell semantic node.
    pub fn a11y_label(&self, node: ShellA11yNode) -> String {
        Lang(&self.shell.lang).tr(node.label_key()).to_string()
    }

    /// The role the shared grammar assigns to a node. Iced cannot publish it,
    /// but keeping the lookup here means the surface reports the same role the
    /// Bevy tree mounts instead of inventing a second vocabulary.
    pub fn a11y_role(&self, node: ShellA11yNode) -> A11yRole {
        node.role()
    }
}

/// Wrap a control in a tooltip carrying the shared label of `node`.
///
/// This is the honest Iced mapping of a semantic label: a visible hover label
/// for controls whose meaning is otherwise a colour, a glyph or a number
/// (status dot, live rate readout, the Mini HUD quick switches).
pub fn labelled<'a, Message: 'a>(
    node: ShellA11yNode,
    lang: &str,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let label = Lang(lang).tr(node.label_key()).to_string();
    tooltip(
        content,
        container(text(label).size(12))
            .padding(6)
            .style(container::rounded_box),
        tooltip::Position::Top,
    )
    .gap(4.0)
    .into()
}
