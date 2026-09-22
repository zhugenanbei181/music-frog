//! DUAL-09-02/13: shared editor viewport window model tests.

use super::*;

#[test]
fn window_clamps_to_the_document_and_reports_hidden_lines() {
    let viewport = EditorViewport::new(10_000, 240, 0);
    assert_eq!(viewport.rendered_len(), 240);
    assert_eq!(viewport.hidden_above(), 0);
    assert_eq!(viewport.hidden_below(), 10_000 - 240);
    assert_eq!(EditorViewport::max_first_line(10_000, 240), 9_760);

    // Scrolling past the end stops at the last full window.
    let bottom = viewport.scrolled(100_000);
    assert_eq!(bottom.first_line(), 9_760);
    assert_eq!(bottom.last_line(), 9_999);
    assert_eq!(bottom.hidden_below(), 0);

    // Scrolling above the top stops at line 1.
    let top = bottom.scrolled(-100_000);
    assert_eq!(top.first_line(), 0);
    assert_eq!(top.hidden_above(), 0);
}

#[test]
fn a_document_shorter_than_the_window_is_fully_covered() {
    let viewport = EditorViewport::top(12, 240);
    assert!(viewport.covers_document());
    assert_eq!(viewport.rendered_len(), 12);
    assert_eq!(viewport.line_numbers(), 1..=12);
    assert_eq!(viewport.hidden_above(), 0);
    assert_eq!(viewport.hidden_below(), 0);
}

#[test]
fn follow_caret_moves_the_window_only_when_the_caret_leaves_it() {
    let viewport = EditorViewport::top(1_000, 100).scrolled(400);
    assert_eq!(viewport.first_line(), 400);

    // A caret inside the window never moves it.
    assert_eq!(viewport.follow_caret(450).first_line(), 400);

    // A caret below the window scrolls it just enough to end on the caret.
    assert_eq!(viewport.follow_caret(520).first_line(), 420);

    // A caret above the window scrolls it up to the caret.
    assert_eq!(viewport.follow_caret(12).first_line(), 11);
}

#[test]
fn window_arithmetic_is_shared_and_deterministic_on_a_large_document() {
    // DUAL-09-13: the bounded window means the rendered line count never grows
    // with the document — 10,000 lines still render 240, and a 28-line pane
    // still renders 28. No frame-rate claim is made anywhere.
    let render_window = EditorViewport::new(10_000, 240, 5_000);
    assert_eq!(render_window.line_numbers().count(), 240);
    let pane_window = EditorViewport::new(10_000, 28, 5_000);
    assert_eq!(pane_window.line_numbers().count(), 28);
    assert_eq!(pane_window.line_numbers(), 5_001..=5_028);
}

#[test]
fn document_changes_reanchor_the_window_within_the_new_bounds() {
    let viewport = EditorViewport::new(10_000, 240, 9_900);
    assert_eq!(viewport.first_line(), 9_760);
    let shorter = viewport.with_document(100, 240);
    assert_eq!(shorter.first_line(), 0);
    assert_eq!(shorter.total_lines(), 100);
}

#[test]
fn indent_reference_model_counts_steps_of_two_spaces() {
    assert_eq!(line_indent_columns("  - name: x"), 2);
    assert_eq!(line_indent_level("  - name: x"), 1);
    assert_eq!(line_indent_level("      - DIRECT"), 3);
    assert_eq!(line_indent_level("- MATCH"), 0);
    assert_eq!(line_indent_level("\t- tabbed"), 1);
    assert_eq!(line_indent_level("   three"), 1);

    let guides = indent_guides("      - DIRECT");
    assert_eq!(guides.len(), 3);
    assert_eq!(guides[2].level, 3);
    assert_eq!(guides[2].column, 6);
    assert!(indent_guides("# comment").is_empty());
}
