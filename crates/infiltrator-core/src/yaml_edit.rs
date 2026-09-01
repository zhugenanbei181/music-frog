//! Byte-faithful, text-level YAML splicing (prototype for the apply-transaction
//! YAML fidelity project; design and migration plan in
//! `docs/YAML_FIDELITY_PLAN.md`).
//!
//! The existing write paths (`mixin::merge_profile_with_config`,
//! `profile_options::strip_rule_lines`, the filter pipeline) round-trip user
//! configs through `serde_yaml_ng`: deserialization has no model for comments
//! or anchors and re-serialization emits a normalized document, so a
//! hand-annotated config loses its comments after the first apply. [`SourceDoc`]
//! is a deliberately smaller tool: it keeps the file as a vector of physical
//! lines (verbatim text + exact line terminator) and performs
//! indentation-aware edits that rewrite only the lines they must touch.
//! Everything else — comments, blank lines, anchors (`&a`/`*a`), key order,
//! quoting, CRLF, BOM — passes through byte-for-byte.
//!
//! Hard limits, all enforced (see [`YamlEditError`]):
//! - single-document YAML rooted in a mapping;
//! - no edits inside `` | ``/`>` block scalars: detected conservatively and
//!   rejected instead of guessed at;
//! - no tab indentation (invalid YAML anyway);
//! - new values are single-line YAML scalar text supplied verbatim by the
//!   caller (no quoting synthesis).
//!
//! No full YAML parse happens here by design: the point is that untouched
//! lines can never drift because they are never re-rendered.

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

/// A parsed-for-splicing YAML document: physical lines plus a BOM flag, nothing
/// more. [`SourceDoc::parse`] + [`SourceDoc::render`] round-trips any input
/// byte-identically; the edit operations below are the only ways to change it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceDoc {
    lines: Vec<Line>,
    bom: bool,
}

impl SourceDoc {
    /// Split `input` into physical lines, preserving each line's exact
    /// terminator and a leading UTF-8 BOM. Rejects documents this module
    /// cannot splice safely (multi-document, tab indentation).
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

    /// Render back to text. Without edits this is byte-identical to the parse
    /// input (BOM, CRLF and the missing trailing newline included).
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

