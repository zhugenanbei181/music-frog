//! DUAL-09-04: the shared YAML snippet catalogue.
//!
//! Both editors insert from this one list: the stable id, the category, the
//! shared localization key and the exact YAML bytes
//! that are spliced at the caret. A surface only decides *where* the caret is
//! and how it edits its own buffer; it never carries its own copy of a snippet.
//!
//! This module owns the catalogue (ids, localization keys and exact bodies).
//! The byte-faithful splice mechanics live in
//! `mihomo_driver::codec::snippet`; the product codec adapter injects this
//! catalogue into that single neutral implementation.
//!
//! Honest boundary: the catalogue is a fixed, hand-written set (the eight
//! entries the Iced surface used to hardcode); it is not user-editable and it
//! does not generate values (no random UUID/port). A caret outside the
//! document is clamped, never silently rejected.

use serde::{Deserialize, Serialize};

/// YAML-specific syntax detail for the Mihomo snippet/editor extension.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxDiagnosticSnapshot {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl SyntaxDiagnosticSnapshot {
    pub fn line_label(&self) -> String {
        format!("Line {} : Col {}", self.line, self.column)
    }
}

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

/// One catalogue entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct YamlSnippet {
    /// Stable id every surface passes around (Iced `Message`, Bevy component).
    pub id: &'static str,
    /// Shared localization key for the button copy.
    pub label_key: &'static str,
    /// Exact YAML text inserted at the caret, indentation included.
    pub body: &'static str,
    pub kind: YamlSnippetKind,
}

/// Every snippet either surface may insert, in stable catalogue order.
pub const YAML_SNIPPETS: &[YamlSnippet] = &[
    YamlSnippet {
        id: "ss",
        label_key: "yaml_snippet_ss",
        body: "  - name: SS-Node\n    type: ss\n    server: server.example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: password\n",
        kind: YamlSnippetKind::ProxyNode,
    },
    YamlSnippet {
        id: "vmess",
        label_key: "yaml_snippet_vmess",
        body: "  - name: Vmess-Node\n    type: vmess\n    server: server.example.com\n    port: 443\n    uuid: a3482e88-7d8f-4a42-9988-1a2b3c4d5e6f\n    alterId: 0\n    cipher: auto\n    tls: true\n",
        kind: YamlSnippetKind::ProxyNode,
    },
    YamlSnippet {
        id: "trojan",
        label_key: "yaml_snippet_trojan",
        body: "  - name: Trojan-Node\n    type: trojan\n    server: server.example.com\n    port: 443\n    password: password\n    sni: example.com\n",
        kind: YamlSnippetKind::ProxyNode,
    },
    YamlSnippet {
        id: "hy2",
        label_key: "yaml_snippet_hy2",
        body: "  - name: Hy2-Node\n    type: hysteria2\n    server: server.example.com\n    port: 443\n    password: password\n    sni: example.com\n",
        kind: YamlSnippetKind::ProxyNode,
    },
    YamlSnippet {
        id: "select",
        label_key: "yaml_snippet_select",
        body: "  - name: PROXIES\n    type: select\n    proxies:\n      - DIRECT\n",
        kind: YamlSnippetKind::ProxyGroup,
    },
    YamlSnippet {
        id: "url-test",
        label_key: "yaml_snippet_urltest",
        body: "  - name: AUTO-TEST\n    type: url-test\n    url: http://www.gstatic.com/generate_204\n    interval: 300\n    proxies:\n      - DIRECT\n",
        kind: YamlSnippetKind::ProxyGroup,
    },
    YamlSnippet {
        id: "rule-domain",
        label_key: "yaml_snippet_rule_domain",
        body: "  - DOMAIN-SUFFIX,google.com,PROXIES\n",
        kind: YamlSnippetKind::Rule,
    },
    YamlSnippet {
        id: "rule-geoip",
        label_key: "yaml_snippet_rule_geoip",
        body: "  - GEOIP,CN,DIRECT\n",
        kind: YamlSnippetKind::Rule,
    },
];

/// Look a snippet up by its stable id.
pub fn yaml_snippet(id: &str) -> Option<&'static YamlSnippet> {
    YAML_SNIPPETS.iter().find(|snippet| snippet.id == id)
}

/// Byte offset of a character column inside one line, clamped to its end.
///
/// The two text widgets count columns in bytes (Iced's `text_editor` uses the
/// cosmic-text cursor index, Bevy's `CodeEditorState` indexes its `String`),
/// while the codec caret (`ProfileDocumentCaret`) counts characters. These two
/// conversions are the only place that difference is allowed to matter.
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

#[cfg(test)]
#[path = "yaml_snippets_test.rs"]
mod yaml_snippets_test;
