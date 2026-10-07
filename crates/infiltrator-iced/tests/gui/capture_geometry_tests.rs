//! Native layout clipping and scroll scope regression cases.
use super::*;
use iced::advanced::widget::operation::scrollable::{AbsoluteOffset, RelativeOffset};

struct ReadOnlyScroll;
impl Scrollable for ReadOnlyScroll {
    fn snap_to(&mut self, _: RelativeOffset<Option<f32>>) {}
    fn scroll_to(&mut self, _: AbsoluteOffset<Option<f32>>) {}
    fn scroll_by(&mut self, _: AbsoluteOffset, _: Rectangle, _: Rectangle) {}
}
fn bounds(y: f32, height: f32) -> Rectangle {
    Rectangle {
        x: 100.0,
        y,
        width: 200.0,
        height,
    }
}

#[derive(Default)]
struct RecordedScroll {
    offset: Option<AbsoluteOffset<Option<f32>>>,
}
impl Scrollable for RecordedScroll {
    fn snap_to(&mut self, _: RelativeOffset<Option<f32>>) {}
    fn scroll_to(&mut self, offset: AbsoluteOffset<Option<f32>>) {
        self.offset = Some(offset);
    }
    fn scroll_by(&mut self, _: AbsoluteOffset, _: Rectangle, _: Rectangle) {}
}
#[test]
fn reveal_uses_native_scroll_then_requires_a_fresh_fully_visible_layout() {
    let region = Id::new("report");
    let scroll = Id::new("native-scroll");
    let mut probe =
        VisibleRegion::new(region.clone(), Size::new(720.0, 480.0)).revealing(scroll.clone());
    probe.scrollable(
        Some(&scroll),
        bounds(200.0, 200.0),
        bounds(200.0, 900.0),
        Vector::new(0.0, 300.0),
        &mut ReadOnlyScroll,
    );
    probe.traverse(&mut |operation| operation.container(Some(&region), bounds(450.0, 180.0)));
    assert!(probe.found.is_none());
    let Outcome::Chain(mut operation) = probe.finish() else {
        panic!("clipped report must request a native scroll, not a receipt");
    };
    let mut state = RecordedScroll::default();
    operation.scrollable(
        Some(&scroll),
        bounds(200.0, 200.0),
        bounds(200.0, 900.0),
        Vector::new(0.0, 300.0),
        &mut state,
    );
    assert_eq!(state.offset.unwrap().y, Some(240.0));
    assert!(
        matches!(operation.finish(), Outcome::Some(None)),
        "native scrolling must request a subsequent UI/layout update without issuing a receipt"
    );
    let mut fresh =
        VisibleRegion::new(region.clone(), Size::new(720.0, 480.0)).revealing(scroll.clone());
    fresh.scrollable(
        Some(&scroll),
        bounds(200.0, 200.0),
        bounds(200.0, 900.0),
        Vector::new(0.0, 240.0),
        &mut ReadOnlyScroll,
    );
    fresh.traverse(&mut |operation| operation.container(Some(&region), bounds(450.0, 180.0)));
    assert!(matches!(fresh.finish(), Outcome::Some(Some(rect)) if rect == bounds(210.0, 180.0)));
}
#[test]
fn native_scroll_cannot_accept_a_report_larger_than_the_viewport() {
    let region = Id::new("report");
    let scroll = Id::new("native-scroll");
    let mut probe =
        VisibleRegion::new(region.clone(), Size::new(720.0, 480.0)).revealing(scroll.clone());
    probe.scrollable(
        Some(&scroll),
        bounds(200.0, 200.0),
        bounds(200.0, 900.0),
        Vector::ZERO,
        &mut ReadOnlyScroll,
    );
    probe.traverse(&mut |operation| operation.container(Some(&region), bounds(220.0, 250.0)));
    assert!(matches!(probe.finish(), Outcome::Some(None)));
}

