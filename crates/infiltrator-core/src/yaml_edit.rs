//! Byte-faithful, text-level YAML splicing and L3 anchor/alias rewriting.
//!
//! [`SourceDoc`] keeps the file as a vector of physical lines (verbatim text +
//! exact line terminator) and performs indentation-aware edits that rewrite only
//! the lines and tokens they must touch. Everything else — comments, blank lines,
//! anchors (`&a`/`*a`), key order, quoting, CRLF, BOM — passes through byte-for-byte.

pub mod anchor;
pub mod mixin_fidelity;

use thiserror::Error;

/// Physical line terminator of one source line, preserved exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eol {
    Lf,
    Crlf,
    /// Last line of a file without a trailing newline.
    None,
}

impl Eol {
    fn as_str(self) -> &'static str {
        match self {
            Eol::Lf => "\n",
            Eol::Crlf => "\r\n",
            Eol::None => "",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Line {
    text: String,
    eol: Eol,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum YamlEditError {
    #[error("multi-document YAML (a second `---` marker) is not supported")]
    MultiDocument,
    #[error("tab in indentation on line {0}: invalid YAML and not splice-safe")]
    TabIndentation(usize),
    #[error("refusing to edit inside a `|`/`>` block scalar (line {0})")]
    BlockScalar(usize),
    #[error("top-level key `{0}` not found")]
    KeyNotFound(String),
    #[error("top-level key `{0}` uses flow/inline syntax; text splice not supported")]
    FlowSyntax(String),
    #[error("`rules` block not found")]
    RulesBlockMissing,
    #[error("rule line not found in `rules` block: {0}")]
    RuleNotFound(String),
    #[error("unsupported edit: {0}")]
    Unsupported(String),
}

/// A parsed-for-splicing YAML document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceDoc {
    lines: Vec<Line>,
    bom: bool,
}

impl SourceDoc {
    /// Split `input` into physical lines, preserving exact terminators and BOM.
    pub fn parse(input: &str) -> Result<Self, YamlEditError> {
        let (bom, rest) = match input.strip_prefix('\u{feff}') {
            Some(rest) => (true, rest),
            None => (false, input),
        };
        let mut lines: Vec<Line> = Vec::new();
        let mut current = String::new();
        for ch in rest.chars() {
            if ch == '\n' {
                let eol = if current.ends_with('\r') {
                    current.pop();
                    Eol::Crlf
                } else {
                    Eol::Lf
                };
                lines.push(Line {
                    text: std::mem::take(&mut current),
                    eol,
                });
            } else {
                current.push(ch);
            }
        }
        if !current.is_empty() {
            lines.push(Line {
                text: current,
                eol: Eol::None,
            });
        }
        let doc = Self { lines, bom };
        doc.check_no_tabs()?;
        doc.check_single_document()?;
        Ok(doc)
    }

    /// Render back to text byte-identically.
    pub fn render(&self) -> String {
        let mut out = String::new();
        if self.bom {
            out.push('\u{feff}');
        }
        for line in &self.lines {
            out.push_str(&line.text);
            out.push_str(line.eol.as_str());
        }
        out
    }

    /// Append `rule` as the last item of the top-level `rules` sequence.
    pub fn append_rule(&mut self, rule: &str) -> Result<(), YamlEditError> {
        let rule = rule.trim();
        if rule.is_empty() || rule.contains(['\n', '\r']) {
            return Err(YamlEditError::Unsupported(
                "rule must be a non-empty single line".into(),
            ));
        }
        let spans = self.block_scalar_spans();
        let Some(header) = self.find_top_level_key("rules") else {
            if self.root_is_sequence() {
                return Err(YamlEditError::Unsupported(
                    "top-level sequence document has no mapping to host `rules`".into(),
                ));
            }
            let eol = self.default_eol();
            if let Some(last) = self.lines.last_mut()
                && last.eol == Eol::None
            {
                last.eol = eol;
            }
            self.lines.push(Line {
                text: "rules:".into(),
                eol,
            });
            self.lines.push(Line {
                text: format!("  - {rule}"),
                eol,
            });
            return Ok(());
        };
        if let Some(err) = self.block_scalar_error(&spans, header) {
            return Err(err);
        }
        let rest = split_key(self.lines[header].text.trim_start())
            .map(|(_, rest)| rest)
            .unwrap_or_default();
        if is_block_scalar_header(rest) {
            return Err(YamlEditError::BlockScalar(header + 1));
        }
        if !strip_inline_comment(rest).trim().is_empty() {
            return Err(YamlEditError::FlowSyntax("rules".into()));
        }
        let mut last_item = None;
        let mut item_indent = 2;
        let mut end = self.lines.len();
        for i in (header + 1)..self.lines.len() {
            let text = &self.lines[i].text;
            if is_blank(text) {
                continue;
            }
            if indent_of(text) == 0 {
                end = i;
                break;
            }
            let trimmed = text.trim_start();
            if is_item(trimmed) {
                last_item = Some(i);
                item_indent = indent_of(text);
            }
        }
        if let Some(&(s, _)) = spans.iter().find(|&&(s, _)| s > header && s < end) {
            return Err(YamlEditError::BlockScalar(s + 1));
        }
        let mut eol = last_item
            .map(|idx| self.lines[idx].eol)
            .unwrap_or_else(|| self.default_eol());
        if eol == Eol::None {
            eol = self.default_eol();
        }
        let at = last_item.map_or(header + 1, |idx| idx + 1);
        if at == self.lines.len()
            && let Some(last) = self.lines.last_mut()
            && last.eol == Eol::None
        {
            last.eol = eol;
        }
        self.lines.insert(
            at,
            Line {
                text: format!("{}- {rule}", " ".repeat(item_indent)),
                eol,
            },
        );
        Ok(())
    }

    /// Remove the first `rules` item whose payload equals `rule`.
    pub fn remove_rule(&mut self, rule: &str) -> Result<(), YamlEditError> {
        let target = rule.trim();
        if target.is_empty() {
            return Err(YamlEditError::Unsupported("empty rule line".into()));
        }
        let Some(header) = self.find_top_level_key("rules") else {
            return Err(YamlEditError::RulesBlockMissing);
        };
        let spans = self.block_scalar_spans();
        if let Some(err) = self.block_scalar_error(&spans, header) {
            return Err(err);
        }
        for i in (header + 1)..self.lines.len() {
            let text = self.lines[i].text.clone();
            if is_blank(&text) {
                continue;
            }
            if indent_of(&text) == 0 {
                break;
            }
            let trimmed = text.trim_start();
            if is_item(trimmed)
                && let Some(after) = trimmed.strip_prefix("- ")
                && unquote(strip_inline_comment(after).trim()) == target
            {
                if let Some(err) = self.block_scalar_error(&spans, i) {
                    return Err(err);
                }
                self.lines.remove(i);
                return Ok(());
            }
        }
        Err(YamlEditError::RuleNotFound(target.to_string()))
    }

    /// Rewrite the value of a top-level scalar key (`mode: rule` → `mode: global`).
    pub fn set_top_scalar(&mut self, key: &str, value: &str) -> Result<(), YamlEditError> {
        let Some(idx) = self.find_top_level_key(key) else {
            return Err(YamlEditError::KeyNotFound(key.to_string()));
        };
        let spans = self.block_scalar_spans();
        if let Some(err) = self.block_scalar_error(&spans, idx) {
            return Err(err);
        }
        let value = value.trim();
        if value.contains(['\n', '\r']) {
            return Err(YamlEditError::Unsupported(
                "value must be a single line".into(),
            ));
        }
        for (i, b) in value.bytes().enumerate() {
            if b == b'#' && (i == 0 || value.as_bytes()[i - 1].is_ascii_whitespace()) {
                return Err(YamlEditError::Unsupported(
                    "value contains a `#` comment starter; pass the quoted scalar yourself".into(),
                ));
            }
        }
        let line_text = self.lines[idx].text.clone();
        let trimmed = line_text.trim_start();
        let (raw_key, rest) =
            split_key(trimmed).ok_or_else(|| YamlEditError::KeyNotFound(key.to_string()))?;
        if strip_inline_comment(rest).trim().is_empty() && self.heads_block(idx) {
            return Err(YamlEditError::Unsupported(format!(
                "`{key}` heads a nested block; only scalar overrides are supported"
            )));
        }
        let stripped = strip_inline_comment(rest);
        let value_end = stripped.trim_end().len();
        let separator = &stripped[value_end..];
        let comment = &rest[stripped.len()..];
        let ws_len = rest.len() - rest.trim_start().len();
        let gap = if value.is_empty() {
            ""
        } else if ws_len == 0 {
            " "
        } else {
            &rest[..ws_len]
        };
        let mut new_text = String::with_capacity(line_text.len() + value.len());
        new_text.push_str(raw_key);
        new_text.push(':');
        new_text.push_str(gap);
        new_text.push_str(value);
        new_text.push_str(separator);
        new_text.push_str(comment);
        self.lines[idx].text = new_text;
        Ok(())
    }

    // ---- internals ---------------------------------------------------------

    pub(crate) fn block_scalar_spans(&self) -> Vec<(usize, usize)> {
        let mut spans = Vec::new();
        for i in 0..self.lines.len() {
            let text = &self.lines[i].text;
            if is_blank(text) {
                continue;
            }
            let trimmed = text.trim_start();
            if trimmed.starts_with('#') {
                continue;
            }
            let rest = match split_key(trimmed) {
                Some((_, rest)) => rest,
                None => match trimmed.strip_prefix("- ") {
                    Some(after) => after,
                    None => continue,
                },
            };
            if !is_block_scalar_header(rest) {
                continue;
            }
            let indent = indent_of(text);
            let mut end = i;
            let mut j = i + 1;
            while j < self.lines.len() {
                let t = &self.lines[j].text;
                if is_blank(t) || indent_of(t) > indent {
                    end = j;
                    j += 1;
                } else {
                    break;
                }
            }
            spans.push((i, end));
        }
        spans
    }

    fn block_scalar_error(&self, spans: &[(usize, usize)], idx: usize) -> Option<YamlEditError> {
        spans
            .iter()
            .find(|&&(s, e)| idx >= s && idx <= e)
            .map(|&(s, _)| YamlEditError::BlockScalar(s + 1))
    }

    fn find_top_level_key(&self, key: &str) -> Option<usize> {
        self.lines.iter().position(|line| {
            let text = &line.text;
            if indent_of(text) != 0 {
                return false;
            }
            let trimmed = text.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') || is_item(trimmed) {
                return false;
            }
            split_key(trimmed).is_some_and(|(raw, _)| unquote(raw.trim_end()) == key)
        })
    }

    fn heads_block(&self, idx: usize) -> bool {
        self.lines[idx + 1..]
            .iter()
            .take_while(|l| is_blank(&l.text) || indent_of(&l.text) > 0)
            .any(|l| !is_blank(&l.text))
    }

    fn default_eol(&self) -> Eol {
        self.lines
            .iter()
            .map(|l| l.eol)
            .find(|&eol| eol != Eol::None)
            .unwrap_or(Eol::Lf)
    }

    fn root_is_sequence(&self) -> bool {
        self.lines.iter().any(|l| {
            let t = l.text.trim_start();
            indent_of(&l.text) == 0 && is_item(t)
        })
    }

    fn check_no_tabs(&self) -> Result<(), YamlEditError> {
        for (idx, line) in self.lines.iter().enumerate() {
            if line.text.trim().is_empty() {
                continue;
            }
            let ws: String = line
                .text
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            if ws.contains('\t') {
                return Err(YamlEditError::TabIndentation(idx + 1));
            }
        }
        Ok(())
    }

    fn check_single_document(&self) -> Result<(), YamlEditError> {
        let mut past_header = false;
        for line in &self.lines {
            let trimmed = line.text.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !past_header {
                if trimmed == "---" || trimmed.starts_with("--- ") || trimmed.starts_with('%') {
                    continue;
                }
                past_header = true;
                continue;
            }
            if trimmed == "---" || trimmed.starts_with("--- ") {
                return Err(YamlEditError::MultiDocument);
            }
        }
        Ok(())
    }
}

// ---- line-level helpers ----------------------------------------------------

fn is_blank(text: &str) -> bool {
    text.trim().is_empty()
}

fn indent_of(text: &str) -> usize {
    text.bytes().take_while(|b| *b == b' ').count()
}

fn is_item(trimmed: &str) -> bool {
    trimmed == "-" || trimmed.starts_with("- ")
}

fn split_key(trimmed: &str) -> Option<(&str, &str)> {
    let bytes = trimmed.as_bytes();
    match bytes.first()? {
        b'"' | b'\'' => {
            let quote = bytes[0];
            let mut i = 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' if quote == b'"' => i += 1,
                    b if b == quote => {
                        let after = trimmed[i + 1..].trim_start();
                        return after
                            .strip_prefix(':')
                            .map(|value| (&trimmed[..i + 1], value));
                    }
                    _ => {}
                }
                i += 1;
            }
            None
        }
        _ => bytes
            .iter()
            .enumerate()
            .find(|&(i, b)| {
                *b == b':'
                    && (i + 1 == bytes.len() || bytes[i + 1] == b' ' || bytes[i + 1] == b'\t')
            })
            .map(|(i, _)| (&trimmed[..i], &trimmed[i + 1..])),
    }
}

fn unquote(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &raw[1..raw.len() - 1];
        }
    }
    raw
}

fn strip_inline_comment(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if in_double => i += 1,
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'#' if !in_single && !in_double && (i == 0 || bytes[i - 1].is_ascii_whitespace()) => {
                return &s[..i];
            }
            _ => {}
        }
        i += 1;
    }
    s
}

fn is_block_scalar_header(rest: &str) -> bool {
    let rest = strip_inline_comment(rest).trim();
    let mut chars = rest.chars();
    match chars.next() {
        Some('|') | Some('>') => chars.all(|c| c.is_ascii_digit() || c == '+' || c == '-'),
        _ => false,
    }
}

#[cfg(test)]
#[path = "yaml_edit_test.rs"]
mod yaml_edit_test;
