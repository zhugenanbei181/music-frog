//! DUAL-09-02/13: shared editor viewport window model.
//!
//! Both profile editors render a *bounded window* of the document instead of
//! the whole file, and both now show a line-number gutter for that window.
//! This module owns the arithmetic of the window — which lines are rendered,
//! how a wheel delta moves it, how it follows the caret, how many lines are
//! hidden above/below — so the two surfaces cannot drift apart. The window
//! size stays a surface parameter (`window_lines`): the Iced surface sizes it
//! from the pane height, the Bevy surface keeps its render limit.
//!
//! Honest boundary: this is a *bounded window*, not virtual scrolling. The
//! model never claims O(1) random access into the document, and no surface
//! claims a frame-rate target; the closed evidence is the arithmetic itself
//! plus counted rendered rows (see the DUAL-09-13 ledger row).
//!
//! Two invariants make the window the single source of truth:
//!
//! * every mutator clamps immediately, so the stored `first_line` is always a
//!   valid top line (a surface that mirrors a text widget's own scroll offset
//!   can therefore accumulate deltas without ever leaving the document);
//! * [`EditorViewport::first_line`]/[`EditorViewport::last_line`] are always
//!   inside `0..total_lines`, so a render path can index lines directly.

use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;
use std::time::Instant;

/// Performance budget for document viewport and window operations.
///
/// Guaranteed upper bounds ensure editor interaction (opening, scrolling,
/// slicing visible lines, search within window) remains responsive on
/// both standard (e.g. 4k lines) and large (e.g. 50k lines) documents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentPerformanceBudget {
    /// Maximum allowed microseconds to compute a windowed slice of lines.
    pub max_slice_micros: u64,
    /// Maximum allowed microseconds to compute a scroll or caret step.
    pub max_scroll_micros: u64,
    /// Maximum number of lines rendered in one window view.
    pub max_render_window_lines: usize,
}

impl Default for DocumentPerformanceBudget {
    fn default() -> Self {
        Self {
            // A 60 FPS frame allows ~16.6ms total. Editor slicing must not
            // consume more than 2.5ms so layout/rendering has ample headroom.
            max_slice_micros: 2_500,
            max_scroll_micros: 500,
            max_render_window_lines: 240,
        }
    }
}

impl DocumentPerformanceBudget {
    /// Strict budget for standard 4,000-line documents.
    pub const fn standard_4k() -> Self {
        Self {
            max_slice_micros: 2_000,
            max_scroll_micros: 500,
            max_render_window_lines: 240,
        }
    }

    /// Scaled budget for large 50,000-line documents.
    pub const fn large_50k() -> Self {
        Self {
            max_slice_micros: 8_000,
            max_scroll_micros: 1_000,
            max_render_window_lines: 240,
        }
    }
}

/// Detailed status classification of a document performance verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentPerformanceStatus {
    WithinBudget,
    ExceededSliceBudget {
        actual_micros: u64,
        max_micros: u64,
    },
    ExceededScrollBudget {
        actual_micros: u64,
        max_micros: u64,
    },
    ExceededWindowLimit {
        actual_lines: usize,
        max_lines: usize,
    },
}

impl DocumentPerformanceStatus {
    pub fn is_within_budget(&self) -> bool {
        matches!(self, Self::WithinBudget)
    }
}

/// Verification receipt measuring that editor operations on a document
/// scale with the viewport window size rather than document length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentPerformanceReceipt {
    pub document_lines: usize,
    pub window_lines: usize,
    pub slice_duration_micros: u64,
    pub scroll_duration_micros: u64,
    pub status: DocumentPerformanceStatus,
    pub budget: DocumentPerformanceBudget,
}

/// A search hit found within the visible editor viewport window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSearchResult {
    /// 1-based line number inside the document.
    pub line_number: usize,
    /// 0-based character column offset within the line.
    pub column: usize,
    /// Length of the matched query in bytes.
    pub match_length: usize,
}
/// One indentation step of the shared indent-reference model. Config YAML in
/// this project is written with two-space indentation (the same width the
/// AST-preserving formatter normalizes to).
pub const INDENT_WIDTH: usize = 2;