#[test]
fn an_offscreen_nested_scroll_reveals_through_the_visible_outer_viewport() {
    let region = Id::new("group-results");
    let outer = Id::new("outer-scroll");
    let inner = Id::new("inner-scroll");
    let mut probe =
        VisibleRegion::new(region.clone(), Size::new(720.0, 480.0)).revealing(outer.clone());
    probe.scrollable(
        Some(&outer),
        bounds(80.0, 360.0),
        bounds(80.0, 1600.0),
        Vector::ZERO,
        &mut ReadOnlyScroll,
    );
    probe.traverse(&mut |operation| {
        operation.scrollable(
            Some(&inner),
            bounds(1100.0, 80.0),
            bounds(1100.0, 80.0),
            Vector::ZERO,
            &mut ReadOnlyScroll,
        );
        operation.traverse(&mut |nested| nested.container(Some(&region), bounds(1100.0, 40.0)));
    });
    assert!(probe.found.is_none());
    let Outcome::Chain(mut operation) = probe.finish() else {
        panic!("outer scrolling must reveal the offscreen nested surface");
    };
    let mut state = RecordedScroll::default();
    operation.scrollable(
        Some(&outer),
        bounds(80.0, 360.0),
        bounds(80.0, 1600.0),
        Vector::ZERO,
        &mut state,
    );
    assert_eq!(state.offset.unwrap().y, Some(860.0));
    assert!(matches!(operation.finish(), Outcome::Some(None)));
    let mut fresh =
        VisibleRegion::new(region.clone(), Size::new(720.0, 480.0)).revealing(outer.clone());
    fresh.scrollable(
        Some(&outer),
        bounds(80.0, 360.0),
        bounds(80.0, 1600.0),
        Vector::new(0.0, 860.0),
        &mut ReadOnlyScroll,
    );
    fresh.traverse(&mut |operation| {
        operation.scrollable(
            Some(&inner),
            bounds(1100.0, 80.0),
            bounds(1100.0, 80.0),
            Vector::ZERO,
            &mut ReadOnlyScroll,
        );
        operation.traverse(&mut |nested| nested.container(Some(&region), bounds(1100.0, 40.0)));
    });
    assert!(matches!(fresh.finish(), Outcome::Some(Some(rect)) if rect == bounds(240.0, 40.0)));
}

#[test]
fn an_offscreen_or_partially_clipped_control_cannot_produce_visible_evidence() {
    let id = Id::new("control");
    let mut probe = VisibleRegion::new(id.clone(), Size::new(720.0, 480.0));
    probe.container(Some(&id), bounds(500.0, 80.0));
    assert!(probe.found.is_none());
    probe.scrollable(
        None,
        bounds(200.0, 200.0),
        bounds(200.0, 800.0),
        Vector::ZERO,
        &mut ReadOnlyScroll,
    );
    probe.traverse(&mut |operation| operation.container(Some(&id), bounds(380.0, 80.0)));
    assert!(probe.found.is_none());
    probe.container(Some(&id), bounds(f32::NAN, 80.0));
    assert!(probe.found.is_none());
}

#[test]
fn native_scroll_translation_is_measured_and_does_not_leak_into_sibling_controls() {
    let id = Id::new("control");
    let mut probe = VisibleRegion::new(id.clone(), Size::new(720.0, 480.0));
    probe.scrollable(
        None,
        bounds(200.0, 200.0),
        bounds(200.0, 800.0),
        Vector::new(0.0, 100.0),
        &mut ReadOnlyScroll,
    );
    probe.traverse(&mut |operation| operation.container(Some(&id), bounds(310.0, 80.0)));
    assert_eq!(probe.found, Some(bounds(210.0, 80.0)));
    assert_eq!(probe.translation, Vector::ZERO);
    probe.container(Some(&id), bounds(10.0, 80.0));
    assert_eq!(probe.found, Some(bounds(10.0, 80.0)));
}

#[test]
fn a_visible_inspector_cannot_pass_when_its_required_history_is_missing_or_clipped() {
    let card = Id::new("card");
    let chart = Id::new("chart");
    let mut probe =
        VisibleRegion::new(card.clone(), Size::new(720.0, 480.0)).requiring(chart.clone());
    probe.container(Some(&card), bounds(40.0, 400.0));
    assert!(matches!(probe.finish(), Outcome::Some(None)));
    probe.scrollable(
        None,
        bounds(100.0, 160.0),
        bounds(100.0, 600.0),
        Vector::ZERO,
        &mut ReadOnlyScroll,
    );
    probe.traverse(&mut |operation| operation.container(Some(&chart), bounds(220.0, 90.0)));
    assert!(matches!(probe.finish(), Outcome::Some(None)));
    let mut probe =
        VisibleRegion::new(card.clone(), Size::new(720.0, 480.0)).requiring(chart.clone());
    probe.container(Some(&card), bounds(40.0, 400.0));
    probe.scrollable(
        None,
        bounds(100.0, 160.0),
        bounds(100.0, 600.0),
        Vector::ZERO,
        &mut ReadOnlyScroll,
    );
    probe.traverse(&mut |operation| operation.container(Some(&chart), bounds(110.0, 90.0)));
    assert!(matches!(probe.finish(), Outcome::Some(Some(rect)) if rect == bounds(40.0, 400.0)));
}

#[test]
fn fractional_native_layout_is_contained_without_comparing_reconstructed_sizes() {
    let id = Id::new("control");
    let mut probe = VisibleRegion::new(id.clone(), Size::new(720.0, 480.0));
    let control = Rectangle {
        x: 50.0,
        y: 37.199997,
        width: 620.0,
        height: 405.6,
    };
    probe.container(Some(&id), control);
    assert_eq!(probe.found, Some(control));
}

