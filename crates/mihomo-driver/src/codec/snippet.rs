//! Byte-faithful splicing of an editor snippet body into a YAML document.
//!
//! The splice mechanics are engine-neutral: the snippet *bodies* stay in the
//! product/UI snippet catalogue, which reaches this module as data. A caret is
//! clamped into the document, the untouched bytes are preserved verbatim, and
//! a snippet pasted into a non-empty line starts its own line.

use lurker_core_ports::profile_document_codec::ProfileDocumentCaret;

/// A document after one byte-faithful snippet splice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplicedDocument {
    /// The identifier of the snippet that was spliced.
    pub snippet_id: String,
    /// The document after the splice; every untouched byte is preserved.
    pub content: String,
    /// One-based caret line after the insertion.
    pub cursor_line: usize,
    /// Zero-based caret column after the insertion.
    pub cursor_column: usize,
}

/// Splice `snippet_body` into `content` at `caret`.
///
/// A caret past the last line lands on the last line, a column past the line's
/// end lands at its end. The returned caret sits directly after the inserted
/// text so a surface can keep typing at the natural position.
pub fn splice_at_caret(
    content: &str,
    snippet_id: &str,
    snippet_body: &str,
    caret: ProfileDocumentCaret,
) -> SplicedDocument {
    let mut lines: Vec<String> = content.split('\n').map(str::to_string).collect();
    if lines.is_empty() {
        lines.push(String::new());
    }
    let line = caret.line.clamp(1, lines.len()) - 1;
    let current = lines[line].clone();
    let column = caret.column.min(current.chars().count());
    let offset: usize = current.chars().take(column).map(char::len_utf8).sum();
    let (before, after) = current.split_at(offset);
    let at_line_start = column == 0 && current.trim().is_empty();
    let insertion = if at_line_start {
        snippet_body.to_owned()
    } else {
        format!("\n{snippet_body}")
    };
    let spliced = format!("{before}{insertion}{after}");

    let replacement: Vec<String> = spliced.split('\n').map(str::to_string).collect();
    // `before` and `after` never contain a newline, so every new line in the
    // replacement comes from the insertion itself.
    let inserted_lines = replacement.len() - 1;
    let cursor_line = line + inserted_lines + 1;
    let cursor_column = replaced_caret_column(&replacement, after);

    lines.splice(line..=line, replacement);
    SplicedDocument {
        snippet_id: snippet_id.to_owned(),
        content: lines.join("\n"),
        cursor_line,
        cursor_column,
    }
}

/// The caret column after the splice: the length of the insertion's last line.
/// `after` is the untouched tail of the original line, so subtracting it from
/// the replacement's last line leaves exactly the inserted characters.
fn replaced_caret_column(replacement: &[String], after: &str) -> usize {
    replacement
        .last()
        .map(|line| line.chars().count())
        .unwrap_or_default()
        .saturating_sub(after.chars().count())
}

#[cfg(test)]
#[path = "snippet_tests.rs"]
mod tests;
