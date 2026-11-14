//! test-intent: behavior
//! The locate operation measures the live layout and scrolls the proxies
//! page to the node the owner published as selected.

use super::*;
use iced::Rectangle;
use iced::widget::scrollable::{AbsoluteOffset, RelativeOffset};
use std::cell::RefCell;
use std::rc::Rc;

struct RecordedScrollable {
    offset: Rc<RefCell<Option<AbsoluteOffset<Option<f32>>>>>,
}

impl Scrollable for RecordedScrollable {
    fn snap_to(&mut self, _offset: RelativeOffset<Option<f32>>) {}

    fn scroll_to(&mut self, offset: AbsoluteOffset<Option<f32>>) {
        *self.offset.borrow_mut() = Some(offset);
    }

    fn scroll_by(
        &mut self,
        _offset: AbsoluteOffset,
        _bounds: Rectangle,
        _content_bounds: Rectangle,
    ) {
    }
}

fn probe() -> LocateNode {
    LocateNode {
        scroll_id: Id::from(PROXIES_PAGE_SCROLL_ID.to_owned()),
        card_id: group_card_id("GLOBAL"),
        body_id: group_body_id("GLOBAL"),
        grid_id: group_grid_id("GLOBAL"),
        row_id: group_row_id("GLOBAL", 1),
        node_id: node_region_id("GLOBAL", "proxy-a"),
        card: None,
        body: None,
        grid: None,
        row: None,
        node: None,
        scroll: None,
        viewport_height: None,
        content_height: None,
    }
}

fn box_at(y: f32, height: f32) -> Rectangle {
    Rectangle {
        x: 0.0,
        y,
        width: 100.0,
        height,
    }
}

#[test]
fn every_box_must_be_measured_before_an_offset_is_known() {
    let mut operation = probe();
    assert_eq!(operation.content_offset(), None);
    operation.container(Some(&group_card_id("GLOBAL")), box_at(100.0, 200.0));
    operation.container(Some(&group_body_id("GLOBAL")), box_at(110.0, 180.0));
    operation.container(Some(&group_grid_id("GLOBAL")), box_at(150.0, 100.0));
    operation.container(Some(&group_row_id("GLOBAL", 1)), box_at(160.0, 60.0));
    assert_eq!(operation.content_offset(), None);
    operation.container(
        Some(&node_region_id("GLOBAL", "proxy-a")),
        box_at(170.0, 40.0),
    );
    // The scrollable's own bounds are required: the offset is content-space.
    assert_eq!(operation.content_offset(), None);
    operation.scrollable(
        Some(&Id::from(PROXIES_PAGE_SCROLL_ID.to_owned())),
        box_at(400.0, 400.0),
        box_at(400.0, 900.0),
        Vector::ZERO,
        &mut RecordedScrollable {
            offset: Rc::new(RefCell::new(None)),
        },
    );
    assert_eq!(operation.content_offset(), Some(170.0 - 400.0));
}

#[test]
fn the_finish_chains_a_scroll_to_the_measured_offset() {
    let mut operation = probe();
    operation.container(Some(&group_card_id("GLOBAL")), box_at(100.0, 200.0));
    operation.container(Some(&group_body_id("GLOBAL")), box_at(110.0, 180.0));
    operation.container(Some(&group_grid_id("GLOBAL")), box_at(150.0, 100.0));
    operation.container(Some(&group_row_id("GLOBAL", 1)), box_at(160.0, 60.0));
    operation.container(
        Some(&node_region_id("GLOBAL", "proxy-a")),
        box_at(170.0, 40.0),
    );
    operation.scrollable(
        Some(&Id::from(PROXIES_PAGE_SCROLL_ID.to_owned())),
        box_at(0.0, 400.0),
        box_at(0.0, 900.0),
        Vector::ZERO,
        &mut RecordedScrollable {
            offset: Rc::new(RefCell::new(None)),
        },
    );
    let Outcome::Chain(next) = operation.finish() else {
        panic!("a measured locate must chain its scroll")
    };
    let recorded = Rc::new(RefCell::new(None));
    let mut scrollable = RecordedScrollable {
        offset: recorded.clone(),
    };
    let mut chained = next;
    chained.scrollable(
        Some(&Id::from(PROXIES_PAGE_SCROLL_ID.to_owned())),
        Rectangle::default(),
        Rectangle::default(),
        Vector::ZERO,
        &mut scrollable,
    );
    assert_eq!(
        recorded.borrow().as_ref().and_then(|offset| offset.y),
        Some(170.0)
    );
}

#[test]
fn the_scroll_is_clamped_to_the_content_that_exists() {
    let mut operation = probe();
    operation.container(Some(&group_card_id("GLOBAL")), box_at(800.0, 200.0));
    operation.container(Some(&group_body_id("GLOBAL")), box_at(810.0, 180.0));
    operation.container(Some(&group_grid_id("GLOBAL")), box_at(850.0, 100.0));
    operation.container(Some(&group_row_id("GLOBAL", 1)), box_at(860.0, 60.0));
    operation.container(
        Some(&node_region_id("GLOBAL", "proxy-a")),
        box_at(870.0, 40.0),
    );
    operation.scrollable(
        Some(&Id::from(PROXIES_PAGE_SCROLL_ID.to_owned())),
        box_at(400.0, 400.0),
        box_at(400.0, 900.0),
        Vector::ZERO,
        &mut RecordedScrollable {
            offset: Rc::new(RefCell::new(None)),
        },
    );
    let Outcome::Chain(next) = operation.finish() else {
        panic!("a measured locate must chain its scroll")
    };
    let recorded = Rc::new(RefCell::new(None));
    let mut scrollable = RecordedScrollable {
        offset: recorded.clone(),
    };
    let mut chained = next;
    chained.scrollable(
        Some(&Id::from(PROXIES_PAGE_SCROLL_ID.to_owned())),
        Rectangle::default(),
        Rectangle::default(),
        Vector::ZERO,
        &mut scrollable,
    );
    assert_eq!(
        recorded.borrow().as_ref().and_then(|offset| offset.y),
        Some(470.0)
    );
}
