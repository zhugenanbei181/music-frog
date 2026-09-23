use super::*;

fn placement(x: i32, y: i32) -> MiniHudPlacement {
    MiniHudPlacement::new(x, y)
}

#[test]
fn a_handle_without_a_live_window_refuses_the_placement() {
    let handle = IcedMiniHudWindowHandle::default();
    assert!(!handle.apply_placement(placement(10, 20)));
    assert!(handle.take_pending().is_empty());
    assert!(!handle.set_visible(true));
}

#[test]
fn a_live_window_accepts_exactly_once_per_request() {
    let handle = IcedMiniHudWindowHandle::default();
    handle.mark_live(true);
    assert!(handle.apply_placement(placement(10, 20)));
    assert!(handle.apply_placement(placement(30, 40)));
    assert_eq!(
        handle.take_pending(),
        vec![placement(10, 20), placement(30, 40)]
    );
    assert!(
        handle.take_pending().is_empty(),
        "draining is destructive so the update path never re-applies a move"
    );
    handle.mark_live(false);
    assert!(!handle.apply_placement(placement(50, 60)));
}