/// One reference-line position inside a line's leading indentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndentGuide {
    /// 1-based indentation level the guide closes (`level * INDENT_WIDTH`).
    pub level: usize,
    /// 0-based column the guide sits on.
    pub column: usize,
}

/// A bounded, caret-aware window over one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorViewport {
    total_lines: usize,
    window_lines: usize,
    first_line: usize,
}

impl Default for EditorViewport {
    fn default() -> Self {
        Self {
            total_lines: 1,
            window_lines: 1,
            first_line: 0,
        }
    }
}

impl EditorViewport {
    /// A window over `total_lines` showing at most `window_lines` lines and
    /// starting at `first_line` (0-based, clamped into the document).
    pub fn new(total_lines: usize, window_lines: usize, first_line: usize) -> Self {
        let total_lines = total_lines.max(1);
        let window_lines = window_lines.max(1);
        Self {
            total_lines,
            window_lines,
            first_line: first_line.min(Self::max_first_line(total_lines, window_lines)),
        }
    }

    /// A window over `total_lines` that starts at the top.
    pub fn top(total_lines: usize, window_lines: usize) -> Self {
        Self::new(total_lines, window_lines, 0)
    }

    /// The window that keeps `caret_line` (1-based) visible with the smallest
    /// possible movement — the same rule on both surfaces, so a caret step
    /// moves the window identically on each end.
    pub fn follow_caret(self, caret_line: usize) -> Self {
        let caret = caret_line.clamp(1, self.total_lines) - 1;
        let first = if caret < self.first_line {
            caret
        } else if caret >= self.first_line + self.rendered_len() {
            caret + 1 - self.rendered_len()
        } else {
            self.first_line
        };
        self.with_first_line(first)
    }

    /// Move the window by a wheel delta in lines (positive scrolls down).
    ///
    /// The clamp happens here, never at render time: a surface mirroring a
    /// text widget's own offset must apply the same accumulation the widget
    /// does, or the two would drift once the widget clamps and this model does
    /// not.
    pub fn scrolled(self, delta: i32) -> Self {
        let first = self.first_line as i64 + i64::from(delta);
        self.with_first_line(first.max(0) as usize)
    }

    /// Re-anchor onto a document whose line count (and possibly window size)
    /// changed, keeping the current top line where it is still valid.
    pub fn with_document(self, total_lines: usize, window_lines: usize) -> Self {
        Self::new(total_lines, window_lines, self.first_line)
    }

    /// Re-anchor onto an explicit top line (clamped).
    pub fn with_first_line(self, first_line: usize) -> Self {
        Self::new(self.total_lines, self.window_lines, first_line)
    }

    /// Highest valid top line: the last window that still ends on the last line.
    pub fn max_first_line(total_lines: usize, window_lines: usize) -> usize {
        total_lines.max(1).saturating_sub(window_lines.max(1))
    }

    pub fn total_lines(&self) -> usize {
        self.total_lines
    }

    pub fn window_lines(&self) -> usize {
        self.window_lines
    }

    /// 0-based first rendered line.
    pub fn first_line(&self) -> usize {
        self.first_line
    }

    /// 0-based last rendered line (inclusive).
    pub fn last_line(&self) -> usize {
        (self.first_line + self.rendered_len() - 1).min(self.total_lines - 1)
    }

    /// Number of lines the window renders.
    pub fn rendered_len(&self) -> usize {
        self.window_lines.min(self.total_lines)
    }

    /// Whether the window covers the whole document (no hidden lines).
    pub fn covers_document(&self) -> bool {
        self.rendered_len() == self.total_lines
    }

    /// 1-based rendered line numbers, in reading order.
    pub fn line_numbers(&self) -> RangeInclusive<usize> {
        self.first_line + 1..=self.last_line() + 1
    }

    /// Lines above the window that are not rendered.
    pub fn hidden_above(&self) -> usize {
        self.first_line
    }

    /// Lines below the window that are not rendered.
    pub fn hidden_below(&self) -> usize {
        self.total_lines - 1 - self.last_line()
    }