#[test]
fn search_evidence_requires_both_the_result_and_separate_input_to_be_fully_visible() {
    let card = Id::new("result");
    let input = Id::new("search");
    let mut probe =
        VisibleRegion::new(card.clone(), Size::new(720.0, 480.0)).requiring_peer(input.clone());
    probe.container(Some(&card), bounds(300.0, 70.0));
    assert!(matches!(probe.finish(), Outcome::Some(None)));
    probe.container(Some(&input), bounds(460.0, 40.0));
    assert!(matches!(probe.finish(), Outcome::Some(None)));
    let mut probe =
        VisibleRegion::new(card.clone(), Size::new(720.0, 480.0)).requiring_peer(input.clone());
    probe.container(Some(&card), bounds(300.0, 70.0));
    probe.container(Some(&input), bounds(100.0, 40.0));
    assert!(matches!(probe.finish(), Outcome::Some(Some(rect)) if rect == bounds(300.0, 70.0)));
}

#[test]
fn every_required_native_control_must_be_present_and_visible_without_duplicate_identities() {
    let card = Id::new("multi-card");
    let first = Id::new("name-field");
    let second = Id::new("run-button");
    let third = Id::new("cancel-button");
    let make = || {
        VisibleRegion::new(card.clone(), Size::new(720.0, 480.0))
            .requiring(first.clone())
            .requiring(second.clone())
            .requiring(third.clone())
    };
    let mut missing = make();
    missing.container(Some(&card), bounds(40.0, 400.0));
    missing.container(Some(&third), bounds(150.0, 30.0));
    assert!(
        matches!(missing.finish(), Outcome::Some(None)),
        "the last declared control cannot satisfy the first two"
    );
    let mut clipped = make();
    clipped.container(Some(&card), bounds(40.0, 400.0));
    clipped.container(Some(&first), bounds(470.0, 30.0));
    clipped.container(Some(&second), bounds(100.0, 30.0));
    clipped.container(Some(&third), bounds(150.0, 30.0));
    assert!(
        matches!(clipped.finish(), Outcome::Some(None)),
        "a clipped first control blocks the receipt"
    );
    let mut complete = make();
    complete.container(Some(&card), bounds(40.0, 400.0));
    complete.container(Some(&first), bounds(60.0, 30.0));
    complete.container(Some(&second), bounds(100.0, 30.0));
    complete.container(Some(&third), bounds(150.0, 30.0));
    assert!(matches!(complete.finish(), Outcome::Some(Some(rect)) if rect == bounds(40.0, 400.0)));
    complete.container(Some(&first), bounds(70.0, 30.0));
    assert!(
        matches!(complete.finish(), Outcome::Some(None)),
        "duplicate native identity is ambiguous"
    );
}

#[test]
fn reveal_centres_the_complete_panel_instead_of_oscillating_between_required_controls() {
    let panel = Id::new("panel");
    let advanced = Id::new("advanced");
    let save = Id::new("save");
    let scroll = Id::new("panel-scroll");
    let mut probe = VisibleRegion::new(panel.clone(), Size::new(720.0, 480.0))
        .requiring(advanced.clone())
        .requiring(save.clone())
        .revealing(scroll.clone());
    probe.scrollable(
        Some(&scroll),
        bounds(200.0, 200.0),
        bounds(200.0, 900.0),
        Vector::ZERO,
        &mut ReadOnlyScroll,
    );
    probe.traverse(&mut |operation| {
        operation.container(Some(&panel), bounds(600.0, 180.0));
        operation.container(Some(&advanced), bounds(630.0, 20.0));
        operation.container(Some(&save), bounds(750.0, 20.0));
    });
    let Outcome::Chain(mut operation) = probe.finish() else {
        panic!("panel needs actual scroll");
    };
    let mut state = RecordedScroll::default();
    operation.scrollable(
        Some(&scroll),
        bounds(200.0, 200.0),
        bounds(200.0, 900.0),
        Vector::ZERO,
        &mut state,
    );
    assert_eq!(
        state.offset.unwrap().y,
        Some(390.0),
        "centre the whole panel, not its last clipped control"
    );
    assert!(matches!(operation.finish(), Outcome::Some(None)));
    let mut fresh = VisibleRegion::new(panel.clone(), Size::new(720.0, 480.0))
        .requiring(advanced.clone())
        .requiring(save.clone())
        .revealing(scroll.clone());
    fresh.scrollable(
        Some(&scroll),
        bounds(200.0, 200.0),
        bounds(200.0, 900.0),
        Vector::new(0.0, 390.0),
        &mut ReadOnlyScroll,
    );
    fresh.traverse(&mut |operation| {
        operation.container(Some(&panel), bounds(600.0, 180.0));
        operation.container(Some(&advanced), bounds(630.0, 20.0));
        operation.container(Some(&save), bounds(750.0, 20.0));
    });
    assert!(matches!(fresh.finish(),Outcome::Some(Some(rect)) if rect==bounds(210.0,180.0)));
}
