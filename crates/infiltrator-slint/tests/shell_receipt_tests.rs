//! SLINT-030: the chrome a frame states about itself comes from the bindings.
//!
//! Since `SPACEUTIL-007` the space-utilization judge builds its measurement
//! box from the frame's own `CAPTURE_CHROME` line instead of paddings it
//! remembered, so the projection that reads the shell's bindings back into
//! those numbers is load-bearing: if it mis-divides the ui unit, mixes up two
//! of the paddings, or publishes a band the mounted form never reserves, the
//! receipt describes a box no surface painted. The shapes themselves - a rail
//! at 600..839px, a bar on a touch surface - are certified per frame by the
//! judge over the published batch and by the exported ladder, which is what
//! the line has to agree with.

use infiltrator_contract::responsive_viewport::SidebarForm;
use infiltrator_contract::shell_receipt::ShellChromeFacts;
use infiltrator_slint::AppWindow;
use infiltrator_slint::shell_receipt_host::shell_chrome_facts;

fn chrome_of(window: &AppWindow) -> ShellChromeFacts {
    let facts = shell_chrome_facts(window);
    // The line is the wire the judge parses, so every fact must survive it.
    let line = facts.to_marker_line();
    assert_eq!(
        ShellChromeFacts::parse_marker_line(&line),
        Some(facts),
        "{line}"
    );
    facts
}

#[test]
fn a_cold_frame_states_the_bar_it_mounts_and_the_band_it_reserves() {
    // An unresized window is width 0, so the ladder answers `compact`: bottom
    // navigation, no column, and the dock band the shell reserves for it. That
    // pairing is the invariant SPACEUTIL-006 was written for, checked here on
    // the projection rather than only in the judge.
    let window = AppWindow::new().expect("the shell builds");
    let facts = chrome_of(&window);
    assert_eq!(facts.nav_form, SidebarForm::BottomNav, "{facts:?}");
    assert_eq!(facts.nav_column_px, 0, "a bar reserves no side column");
    assert_eq!(
        facts.nav_stack_bottom_px, 0,
        "a bar owns no vertical stack, so it states no stack end either"
    );
    assert_eq!(
        facts.nav_stack_bottom_px > 0,
        facts.nav_column_px > 0,
        "the column and the stack end are one fact seen from two edges"
    );
    assert_eq!(
        facts.nav_content_offset_px, 0,
        "a bar indents nothing, so it states no leading either"
    );
    assert_eq!(
        facts.nav_content_offset_px > 0,
        facts.insets_nav_content(),
        "the leading is published exactly by the form that labels its rows"
    );
    assert!(facts.reserves_dock_band(), "{facts:?}");
    assert_eq!(
        facts.page_margin_px, 16,
        "the compact chrome keeps its own margin"
    );
    assert_eq!(facts.shell_bottom_air_px, 0, "and no page air of its own");
    assert_eq!(facts.scale_percent, 100);
}

#[test]
fn the_receipt_states_logical_units_and_not_the_multiplied_pixels() {
    // `LumenUiScale.unit` is `1px * page-scale-percent / 100`, so a binding
    // that reads `16 * unit` is 32 device pixels at 200% and still 16 logical
    // units. The receipt states the second, because that is the language the
    // exported column, the thresholds and the judge's box are stated in - and
    // a remembered constant could not follow the zoom at all.
    use infiltrator_slint::shell_receipt_host::logical_units;
    assert_eq!(logical_units(16.0, 100), 16);
    assert_eq!(
        logical_units(32.0, 200),
        16,
        "the same margin through the zoom"
    );
    assert_eq!(logical_units(84.0, 100), 84);
    assert_eq!(
        logical_units(168.0, 200),
        84,
        "the same band through the zoom"
    );
    assert_eq!(logical_units(240.0, 150), 160);
    // A scale that cannot be a unit is clamped rather than dividing by zero,
    // and a negative length is never published as a negative margin.
    assert_eq!(logical_units(16.0, 0), 16);
    assert_eq!(logical_units(-4.0, 100), 0);
}
