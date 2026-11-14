//! The pending "locate the current node" request of the proxies page.
//!
//! The reference client (Mihomo Party / Clash Party) scrolls its group
//! list to the node the group has selected. The node a group selected is
//! the owner's fact, so the surface never derives it: it asks the owner
//! published selection and names the node it published.
//!
//! A request waits until the page can show the node. A collapsed group is
//! expanded first, and the scroll happens once the group is on screen.

use crate::state::AppState;
use crate::state::outbound_inventory::OutboundGroupView;

/// The pending locate request of the proxies page.
///
/// The two states are exclusive: the page either has no request, or it has
/// one request naming a group and the node that group selected.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProxyLocator {
    /// No locate request is pending.
    #[default]
    Idle,
    /// A locate request is pending for one group and its selected node.
    Pending { group: String, node: String },
}

impl ProxyLocator {
    /// The group and the node the request names.
    pub fn target(&self) -> Option<(&str, &str)> {
        match self {
            Self::Idle => None,
            Self::Pending { group, node } => Some((group.as_str(), node.as_str())),
        }
    }
}

impl AppState {
    /// The node the group has selected, when the owner published one.
    ///
    /// The selection is the owner's fact: the surface reads it and never
    /// guesses which node of a group is the current one.
    pub(crate) fn outbound_group_current_node(&self, group: &str) -> Option<&str> {
        self.runtime
            .proxy_groups
            .iter()
            .find(|candidate| candidate.name == group)
            .and_then(OutboundGroupView::current)
    }

    /// Whether the pending locate can be served right now.
    ///
    /// A locate is served when its group is expanded and the node it names
    /// is one of the members the page shows. A node the group's own filter
    /// hides is not on the page, so there is nothing to scroll to. The
    /// reference opens the group to expose the node; the Iced page keeps
    /// the group's filter honest instead of dropping it behind the
    /// reader's back.
    pub(crate) fn outbound_group_locate_is_ready(&self) -> bool {
        let Some((group, node)) = self.runtime.proxy_locator.target() else {
            return false;
        };
        let Some(group_view) = self
            .runtime
            .proxy_groups
            .iter()
            .find(|candidate| candidate.name == group)
        else {
            return false;
        };
        if !group_view.expanded {
            return false;
        }
        self.runtime
            .filtered_groups
            .iter()
            .any(|(name, members)| name == group && members.iter().any(|member| member == node))
    }
}

#[cfg(test)]
#[path = "../../tests/gui/proxy_locator_tests.rs"]
mod tests;
