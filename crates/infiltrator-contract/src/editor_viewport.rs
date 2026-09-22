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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub fn line_numbers(&self) -> std::ops::RangeInclusive<usize> {
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

    /// Window label for either surface's gutter header, e.g.
    /// `行 41–80 / 共 10000`.
    pub fn range_label(&self) -> String {
        format!(
            "行 {}–{} / 共 {}",
            self.first_line + 1,
            self.last_line() + 1,
            self.total_lines
        )
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
