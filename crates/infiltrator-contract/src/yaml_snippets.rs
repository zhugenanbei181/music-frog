//! DUAL-09-04: the shared YAML snippet catalogue.
//!
//! Both editors insert from this one list: the stable id, the category, the
//! i18n key (Iced) plus the bare-Chinese label (Bevy) and the exact YAML bytes
//! that are spliced at the caret. A surface only decides *where* the caret is
//! and how it edits its own buffer; it never carries its own copy of a snippet.
//!
//! [`insert_at_caret`] is the byte-faithful splice both ends rely on: the
//! document's existing lines are preserved verbatim, the snippet body is
//! inserted at the caret, and a fresh line is started unless the caret already
//! sits on an empty line — so pasting at the end of a line never glues the
//! snippet onto the previous item. Indentation is part of the body: these
//! snippets are list items under `proxies:`/`rules:`.
//!
//! Honest boundary: the catalogue is a fixed, hand-written set (the eight
//! entries the Iced surface used to hardcode); it is not user-editable and it
//! does not generate values (no random UUID/port). A caret outside the
//! document is clamped, never silently rejected.

use std::fmt;

/// Category of a snippet, shared for grouping and for surface copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YamlSnippetKind {
    /// A single proxy node entry (`proxies:` item).
    ProxyNode,
    /// A proxy-group entry (`proxy-groups:` item).
    ProxyGroup,
    /// A routing rule entry (`rules:` item).
    Rule,
}

impl YamlSnippetKind {
    /// Bare-Chinese category label (Bevy convention); Iced renders
    /// [`YamlSnippet::label_key`] instead.
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::ProxyNode => "代理节点",
            Self::ProxyGroup => "策略组",
            Self::Rule => "分流规则",
        }
    }
}

/// One catalogue entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct YamlSnippet {
    /// Stable id every surface passes around (Iced `Message`, Bevy component).
    pub id: &'static str,
    /// Iced locale key for the button copy.
    pub label_key: &'static str,
    /// Bare-Chinese button copy (Bevy convention).
    pub label_zh: &'static str,
    /// Exact YAML text inserted at the caret, indentation included.
    pub body: &'static str,
    pub kind: YamlSnippetKind,
}

impl YamlSnippet {
    /// The text spliced at a caret. A caret that is *not* at the head of an
    /// empty line gets a separating newline first, so the snippet lands as its
    /// own item instead of continuing the current line.
    pub fn insertion_text(&self, at_line_start: bool) -> String {
        if at_line_start {
            self.body.to_string()
        } else {
            format!("\n{}", self.body)
        }
    }
}

/// Every snippet either surface may insert, in stable catalogue order.
pub const YAML_SNIPPETS: &[YamlSnippet] = &[
    YamlSnippet {
        id: "ss",
        label_key: "yaml_snippet_ss",
        label_zh: "+ Shadowsocks",
        body: "  - name: SS-Node\n    type: ss\n    server: server.example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: password\n",
        kind: YamlSnippetKind::ProxyNode,
    },
    YamlSnippet {
        id: "vmess",
        label_key: "yaml_snippet_vmess",
        label_zh: "+ Vmess",
        body: "  - name: Vmess-Node\n    type: vmess\n    server: server.example.com\n    port: 443\n    uuid: a3482e88-7d8f-4a42-9988-1a2b3c4d5e6f\n    alterId: 0\n    cipher: auto\n    tls: true\n",
        kind: YamlSnippetKind::ProxyNode,
    },
    YamlSnippet {
        id: "trojan",
        label_key: "yaml_snippet_trojan",
        label_zh: "+ Trojan",
        body: "  - name: Trojan-Node\n    type: trojan\n    server: server.example.com\n    port: 443\n    password: password\n    sni: example.com\n",
        kind: YamlSnippetKind::ProxyNode,
    },
    YamlSnippet {
        id: "hy2",
        label_key: "yaml_snippet_hy2",
        label_zh: "+ Hy2",
        body: "  - name: Hy2-Node\n    type: hysteria2\n    server: server.example.com\n    port: 443\n    password: password\n    sni: example.com\n",
        kind: YamlSnippetKind::ProxyNode,
    },
    YamlSnippet {
        id: "select",
        label_key: "yaml_snippet_select",
        label_zh: "+ Select",
        body: "  - name: PROXIES\n    type: select\n    proxies:\n      - DIRECT\n",
        kind: YamlSnippetKind::ProxyGroup,
    },
    YamlSnippet {
        id: "url-test",
        label_key: "yaml_snippet_urltest",
        label_zh: "+ URL-Test",
        body: "  - name: AUTO-TEST\n    type: url-test\n    url: http://www.gstatic.com/generate_204\n    interval: 300\n    proxies:\n      - DIRECT\n",
        kind: YamlSnippetKind::ProxyGroup,
    },
    YamlSnippet {
        id: "rule-domain",
        label_key: "yaml_snippet_rule_domain",
        label_zh: "+ DOMAIN",
        body: "  - DOMAIN-SUFFIX,google.com,PROXIES\n",
        kind: YamlSnippetKind::Rule,
    },
    YamlSnippet {
        id: "rule-geoip",
        label_key: "yaml_snippet_rule_geoip",
        label_zh: "+ GEOIP",
        body: "  - GEOIP,CN,DIRECT\n",
        kind: YamlSnippetKind::Rule,
    },
];

