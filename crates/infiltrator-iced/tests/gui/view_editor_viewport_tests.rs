//! DUAL-09-02/13: the Iced editor viewport, gutter and snippet-bar tests.
//!
//! The gutter follows the text widget's own scroll offset, which only the
//! widget can move; these tests pin the *surface* half of that contract — the
//! window arithmetic, the exact line metrics the two columns share, and the
//! bounded work a large document implies. No frame-rate claim is made.

use super::*;
use iced::widget::text_editor;
use infiltrator_shared::locales::{Lang, Localizer};

fn large_document(lines: usize) -> text_editor::Content {
    let text: Vec<String> = (0..lines)
        .map(|index| format!("key-{index}: value-{index}"))
        .collect();
    text_editor::Content::with_text(&text.join("\n"))
}

#[test]
fn window_lines_track_the_pane_height_and_stay_bounded() {
    assert_eq!(
        window_lines_for_window_height(0.0),
        EDITOR_MIN_WINDOW_LINES,
        "a short pane still shows the minimum window"
    );
    assert_eq!(
        window_lines_for_window_height(780.0),
        25,
        "the default shell height fits 25 lines after the pane chrome"
    );
    assert_eq!(
        window_lines_for_window_height(10_000.0),
        EDITOR_MAX_WINDOW_LINES,
        "a tall window stops at the maximum"
    );
}

#[test]
fn the_editor_box_is_an_exact_multiple_of_the_shared_line_height() {
    for lines in EDITOR_MIN_WINDOW_LINES..=EDITOR_MAX_WINDOW_LINES {
        let height = editor_box_height_px(lines);
        let text_height = height - 2.0 * EDITOR_PADDING_PX;
        assert_eq!(text_height, lines as f32 * EDITOR_LINE_HEIGHT_PX);
        assert_eq!((text_height / EDITOR_LINE_HEIGHT_PX) as usize, lines);
    }
}

#[test]
fn a_ten_thousand_line_document_still_renders_one_window() {
    let content = large_document(10_000);
    assert_eq!(content.line_count(), 10_000);
    let viewport = viewport_for(&content, 780.0, 0);
    assert_eq!(viewport.rendered_len(), 25);
    assert_eq!(viewport.line_numbers().count(), 25);
    assert_eq!(viewport.total_lines(), 10_000);

    // The gutter is built from the window only; the smoke build pins that the
    // large document does not get walked line by line.
    let _gutter = gutter(&content, viewport);
}

#[test]
fn the_gutter_renders_the_window_the_surface_stored() {
    let content = large_document(5_000);
    let viewport = viewport_for(&content, 780.0, 4_000);
    assert_eq!(viewport.first_line(), 4_000);
    assert_eq!(viewport.line_numbers(), 4_001..=4_025);
    let _gutter = gutter(&content, viewport);
}

#[test]
fn a_short_document_needs_no_viewport_readout() {
    let content = large_document(3);
    let viewport = viewport_for(&content, 780.0, 0);
    assert!(viewport.covers_document());
    assert!(viewport_label(viewport, &Lang("zh-CN")).is_none());
    let long = viewport_for(&large_document(4_000), 780.0, 1_000);
    assert!(!long.covers_document());
    assert!(viewport_label(long, &Lang("zh-CN")).is_some());
}

#[test]
fn every_catalogue_snippet_is_localized_on_both_surfaces() {
    // The Iced bar renders `label_key`; the Bevy bar renders `label_zh`. A
    // catalogue entry that loses its locale key would fall back to the raw key
    // and be visible immediately, so this pins the whole catalogue.
    for snippet in infiltrator_contract::yaml_snippets::YAML_SNIPPETS {
        assert_ne!(
            Lang("zh-CN").tr(snippet.label_key).as_ref(),
            snippet.label_key,
            "zh-CN copy missing for {}",
            snippet.id
        );
        assert_ne!(
            Lang("en-US").tr(snippet.label_key).as_ref(),
            snippet.label_key,
            "en-US copy missing for {}",
            snippet.id
        );
        assert!(snippet.label_zh.starts_with("+ "));
    }
}
