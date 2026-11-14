//! Bringing a proxy group's selected node into view.
//!
//! The node is the owner's published selection; this module only measures the
//! live layout and waits for the typed group-expansion readback before it
//! scrolls. Kept apart from the inventory/action core so each file stays under
//! the line budget.

use crate::state::AppState;
use crate::state::proxy_locator::ProxyLocator;
use crate::types::message::Message;
use crate::types::message::proxy_page_intent::ProxyPageIntent;
use crate::view::proxies::locate;
use iced::Task;
use lurker_core_contract::outbound::preferences::{
    OutboundGroupExpansion, OutboundUiPreferenceIntent,
};

impl AppState {
    /// One typed family of proxies-page intents, dispatched here rather than
    /// as three loose variants on the application message bus.
    pub(crate) fn update_proxy_page_intent(&mut self, intent: ProxyPageIntent) -> Task<Message> {
        match intent {
            ProxyPageIntent::ToggleGroupFilter(group) => {
                self.toggle_outbound_group_filter(&group);
                Task::none()
            }
            ProxyPageIntent::FilterGroup { group, text } => {
                self.filter_outbound_group(&group, text);
                Task::none()
            }
            ProxyPageIntent::LocateGroupNode(group) => self.locate_outbound_group_node(&group),
        }
    }
    /// Bring the node one proxy group selected into view.
    ///
    /// The node is the owner's published selection, so the surface never
    /// derives it: the command names the group and the owner published
    /// selection decides which node it is. A closed group is expanded
    /// first — the node is not on the page at all while its group is
    /// closed — and the scroll happens once the group is on screen.
    pub(crate) fn locate_outbound_group_node(&mut self, group: &str) -> Task<Message> {
        if !self.is_product_command_ingress() && !self.shell.demo {
            return Task::none();
        }
        let Some(node) = self.outbound_group_current_node(group).map(str::to_owned) else {
            return Task::none();
        };
        self.runtime.proxy_locator = ProxyLocator::Pending {
            group: group.to_owned(),
            node,
        };
        if !self.outbound_group_locate_is_ready() {
            let Some(group_id) = self
                .runtime
                .proxy_groups
                .iter()
                .find(|candidate| candidate.name == group)
                .and_then(|candidate| candidate.id.clone())
            else {
                self.runtime.proxy_locator = ProxyLocator::Idle;
                return Task::none();
            };
            let intent = OutboundUiPreferenceIntent::SetGroupExpansion {
                group_id,
                expansion: OutboundGroupExpansion::Expanded,
            };
            if self.is_product_command_ingress() {
                return self.submit_current_outbound_ui_preference(intent);
            }
            self.apply_demo_outbound_ui_preference(intent);
        }
        self.take_outbound_group_locate_task()
    }

    /// The scroll that brings the pending locate target into view.
    ///
    /// The request is consumed when it is served. A request whose node is
    /// not on the page stays pending: the page scrolls to a node it shows,
    /// never to a node it hides.
    pub(crate) fn take_outbound_group_locate_task(&mut self) -> Task<Message> {
        if !self.outbound_group_locate_is_ready() {
            return Task::none();
        }
        let Some((group, node)) = self
            .runtime
            .proxy_locator
            .target()
            .map(|(group, node)| (group.to_owned(), node.to_owned()))
        else {
            return Task::none();
        };
        self.runtime.proxy_locator = ProxyLocator::Idle;
        locate::locate_task(self, &group, &node).unwrap_or_else(Task::none)
    }
}
