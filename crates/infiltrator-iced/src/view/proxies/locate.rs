//! Locating the current node of a proxy group.
//!
//! The reference client (Mihomo Party / Clash Party) scrolls its group list
//! to the node the group has selected. The Iced page is one long scrollable
//! instead of a virtualized list, so a locate is a scroll of that page: the
//! live layout is measured for the boxes that carry the node, and the page
//! scrolls so the node sits at the top of the viewport.
//!
//! Every offset is read from the layout that is on screen, so the scroll
//! never guesses a position. Iced reports every nested container in the
//! same content coordinate space, so the node's own box already names its
//! offset inside the scroll content; the group card, body, node area and
//! grid row are only the chain that proves the box belongs to this group.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::proxies::node_cards::proxy_node_grid_columns;
use crate::view::proxies::node_region_id;
use iced::advanced::widget::operation::{Outcome, Scrollable};
use iced::advanced::widget::{Id, Operation, operate};
use iced::widget::scrollable::AbsoluteOffset;
use iced::{Rectangle, Task, Vector};
use lurker_core_contract::outbound::preferences::OutboundLayout;
use std::iter::once;

/// The scrollable the proxies page body lives in.
pub const PROXIES_PAGE_SCROLL_ID: &str = "proxies-page-scroll";

/// The card of one proxy group.
pub fn group_card_id(group: &str) -> Id {
    Id::from(format!("proxy-group-card:{group}"))
}

/// The body of one proxy group: its header and its node area.
pub fn group_body_id(group: &str) -> Id {
    Id::from(format!("proxy-group-body:{group}"))
}

/// The node area of one proxy group: its grid or its compact list.
pub fn group_grid_id(group: &str) -> Id {
    Id::from(format!("proxy-group-grid:{group}"))
}

/// One row of a group's node area.
pub fn group_row_id(group: &str, row: usize) -> Id {
    Id::from(format!("proxy-group-row:{group}:{row}"))
}

/// The scroll task that brings one node of one group into view.
///
/// The row the node sits in is the page's own layout: the grid puts
/// `columns` nodes in a row, and the compact list puts one node in a row.
/// A node the page does not show has no row, so there is nothing to
/// scroll to and the task is not created.
pub fn locate_task(state: &AppState, group: &str, node: &str) -> Option<Task<Message>> {
    let index = state
        .runtime
        .filtered_groups
        .iter()
        .find(|(name, _)| name == group)
        .and_then(|(_, members)| members.iter().position(|member| member == node))?;
    let row = if state.runtime.proxies_model.layout() == OutboundLayout::CompactList {
        index
    } else {
        let (_, columns) = proxy_node_grid_columns(state);
        index / columns.max(1)
    };
    // The operation itself completes the scroll; the task needs no
    // message back. Erasing the unit output keeps the widget action in
    // the stream without inventing a message that never happened.
    let operation: Box<dyn Operation<Message>> = Box::new(LocateNode {
        scroll_id: Id::from(PROXIES_PAGE_SCROLL_ID.to_owned()),
        card_id: group_card_id(group),
        body_id: group_body_id(group),
        grid_id: group_grid_id(group),
        row_id: group_row_id(group, row),
        node_id: node_region_id(group, node),
        card: None,
        body: None,
        grid: None,
        row: None,
        node: None,
        scroll: None,
        viewport_height: None,
        content_height: None,
    });
    Some(operate(operation))
}

/// The measurement that locates one node inside the proxies page.
///
/// The scroll content is one column of group cards, so the node's offset
/// inside it is the node's own measured y. The boxes that carry it are
/// checked for containment: a missing box or a broken chain means the
/// node is not on the page, so the offset is unknown rather than guessed.
struct LocateNode {
    scroll_id: Id,
    card_id: Id,
    body_id: Id,
    grid_id: Id,
    row_id: Id,
    node_id: Id,
    card: Option<Rectangle>,
    body: Option<Rectangle>,
    grid: Option<Rectangle>,
    row: Option<Rectangle>,
    node: Option<Rectangle>,
    scroll: Option<Rectangle>,
    viewport_height: Option<f32>,
    content_height: Option<f32>,
}

impl LocateNode {
    /// The offset of the node inside the scroll content.
    ///
    /// Every link of the chain must be measured and must carry the node:
    /// a missing box or a broken containment chain means the node is not
    /// on the page, so the offset is unknown rather than guessed.
    fn content_offset(&self) -> Option<f32> {
        let node = self.node?;
        let chain = [self.card?, self.body?, self.grid?, self.row?];
        let mut outer = chain[0];
        for inner in chain.iter().skip(1).chain(once(&node)) {
            if !outer.contains(inner.position()) {
                return None;
            }
            outer = *inner;
        }
        let scroll = self.scroll?;
        Some(node.y - scroll.y)
    }
}

impl Operation<Message> for LocateNode {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Message>)) {
        operate(self);
    }

    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        let Some(id) = id else {
            return;
        };
        if id == &self.card_id {
            self.card = Some(bounds);
        } else if id == &self.body_id {
            self.body = Some(bounds);
        } else if id == &self.grid_id {
            self.grid = Some(bounds);
        } else if id == &self.row_id {
            self.row = Some(bounds);
        } else if id == &self.node_id {
            self.node = Some(bounds);
        }
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content: Rectangle,
        _translation: Vector,
        _state: &mut dyn Scrollable,
    ) {
        if id == Some(&self.scroll_id) {
            self.scroll = Some(bounds);
            self.viewport_height = Some(bounds.height);
            self.content_height = Some(content.height);
        }
    }

    fn finish(&self) -> Outcome<Message> {
        let (Some(offset), Some(viewport), Some(content)) = (
            self.content_offset(),
            self.viewport_height,
            self.content_height,
        ) else {
            return Outcome::None;
        };
        let max_offset = (content - viewport).max(0.0);
        let target = offset.clamp(0.0, max_offset);
        Outcome::Chain(Box::new(ScrollTo {
            id: self.scroll_id.clone(),
            offset: target,
        }))
    }
}

/// The scroll that brings the measured node to the top of the viewport.
struct ScrollTo {
    id: Id,
    offset: f32,
}

impl Operation<Message> for ScrollTo {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Message>)) {
        operate(self);
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        _bounds: Rectangle,
        _content: Rectangle,
        _translation: Vector,
        state: &mut dyn Scrollable,
    ) {
        if id == Some(&self.id) {
            state.scroll_to(AbsoluteOffset {
                x: None,
                y: Some(self.offset),
            });
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/gui/proxy_locate_tests.rs"]
mod tests;