    /// Whether a 1-based line number is inside the window.
    pub fn contains_line(&self, line: usize) -> bool {
        line > self.first_line && line <= self.last_line() + 1
    }

    /// Extracts only the lines in the visible window (`first_line..=last_line()`)
    /// without materializing or cloning unrendered lines outside the window.
    ///
    /// Work stops immediately once `last_line()` is reached. Returns tuples
    /// of `(1-based line number, line_str)`.
    pub fn slice_visible_lines<'a>(&self, full_text: &'a str) -> Vec<(usize, &'a str)> {
        let first = self.first_line;
        let last = self.last_line();
        let mut result = Vec::with_capacity(self.rendered_len());
        for (index, line) in full_text.lines().enumerate() {
            if index < first {
                continue;
            }
            if index > last {
                break;
            }
            result.push((index + 1, line));
        }
        result
    }

    /// Search for `query` only within the visible viewport window.
    ///
    /// Work scales with the viewport window lines rather than the full document.
    pub fn search_in_window(&self, full_text: &str, query: &str) -> Vec<WindowSearchResult> {
        if query.is_empty() {
            return Vec::new();
        }
        let visible = self.slice_visible_lines(full_text);
        let mut results = Vec::new();
        for (line_number, line_content) in visible {
            let mut start = 0;
            while let Some(pos) = line_content[start..].find(query) {
                let col = start + pos;
                results.push(WindowSearchResult {
                    line_number,
                    column: col,
                    match_length: query.len(),
                });
                start = col + query.len().max(1);
            }
        }
        results
    }

    /// Measure window operations against a performance budget.
    pub fn measure_performance(
        &self,
        full_text: &str,
        budget: DocumentPerformanceBudget,
    ) -> DocumentPerformanceReceipt {
        let start_slice = Instant::now();
        let _sliced = self.slice_visible_lines(full_text);
        let slice_micros = start_slice.elapsed().as_micros() as u64;

        let start_scroll = Instant::now();
        let _scrolled = self
            .scrolled(10)
            .scrolled(-5)
            .follow_caret(self.first_line + 5);
        let scroll_micros = start_scroll.elapsed().as_micros() as u64;

        let status = if self.rendered_len() > budget.max_render_window_lines {
            DocumentPerformanceStatus::ExceededWindowLimit {
                actual_lines: self.rendered_len(),
                max_lines: budget.max_render_window_lines,
            }
        } else if slice_micros > budget.max_slice_micros {
            DocumentPerformanceStatus::ExceededSliceBudget {
                actual_micros: slice_micros,
                max_micros: budget.max_slice_micros,
            }
        } else if scroll_micros > budget.max_scroll_micros {
            DocumentPerformanceStatus::ExceededScrollBudget {
                actual_micros: scroll_micros,
                max_micros: budget.max_scroll_micros,
            }
        } else {
            DocumentPerformanceStatus::WithinBudget
        };

        DocumentPerformanceReceipt {
            document_lines: self.total_lines,
            window_lines: self.rendered_len(),
            slice_duration_micros: slice_micros,
            scroll_duration_micros: scroll_micros,
            status,
            budget,
        }
    }
}

/// Leading indentation of one line in columns (a tab counts as one step).
pub fn line_indent_columns(line: &str) -> usize {
    let mut columns = 0;
    for character in line.chars() {
        match character {
            ' ' => columns += 1,
            '\t' => columns += INDENT_WIDTH,
            _ => break,
        }
    }
    columns
}

/// Indentation depth of one line in `INDENT_WIDTH` steps.
pub fn line_indent_level(line: &str) -> usize {
    line_indent_columns(line) / INDENT_WIDTH
}

/// The reference lines of one line: one per closed indentation level. Both
/// surfaces render this same list (the Iced gutter as a depth rail, the Bevy
/// row as an in-flow rail) instead of each deriving its own depth.
pub fn indent_guides(line: &str) -> Vec<IndentGuide> {
    let level = line_indent_level(line);
    (1..=level)
        .map(|level| IndentGuide {
            level,
            column: level * INDENT_WIDTH,
        })
        .collect()
}

#[cfg(test)]
#[path = "editor_viewport_test.rs"]
mod editor_viewport_test;
