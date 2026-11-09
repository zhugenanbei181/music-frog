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

#[test]
fn window_slicing_and_search_operate_within_budget_on_large_documents() {
    // Generate 4,000-line and 50,000-line synthetic documents.
    let doc_4k = (0..4_000)
        .map(|i| format!("  - DOMAIN-SUFFIX,node-{i}.example.com,DIRECT"))
        .collect::<Vec<_>>()
        .join("\n");

    let viewport_4k = EditorViewport::new(4_000, 28, 500);
    let indexed_4k = LineIndexedDocument::new(&doc_4k);
    let sliced_4k = viewport_4k.slice_visible_lines(&indexed_4k);
    assert_eq!(sliced_4k.len(), 28);
    assert_eq!(sliced_4k[0].0, 501);
    assert_eq!(
        sliced_4k[0].1,
        "  - DOMAIN-SUFFIX,node-500.example.com,DIRECT"
    );

    // Search within window hits line in window, ignores lines outside.
    let hits = viewport_4k.search_in_window(&indexed_4k, "node-505");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].line_number, 506);

    let hits_outside = viewport_4k.search_in_window(&indexed_4k, "node-100");
    assert!(
        hits_outside.is_empty(),
        "query outside window must not be returned"
    );

    let receipt_4k =
        viewport_4k.measure_performance(&indexed_4k, DocumentPerformanceBudget::standard_4k());
    assert!(
        receipt_4k.status.is_within_budget(),
        "4k performance receipt failed: {:?}",
        receipt_4k.status
    );
    // The deterministic half of the window promise: a slice hands back exactly
    // the lines its window renders, never the lines above it.
    assert_eq!(receipt_4k.slice_lines, receipt_4k.window_lines);
    assert_eq!(receipt_4k.slice_lines, 28);

    // 50k document scaling test
    let doc_50k = (0..50_000)
        .map(|i| format!("  - DOMAIN-SUFFIX,node-{i}.example.com,DIRECT"))
        .collect::<Vec<_>>()
        .join("\n");

    let viewport_50k = EditorViewport::new(50_000, 240, 25_000);
    let indexed_50k = LineIndexedDocument::new(&doc_50k);
    let sliced_50k = viewport_50k.slice_visible_lines(&indexed_50k);
    assert_eq!(sliced_50k.len(), 240);
    assert_eq!(sliced_50k[0].0, 25_001);
    assert_eq!(sliced_50k[239].0, 25_240);

    let receipt_50k =
        viewport_50k.measure_performance(&indexed_50k, DocumentPerformanceBudget::large_50k());
    assert!(
        receipt_50k.status.is_within_budget(),
        "50k performance receipt failed: {:?}",
        receipt_50k.status
    );
    assert_eq!(receipt_50k.slice_lines, receipt_50k.window_lines);
    assert_eq!(receipt_50k.slice_lines, 240);
}

/// The window op's cost is a property of the window, not of the machine.
///
/// This is the half of `EDITORPERF-001` that a wall-clock number cannot prove:
/// wherever the window sits, it materialises its own lines and nothing else,
/// so repeated scrolling and in-window search stay bounded by the window even
/// in a 50,000-line document.
#[test]
fn window_work_is_proportional_to_the_window_not_the_document() {
    let doc = (0..50_000)
        .map(|i| format!("  - DOMAIN-SUFFIX,node-{i}.example.com,DIRECT"))
        .collect::<Vec<_>>()
        .join("\n");
    let indexed = LineIndexedDocument::new(&doc);
    assert_eq!(indexed.line_count(), 50_000);

    for first in [0usize, 1, 25_000, 49_760] {
        // A whole line body, so the search matches one line instead of every
        // line whose number merely starts with the same digits.
        let window = EditorViewport::new(50_000, 240, first);
        let lines = window.slice_visible_lines(&indexed);
        assert_eq!(
            lines.len(),
            window.rendered_len(),
            "window at {first} must render exactly its own window"
        );
        assert_eq!(lines[0].0, first + 1);
        assert_eq!(lines[lines.len() - 1].0, first + lines.len());
        assert_eq!(
            window
                .search_in_window(&indexed, &format!("node-{}.example.com", first + 7))
                .len(),
            1,
            "the hit inside the window is found wherever the window sits"
        );
    }

    // A hit outside the window stays invisible: the search never reads past it.
    let middle = EditorViewport::new(50_000, 240, 25_000);
    assert!(
        middle
            .search_in_window(&indexed, "node-999.example.com")
            .is_empty(),
        "a line outside the window must not be searched"
    );
}

/// The index is a reading of the text, not a second version of it.
///
/// Every line the index reports has to be the line `str::lines` reports, and
/// the document has to be spelled out of those same lines: if indexing ever
/// drifts from the language's own line counting, this fails on content rather
/// than on a timing number nobody can reproduce.
#[test]
fn line_index_agrees_with_the_languages_own_line_counting() {
    let documents = [
        "",
        "one line",
        "one line\n",
        "a\nb\nc",
        "a\nb\nc\n",
        "a\r\nb\r\nc",
        "a\r\n",
        "a\n\nb",
        "\n",
        "\n\n\n",
        "trailing carriage\r",
    ];
    for text in documents {
        let indexed = LineIndexedDocument::new(text);
        let expected: Vec<&str> = text.lines().collect();
        assert_eq!(
            indexed.line_count(),
            expected.len(),
            "line count for {text:?}"
        );
        let indexed_lines: Vec<&str> = (0..indexed.line_count())
            .filter_map(|index| indexed.line(index))
            .collect();
        assert_eq!(indexed_lines, expected, "lines for {text:?}");
        // Every byte range points back at the line it claims.
        for index in 0..indexed.line_count() {
            let range = indexed
                .line_range(index)
                .unwrap_or_else(|| panic!("{text:?} line {index}"));
            assert_eq!(&text[range.clone()], indexed.line(index).expect("line"));
            assert!(range.end <= text.len());
        }
        // One past the end is not a line.
        assert_eq!(indexed.line(expected.len()), None);
    }
}

/// The window and a plain `str::lines()` read of the same text agree, line for
/// line, on every shape of ending a document has.
#[test]
fn window_matches_lines_semantics_at_document_edges() {
    let short = "line-1\nline-2\nline-3";
    let indexed_short = LineIndexedDocument::new(short);
    assert_eq!(
        EditorViewport::new(9, 4, 6).slice_visible_lines(&indexed_short),
        Vec::<(usize, &str)>::new(),
        "a window past the end renders nothing instead of inventing lines"
    );
    assert_eq!(
        EditorViewport::new(3, 2, 1).slice_visible_lines(&indexed_short),
        vec![(2, "line-2"), (3, "line-3")],
        "a last line with no trailing newline is still a line"
    );
    let trailing = LineIndexedDocument::new("a\nb\n");
    assert_eq!(
        EditorViewport::new(4, 4, 0).slice_visible_lines(&trailing),
        vec![(1, "a"), (2, "b")],
        "a trailing newline does not add a phantom line"
    );
    let empty = LineIndexedDocument::new("");
    assert_eq!(
        EditorViewport::new(3, 3, 0).slice_visible_lines(&empty),
        Vec::<(usize, &str)>::new()
    );
    let crlf = "a\r\nb\r\nc";
    let indexed_crlf = LineIndexedDocument::new(crlf);
    assert_eq!(
        EditorViewport::new(3, 3, 0).slice_visible_lines(&indexed_crlf),
        crlf.lines()
            .enumerate()
            .map(|(index, line)| (index + 1, line))
            .collect::<Vec<_>>()
    );
}