    /// Append `rule` (raw scalar text, anchors/quotes included verbatim) as the
    /// last item of the top-level `rules` sequence. Comments, blank lines and
    /// anchor/reference lines inside the block keep their exact bytes; the new
    /// item is inserted after the last existing item, so trailing block
    /// comments stay after it. A missing `rules` block is created at the end of
    /// the document with the conventional two-space item indent.
    pub fn append_rule(&mut self, rule: &str) -> Result<(), YamlEditError> {
        let rule = rule.trim();
        if rule.is_empty() {
            return Err(YamlEditError::Unsupported("empty rule line".into()));
        }
        if rule.contains(['\n', '\r']) {
            return Err(YamlEditError::Unsupported(
                "rule must be a single line".into(),
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
                text: "rules:".to_string(),
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
            // `rules: [a, b]` or `rules: null`: an inline value cannot receive
            // an appended block item without rewriting the whole line.
            return Err(YamlEditError::FlowSyntax("rules".into()));
        }
        // Walk the block: indented lines belong to the sequence; the first
        // non-blank indent-0 line ends it.
        let mut last_item: Option<usize> = None;
        let mut item_indent = 2;
        let mut end = self.lines.len();
        let mut i = header + 1;
        while i < self.lines.len() {
            let text = &self.lines[i].text;
            if is_blank(text) {
                i += 1;
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
            i += 1;
        }
        // A block scalar anywhere inside the rules block is an edit barrier.
        if let Some(&(s, _)) = spans.iter().find(|&&(s, _)| s > header && s < end) {            return Err(YamlEditError::BlockScalar(s + 1));
        }
        let mut eol = last_item.map(|idx| self.lines[idx].eol).unwrap_or_else(|| self.default_eol());
        if eol == Eol::None {
            eol = self.default_eol();
        }
        let at = last_item.map_or(header + 1, |idx| idx + 1);
        if at == self.lines.len()
            && let Some(last) = self.lines.last_mut()
            && last.eol == Eol::None
        {
            // Appending after a final line without a newline: terminate it
            // first, otherwise it would swallow the inserted item.
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

    /// Remove the first `rules` item whose payload equals `rule` (compared
    /// after stripping the item's trailing `# comment` and optional quotes).
    /// The whole physical line — inline comment included — is removed;
    /// neighbouring lines are not touched. Duplicate rules need repeated calls.
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
        let mut i = header + 1;
        while i < self.lines.len() {
            let text = self.lines[i].text.clone();
            if is_blank(&text) {
                i += 1;
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
            i += 1;
        }
        Err(YamlEditError::RuleNotFound(target.to_string()))
    }

    /// Rewrite the value of a top-level scalar key (`mode: rule` → `mode:
    /// global`) in place: the raw key text, the whitespace between the colon
    /// and the value, a trailing `# comment`, and every other byte of the file
    /// stay exactly as they were. Fails for keys that are missing or that head
    /// a nested block (only scalar overrides are supported).
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
        let (raw_key, rest) = split_key(trimmed)
            .ok_or_else(|| YamlEditError::KeyNotFound(key.to_string()))?;
        if strip_inline_comment(rest).trim().is_empty() && self.heads_block(idx) {
            return Err(YamlEditError::Unsupported(format!(
                "`{key}` heads a nested block; only scalar overrides are supported"
            )));
        }
        let stripped = strip_inline_comment(rest);
        // Preserve the exact bytes around the rewrite: `stripped` is the old
        // value plus the spaces that separated it from `#`, and the comment
        // tail is kept verbatim.
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

    /// Inclusive (header, last-content-line) ranges of every `` | ``/`>` block
    /// scalar in the document. These spans are edit barriers: any operation
    /// whose affected line falls inside one is rejected.
    fn block_scalar_spans(&self) -> Vec<(usize, usize)> {
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

    /// Index of the top-level (indent 0) mapping line whose key is `key`, or
    /// `None`. Comment, blank, sequence-item and nested lines never match.
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

    /// True when `idx` is followed by more-indented content before the next
    /// top-level line, i.e. the key heads a nested block.
    fn heads_block(&self, idx: usize) -> bool {
        self.lines[idx + 1..]
            .iter()
            .take_while(|l| is_blank(&l.text) || indent_of(&l.text) > 0)
            .any(|l| !is_blank(&l.text))
    }

    /// First terminator style found in the document; used for newly created
    /// lines so a CRLF file stays CRLF.
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
                // Directives (`%YAML`, `%TAG`) and the opening `---` are fine.
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

/// A sequence item line: `- value` or a bare `-`.
fn is_item(trimmed: &str) -> bool {
    trimmed == "-" || trimmed.starts_with("- ")
}

/// Split `key: rest` (leading indentation already stripped) into the raw key
/// text and the raw remainder after the colon. Quoted keys are honored; a plain
/// key ends at the first `:` followed by whitespace or end of line (per YAML,
/// `a:b` without a space is the plain scalar `a:b`, not a mapping).
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
                        // `after` is the trimmed post-quote text (": value");
                        // the value part starts after the colon.
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
                *b == b':' && (i + 1 == bytes.len() || bytes[i + 1] == b' ' || bytes[i + 1] == b'\t')
            })
            .map(|(i, _)| (&trimmed[..i], &trimmed[i + 1..])),
    }
}

/// Strip matching outer quotes from a raw key/scalar token.
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

/// Cut a trailing `# comment` (a `#` at word start outside quotes); comments
/// require a preceding space or start of line per YAML.
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

/// True when `rest` (the raw text after `key:` or after `- `) introduces a
/// `` | ``/`>` block scalar, ignoring an optional trailing comment.
fn is_block_scalar_header(rest: &str) -> bool {
    let rest = strip_inline_comment(rest).trim();
    let mut chars = rest.chars();
    match chars.next() {
        Some('|') | Some('>') => chars.all(|c| c.is_ascii_digit() || c == '+' || c == '-'),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(s: &str) -> SourceDoc {
        SourceDoc::parse(s).expect("parse")
    }

    // --- scenario a: append into the rules block ----------------------------

    #[test]
    fn append_rule_keeps_comments_and_anchors_verbatim() {
        let input = "\
# user header comment
mode: rule

rules:
  # ad blocking, added by hand
  - DOMAIN-SUFFIX,ads.example.com,REJECT   # inline note
  - &catchall MATCH,DIRECT
proxies:
  - &hk HK-01
  - *hk
";
        let mut d = doc(input);
        d.append_rule("DOMAIN-SUFFIX,youtube.com,REJECT")
            .expect("append");
        let expected = "\
# user header comment
mode: rule

rules:
  # ad blocking, added by hand
  - DOMAIN-SUFFIX,ads.example.com,REJECT   # inline note
  - &catchall MATCH,DIRECT
  - DOMAIN-SUFFIX,youtube.com,REJECT
proxies:
  - &hk HK-01
  - *hk
";
        assert_eq!(d.render(), expected);
        // The splice output must still be valid YAML with the same meaning.
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&d.render()).unwrap();
        assert_eq!(
            value.get("rules").unwrap().as_sequence().unwrap().len(),
            3
        );
    }

    #[test]
    fn append_rule_creates_block_when_missing() {
        let mut d = doc("mode: rule\nlog-level: info\n");
        d.append_rule("MATCH,DIRECT").expect("append");
        assert_eq!(
            d.render(),
            "mode: rule\nlog-level: info\nrules:\n  - MATCH,DIRECT\n"
        );
    }

    #[test]
    fn append_rule_fills_empty_header_without_trailing_newline() {
        let mut d = doc("port: 7890\nrules:");
        d.append_rule("MATCH,DIRECT").expect("append");
        assert_eq!(d.render(), "port: 7890\nrules:\n  - MATCH,DIRECT\n");
    }

    #[test]
    fn append_rule_inherits_indent_of_existing_items() {
        let mut d = doc("rules:\n    - MATCH,DIRECT\n");
        d.append_rule("DOMAIN,x,PROXY").expect("append");
        assert_eq!(d.render(), "rules:\n    - MATCH,DIRECT\n    - DOMAIN,x,PROXY\n");
    }

    // --- scenario b: remove one rule line -----------------------------------

    #[test]
    fn remove_rule_deletes_only_target_line_with_comment() {
        let input = "\
rules:
  - DOMAIN-SUFFIX,ads.com,REJECT
  - DOMAIN,keep.me,DIRECT   # stay
  - MATCH,DIRECT # drop me
proxies: []
";
        let mut d = doc(input);
        d.remove_rule("MATCH,DIRECT").expect("remove");
        assert_eq!(
            d.render(),
            "rules:\n  - DOMAIN-SUFFIX,ads.com,REJECT\n  - DOMAIN,keep.me,DIRECT   # stay\nproxies: []\n"
        );
    }

    #[test]
    fn remove_rule_errors_when_missing_or_block_absent() {
        let mut d = doc("rules:\n  - MATCH,DIRECT\n");
        assert!(matches!(
            d.remove_rule("DOMAIN,x,REJECT"),
            Err(YamlEditError::RuleNotFound(_))
        ));
        let mut no_rules = doc("mode: rule\n");
        assert!(matches!(
            no_rules.remove_rule("MATCH,DIRECT"),
            Err(YamlEditError::RulesBlockMissing)
        ));
    }

    // --- scenario c: top-level scalar override -------------------------------

    #[test]
    fn set_top_scalar_touches_only_one_line() {
        let input = "\
# top comment
mode: rule   # rule mode
log-level: info
rules:
  - MATCH,DIRECT
";
        let mut d = doc(input);
        d.set_top_scalar("mode", "global").expect("set");
        let out = d.render();
        assert_eq!(
            out,
            "# top comment\nmode: global   # rule mode\nlog-level: info\nrules:\n  - MATCH,DIRECT\n"
        );
        let before: Vec<&str> = input.lines().collect();
        let after: Vec<&str> = out.lines().collect();
        assert_eq!(before.len(), after.len());
        for (i, (b, a)) in before.iter().zip(after.iter()).enumerate() {
            if i != 1 {
                assert_eq!(b, a, "line {i} changed");
            }
        }
    }

    #[test]
    fn set_top_scalar_preserves_inline_comment_and_gap() {
        let mut d = doc("mode:   rule  # gap kept\n");
        d.set_top_scalar("mode", "global").expect("set");
        assert_eq!(d.render(), "mode:   global  # gap kept\n");
    }

    #[test]
    fn set_top_scalar_errors_for_missing_and_block_keys() {
        let mut d = doc("mode: rule\ndns:\n  enable: true\n");
        assert!(matches!(
            d.set_top_scalar("ipv6", "true"),
            Err(YamlEditError::KeyNotFound(_))
        ));
        assert!(matches!(
            d.set_top_scalar("dns", "x"),
            Err(YamlEditError::Unsupported(_))
        ));
        // Unchanged after the failed attempts.
        assert_eq!(d.render(), "mode: rule\ndns:\n  enable: true\n");
    }

    // --- scenario d: boundaries ----------------------------------------------

    #[test]
    fn crlf_documents_stay_crlf() {
        let mut d = doc("mode: rule\r\nrules:\r\n  - MATCH,DIRECT\r\n");
        d.set_top_scalar("mode", "global").expect("set");
        d.append_rule("DOMAIN,x,PROXY").expect("append");
        d.remove_rule("MATCH,DIRECT").expect("remove");
        assert_eq!(d.render(), "mode: global\r\nrules:\r\n  - DOMAIN,x,PROXY\r\n");
    }

    #[test]
    fn bom_is_preserved() {
        let mut d = doc("\u{feff}mode: rule\nrules:\n  - MATCH,DIRECT\n");
        d.append_rule("MATCH,GLOBAL").expect("append");
        assert_eq!(
            d.render(),
            "\u{feff}mode: rule\nrules:\n  - MATCH,DIRECT\n  - MATCH,GLOBAL\n"
        );
    }

    #[test]
    fn blank_lines_and_comments_inside_rules_block_survive() {
        let mut d = doc("rules:\n  - A\n\n  # note between items\n  - B\nproxies: []\n");
        d.append_rule("C").expect("append");
        assert_eq!(
            d.render(),
            "rules:\n  - A\n\n  # note between items\n  - B\n  - C\nproxies: []\n"
        );
    }

    #[test]
    fn parse_render_roundtrip_is_byte_identical() {
        let input = "\u{feff}# c\nmode: rule\r\n\r\nrules:\r\n  - &a A\r\n  - *a\nlast: no-newline";
        assert_eq!(doc(input).render(), input);
    }

    #[test]
    fn block_scalar_content_is_rejected_but_far_edits_allowed() {
        // Folded rule item inside the rules block: refused.
        let mut folded = doc("rules:\n  - >-\n    MATCH,DIRECT\n");
        assert!(matches!(
            folded.append_rule("X,Y,Z"),
            Err(YamlEditError::BlockScalar(_))
        ));
        // The header line of a block scalar is itself a barrier.
        let mut header = doc("desc: |\n  text: kept\n");
        assert!(matches!(
            header.set_top_scalar("desc", "x"),
            Err(YamlEditError::BlockScalar(_))
        ));
        // A block scalar elsewhere in the file does not block distant edits.
        let mut far = doc("desc: |\n  multi\n  line\nmode: rule\n");
        far.set_top_scalar("mode", "global").expect("far edit");
        assert_eq!(
            far.render(),
            "desc: |\n  multi\n  line\nmode: global\n"
        );
    }

    #[test]
    fn unsupported_shapes_are_rejected_up_front() {
        assert!(matches!(
            SourceDoc::parse("a: 1\n---\nb: 2\n"),
            Err(YamlEditError::MultiDocument)
        ));
        assert!(matches!(
            SourceDoc::parse("a:\n\t- x\n"),
            Err(YamlEditError::TabIndentation(2))
        ));
        // Leading `---` alone is a single document and stays acceptable.
        assert!(SourceDoc::parse("---\nmode: rule\n").is_ok());
        // Flow-style rules and top-level sequences are refused, not corrupted.
        let mut flow = doc("rules: [MATCH,DIRECT]\n");
        assert!(matches!(
            flow.append_rule("X,Y,Z"),
            Err(YamlEditError::FlowSyntax(_))
        ));
        let mut seq = doc("- a\n- b\n");
        assert!(matches!(
            seq.append_rule("X,Y,Z"),
            Err(YamlEditError::Unsupported(_))
        ));
    }

    // --- evidence: the serde round-trip this module exists to avoid ----------

    /// Characterization of today's pipeline, cited as the failure example in
    /// `docs/YAML_FIDELITY_PLAN.md` §1. Run with `--nocapture` to print the
    /// re-serialized documents.
    #[test]
    fn characterizes_current_pipeline_fidelity_loss() {
        let base = "\
# 端口与模式（手写注释）
mixed-port: 7890
mode: rule   # rule / global / direct

rules:
  # 手写的兜底规则
  - &catchall MATCH,DIRECT
";
        let mixin = "mode: global\n";

        let merged = crate::mixin::merge_profile_with_mixin(base, mixin).expect("merge");
        println!("--- merge_profile_with_mixin output ---\n{merged}");
        assert!(merged.contains("mode: global"), "semantics still applied");
        assert!(
            !merged.contains('#'),
            "comments dropped by the serde round-trip"
        );
        assert!(!merged.contains("&catchall"), "anchors resolved away");

        let stripped =
            crate::profile_options::strip_rule_lines(base, &["MATCH,DIRECT".to_string()]);
        println!("--- strip_rule_lines output ---\n{stripped}");
        assert!(
            !stripped.contains('#'),
            "strip_rule_lines drops comments too"
        );
    }
}