/// Look a snippet up by its stable id.
pub fn yaml_snippet(id: &str) -> Option<&'static YamlSnippet> {
    YAML_SNIPPETS.iter().find(|snippet| snippet.id == id)
}

/// The caret context of a spliced snippet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnippetCaret {
    /// 1-based line the caret sits on.
    pub line: usize,
    /// 0-based character column inside that line.
    pub column: usize,
}

/// Outcome of a byte-faithful snippet splice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnippetInsertion {
    pub snippet_id: &'static str,
    /// The document after the splice; every untouched byte is preserved.
    pub content: String,
    /// 1-based caret line after the insertion.
    pub cursor_line: usize,
    /// 0-based caret column after the insertion.
    pub cursor_column: usize,
    /// Shared preflight verdict on the spliced document, set by the
    /// application layer (`Some` only when the splice introduced a syntax
    /// error into a previously valid document).
    pub syntax: Option<crate::profile_document::SyntaxDiagnosticSnapshot>,
}

impl SnippetInsertion {
    pub fn with_syntax(
        mut self,
        syntax: Option<crate::profile_document::SyntaxDiagnosticSnapshot>,
    ) -> Self {
        self.syntax = syntax;
        self
    }

    /// Whether the insertion kept the document parseable (or the document was
    /// already unparseable before the splice).
    pub fn is_clean(&self) -> bool {
        self.syntax.is_none()
    }
}

/// Why a splice could not be produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnippetInsertError {
    /// The id is not in [`YAML_SNIPPETS`] — a programming error, never a
    /// dropped insertion.
    UnknownSnippet(String),
}

/// Byte offset of a character column inside one line, clamped to its end.
///
/// The two text widgets count columns in bytes (Iced's `text_editor` uses the
/// cosmic-text cursor index, Bevy's `CodeEditorState` indexes its `String`),
/// while [`SnippetCaret`] counts characters. These two conversions are the only
/// place that difference is allowed to matter.
pub fn byte_offset_of_column(line: &str, column: usize) -> usize {
    line.char_indices()
        .nth(column)
        .map(|(offset, _)| offset)
        .unwrap_or(line.len())
}

/// Character column of a byte offset inside one line, clamped onto a character
/// boundary.
pub fn column_of_byte_offset(line: &str, offset: usize) -> usize {
    let mut offset = offset.min(line.len());
    while offset > 0 && !line.is_char_boundary(offset) {
        offset -= 1;
    }
    line[..offset].chars().count()
}

impl fmt::Display for SnippetInsertError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSnippet(id) => write!(formatter, "未知配置片段：{id}"),
        }
    }
}

/// Splice `snippet_id`'s body into `content` at `caret`, byte-faithfully.
///
/// The caret is clamped into the document first: a caret past the last line
/// lands on the last line, a column past the line's end lands at its end. The
/// returned caret sits directly after the inserted text, so a surface can keep
/// typing at the natural position.
pub fn insert_at_caret(
    content: &str,
    snippet_id: &str,
    caret: SnippetCaret,
) -> Result<SnippetInsertion, SnippetInsertError> {
    let snippet = yaml_snippet(snippet_id)
        .ok_or_else(|| SnippetInsertError::UnknownSnippet(snippet_id.to_string()))?;
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
    let insertion = snippet.insertion_text(at_line_start);
    let spliced = format!("{before}{insertion}{after}");

    let replacement: Vec<String> = spliced.split('\n').map(str::to_string).collect();
    // `before` and `after` never contain a newline, so every new line in the
    // replacement comes from the insertion itself.
    let inserted_lines = replacement.len() - 1;
    let cursor_line = line + inserted_lines + 1;
    let cursor_column = replaced_caret_column(&replacement, after);

    lines.splice(line..=line, replacement);
    Ok(SnippetInsertion {
        snippet_id: snippet.id,
        content: lines.join("\n"),
        cursor_line,
        cursor_column,
        syntax: None,
    })
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
#[path = "yaml_snippets_test.rs"]
mod yaml_snippets_test;
