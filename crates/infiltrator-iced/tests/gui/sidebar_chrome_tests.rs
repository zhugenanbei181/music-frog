//! What the sidebar's own chrome receipt states, and when it must refuse to state it.
//!
//! The receipt is the frame's description of the pane behind its pixels, so these tests
//! pin the two halves that make it honest: the form resolution the pane paints is the
//! one the receipt publishes, and a form whose boxes were never measured publishes
//! nothing at all rather than a remembered number.
//! test-intent: behavior

use super::*;

fn box_at(y: f32, height: f32) -> Rectangle {
    Rectangle {
        x: 0.0,
        y,
        width: 240.0,
        height,
    }
}

fn measured() -> SidebarChromeReceipt {
    SidebarChromeReceipt {
        stack_end: Some(box_at(100.0, 382.0)),
        toggle: Some(box_at(54.0, 28.0)),
        dock: Some(Rectangle {
            x: 0.0,
            y: 700.0,
            width: 400.0,
            height: 72.0,
        }),
    }
}

#[test]
fn the_preference_and_the_viewport_resolve_into_one_painted_form() {
    let expanded =
        SidebarPresentation::resolve(ViewportTier::Expanded, SidebarExpansionPreference::Expanded);
    assert_eq!(expanded.form(), SidebarForm::Standard);
    assert!(expanded.labelled());
    assert!(expanded.toggle_available());
    assert_eq!(
        expanded.nav_content_offset_px(),
        NAV_CONTENT_OFFSET_PX,
        "a labelled column states the leading it paints"
    );

    let collapsed = SidebarPresentation::resolve(
        ViewportTier::Expanded,
        SidebarExpansionPreference::Collapsed,
    );
    assert_eq!(collapsed.form(), SidebarForm::Rail);
    assert!(!collapsed.labelled());
    assert!(
        collapsed.toggle_available(),
        "a collapsed desktop column still offers the way back"
    );
    assert_eq!(collapsed.nav_content_offset_px(), 0);
}

#[test]
fn a_viewport_that_cannot_expand_offers_no_affordance() {
    for (tier, form) in [
        (ViewportTier::Compact, SidebarForm::BottomNav),
        (ViewportTier::Medium, SidebarForm::Rail),
    ] {
        let presentation = SidebarPresentation::resolve(tier, SidebarExpansionPreference::Expanded);
        assert_eq!(presentation.form(), form);
        assert!(!presentation.toggle_available(), "{tier:?}");
        assert_eq!(presentation.nav_content_offset_px(), 0);
    }
}

#[test]
fn a_frame_whose_boxes_are_unmeasured_publishes_nothing() {
    let presentation =
        SidebarPresentation::resolve(ViewportTier::Expanded, SidebarExpansionPreference::Expanded);
    assert!(
        shell_chrome_facts(
            presentation,
            SidebarChromeReceipt::default(),
            ViewportTier::Expanded
        )
        .is_none()
    );
    assert!(shell_chrome_facts(presentation, measured(), ViewportTier::Expanded).is_some());
}

#[test]
fn the_receipt_states_the_boxes_the_pane_painted() {
    let presentation =
        SidebarPresentation::resolve(ViewportTier::Expanded, SidebarExpansionPreference::Expanded);
    let facts = shell_chrome_facts(presentation, measured(), ViewportTier::Expanded)
        .expect("a measured form publishes");
    assert_eq!(facts.nav_form, SidebarForm::Standard);
    assert_eq!(facts.nav_column_px, 240);
    assert_eq!(facts.nav_stack_bottom_px, 482);
    assert_eq!(facts.nav_content_offset_px, NAV_CONTENT_OFFSET_PX);
    assert_eq!(
        facts.page_margin_px,
        ViewportTier::Expanded.content_padding_px()
    );
    assert_eq!(facts.shell_top_air_px, 0);
    assert_eq!(facts.shell_bottom_air_px, 0);
    assert_eq!(facts.dock_band_px, 0);
    assert_eq!(facts.scale_percent, 100);
    assert_eq!(
        facts.sidebar_expansion,
        SidebarExpansionPreference::Expanded
    );
    assert!(!facts.touch_surface);
    assert_eq!(facts.sidebar_toggle_px, 82 + TOGGLE_GAP_PX);
    assert_eq!(
        ShellChromeFacts::parse_marker_line(&facts.to_marker_line()),
        Some(facts)
    );
}

#[test]
fn a_bottom_bar_frame_states_its_own_band_and_no_column() {
    let presentation =
        SidebarPresentation::resolve(ViewportTier::Compact, SidebarExpansionPreference::Expanded);
    let mut receipt = measured();
    receipt.stack_end = None;
    receipt.toggle = None;
    let facts = shell_chrome_facts(presentation, receipt, ViewportTier::Compact)
        .expect("a bar has no column to measure");
    assert_eq!(facts.nav_form, SidebarForm::BottomNav);
    assert_eq!(facts.nav_column_px, 0);
    assert_eq!(facts.nav_stack_bottom_px, 0);
    assert_eq!(facts.sidebar_toggle_px, 0);
    assert_eq!(facts.dock_band_px, 72);
    assert_eq!(
        ShellChromeFacts::parse_marker_line(&facts.to_marker_line()),
        Some(facts)
    );
}
