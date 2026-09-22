//! DUAL-09-05: AST-preserving YAML formatter over [`SourceDoc`].
//!
//! [`format_yaml`] reshapes the *physical* layout of a document — indentation
//! width, trailing whitespace, blank-line runs, canonical top-level key order
//! and the final newline — while every comment, anchor, alias, quote style and
//! inline note stays in the file. It is the single formatter both surfaces use;
//! the old `serde_yaml_ng` re-serialize (which drops comments and anchors) is
//! gone from the Iced path.
//!
//! Two guarantees keep the promise honest:
//!
//! * documents that use anchors/aliases, merge keys (`<<:`), a root sequence or
//!   a top-level shape the line model cannot classify are never reordered; the
//!   report names the skip reason instead of guessing;
//! * the formatted text is re-parsed and compared with the original as a
//!   canonical (key-order-insensitive) tree. Any difference — a changed scalar,
//!   a moved node, a broken block scalar — aborts with `Err`, so the formatter
//!   can never silently rewrite semantics.

use super::{Line, SourceDoc, YamlEditError, indent_of, is_blank, split_key};
use yaml_rust2::{Yaml, YamlLoader};

/// Canonical Clash/Mihomo top-level key order. Keys outside this table keep
/// their relative order after the known ones; that is the documented layout
/// rule, not a claim that the table is exhaustive.
pub const CLASH_TOP_LEVEL_ORDER: &[&str] = &[
    "port",
    "socks-port",
    "redir-port",
    "tproxy-port",
    "mixed-port",
    "allow-lan",
    "bind-address",
    "mode",
    "log-level",
    "ipv6",
    "external-controller",
    "external-controller-tls",
    "external-controller-cors",
    "external-ui",
    "external-ui-name",
    "external-ui-url",
    "secret",
    "interface-name",
    "routing-mark",
    "tcp-concurrent",
    "unified-delay",
    "find-process-mode",
    "global-client-fingerprint",
    "keep-alive-interval",
    "keep-alive-idle",
    "geodata-mode",
    "geodata-loader",
    "geox-url",
    "geosite-matcher",
    "dns",
    "hosts",
    "sniffer",
    "tun",
    "listeners",
    "authentication",
    "skip-auth-prefixes",
    "profile",
    "proxies",
    "proxy-groups",
    "proxy-providers",
    "rule-providers",
    "rules",
    "sub-rules",
    "script",
    "experimental",
];

/// Why a layout rule was skipped; both surfaces can render the label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatSkipReason {
    /// The document defines or references anchors (`&a` / `*a`).
    AnchorsPresent,
    /// The root node is a sequence, so there is no top-level key order.
    RootSequence,
    /// A merge key (`<<:`) makes reordering unsafe.
    MergeKey,
    /// A top-level line is neither a key nor an item; layout rules stay off.
    UnclassifiedTopLevelLine,
    /// Fewer than two top-level keys; nothing to order.
    NothingToOrder,
    /// The document is empty or whitespace only.
    EmptyDocument,
}

impl FormatSkipReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::AnchorsPresent => "anchors_present",
            Self::RootSequence => "root_sequence",
            Self::MergeKey => "merge_key",
            Self::UnclassifiedTopLevelLine => "unclassified_top_level_line",
            Self::NothingToOrder => "nothing_to_order",
            Self::EmptyDocument => "empty_document",
        }
    }

    /// Whether the skipped rule is worth telling the user about (a layout
    /// choice really was not applied), as opposed to a no-op like
    /// "nothing to order".
    pub const fn is_advisory(self) -> bool {
        matches!(
            self,
            Self::AnchorsPresent
                | Self::RootSequence
                | Self::MergeKey
                | Self::UnclassifiedTopLevelLine
        )
    }

    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::AnchorsPresent => "文档含锚点/别名，已跳过键排序以保持引用顺序",
            Self::RootSequence => "顶层是序列，无键排序可言",
            Self::MergeKey => "文档含合并键 `<<:`，已跳过键排序",
            Self::UnclassifiedTopLevelLine => "顶层存在无法按行模型分类的写法，已跳过键排序",
            Self::NothingToOrder => "顶层少于两个键，无需排序",
            Self::EmptyDocument => "文档为空，未做改动",
        }
    }
}

/// Result of one formatting pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatReport {
    /// Fully rendered formatted document.
    pub content: String,
    /// Whether top-level keys were moved into canonical order.
    pub reordered_top_level_keys: bool,
    /// Layout rules that were deliberately skipped.
    pub notes: Vec<FormatSkipReason>,
}

impl FormatReport {
    /// Whether the pass actually changed bytes.
    pub fn changed(&self, original: &str) -> bool {
        self.content != original
    }

    /// First skip reason, if any.
    pub fn skip_reason(&self) -> Option<FormatSkipReason> {
        self.notes.first().copied()
    }
}

/// Format `content` without losing comments, anchors or scalar styles.
///
/// `Err` means "this document is not format-safe" (unparseable, tab-indented,
/// multi-document, or a layout rule that would change the tree); the caller
/// must surface that instead of silently falling back to a lossy writer.
pub fn format_yaml(content: &str) -> Result<FormatReport, YamlEditError> {
    if content.trim().is_empty() {
        return Ok(FormatReport {
            content: content.to_owned(),
            reordered_top_level_keys: false,
            notes: vec![FormatSkipReason::EmptyDocument],
        });
    }
    let doc = SourceDoc::parse(content)?;
    let spans = doc.block_scalar_spans();
    let mut work = normalize_layout(&doc, &spans);
    let mut notes = Vec::new();
    let reordered = reorder_top_level(&mut work, &spans, &mut notes);
    ensure_trailing_newline(&mut work);
    let rendered = work.render();
    verify_structure_preserved(content, &rendered)?;
    Ok(FormatReport {
        content: rendered,
        reordered_top_level_keys: reordered,
        notes,
    })
}

/// Nesting level of one physical line (callers render `level * 2` spaces).
fn indentation_target(widths: &[usize], width: usize, exact: bool) -> usize {
    match widths.binary_search(&width) {
        Ok(rank) => rank,
        // Comments (and any indentation between two structural levels) snap to
        // the enclosing level instead of inventing a deeper one.
        Err(0) => 0,
        Err(insert) if exact => insert,
        Err(insert) => insert - 1,
    }
}

/// Rewrite indentation, trailing whitespace and blank-line runs.
fn normalize_layout(doc: &SourceDoc, spans: &[(usize, usize)]) -> SourceDoc {
    let in_span = |index: usize| {
        spans
            .iter()
            .any(|&(start, end)| index >= start && index <= end)
    };
    let mut widths: Vec<usize> = doc
        .lines
        .iter()
        .enumerate()
        .filter(|(index, line)| {
            !in_span(*index) && !is_blank(&line.text) && !line.text.trim_start().starts_with('#')
        })
        .map(|(_, line)| indent_of(&line.text))
        .collect();
    widths.sort_unstable();
    widths.dedup();
    let mut lines: Vec<Line> = Vec::with_capacity(doc.lines.len());
    let mut protected: Vec<bool> = Vec::with_capacity(doc.lines.len());
    let mut blank_run = 0usize;
    for (index, line) in doc.lines.iter().enumerate() {
        if in_span(index) {
            lines.push(line.clone());
            protected.push(true);
            blank_run = 0;
            continue;
        }
        let without_trailing = line.text.trim_end_matches([' ', '\t']);
        if without_trailing.is_empty() {
            blank_run += 1;
            // Clash layout keeps at most one separator line between blocks.
            if blank_run > 1 {
                continue;
            }
            lines.push(Line {
                text: String::new(),
                eol: line.eol,
            });
            protected.push(false);
            continue;
        }
        blank_run = 0;
        let body = without_trailing.trim_start_matches(' ');
        let width = without_trailing.len() - body.len();
        let level = indentation_target(&widths, width, !body.starts_with('#'));
        let mut text = String::with_capacity(level * 2 + body.len());
        for _ in 0..level * 2 {
            text.push(' ');
        }
        text.push_str(body);
        lines.push(Line {
            text,
            eol: line.eol,
        });
        protected.push(false);
    }
    // A canonical document has no trailing empty lines (block-scalar content
    // is protected: `|+` keeps its trailing blank lines as data).
    while lines.len() > 1 && is_blank(&lines[lines.len() - 1].text) && !protected[lines.len() - 1] {
        lines.pop();
        protected.pop();
    }
    SourceDoc {
        lines,
        bom: doc.bom,
    }
}

/// Ensure the document ends with exactly one terminator.
fn ensure_trailing_newline(doc: &mut SourceDoc) {
    let eol = doc.default_eol();
    if let Some(last) = doc.lines.last_mut()
        && last.eol == super::Eol::None
    {
        last.eol = eol;
    }
}

/// Move top-level key blocks into [`CLASH_TOP_LEVEL_ORDER`].
///
/// Returns whether anything moved; skip reasons are pushed into `notes`.
fn reorder_top_level(
    doc: &mut SourceDoc,
    spans: &[(usize, usize)],
    notes: &mut Vec<FormatSkipReason>,
) -> bool {
    let in_span = |index: usize| {
        spans
            .iter()
            .any(|&(start, end)| index >= start && index <= end)
    };
    if !doc.scan_anchors_and_aliases().is_empty() {
        notes.push(FormatSkipReason::AnchorsPresent);
        return false;
    }
    if doc
        .lines
        .iter()
        .any(|line| line.text.trim_start().starts_with("<<:"))
        || doc.root_is_sequence()
    {
        notes.push(if doc.root_is_sequence() {
            FormatSkipReason::RootSequence
        } else {
            FormatSkipReason::MergeKey
        });
        return false;
    }

    let mut starts: Vec<usize> = Vec::new();
    for (index, line) in doc.lines.iter().enumerate() {
        if in_span(index) || is_blank(&line.text) || indent_of(&line.text) != 0 {
            continue;
        }
        let trimmed = line.text.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('-') || split_key(trimmed).is_none() {
            notes.push(FormatSkipReason::UnclassifiedTopLevelLine);
            return false;
        }
        starts.push(index);
    }
    if starts.len() < 2 {
        notes.push(FormatSkipReason::NothingToOrder);
        return false;
    }

    // A contiguous comment block directly above a key documents that key, so it
    // travels with it. The preamble (header comments/`---`) never moves.
    let mut segment_start = starts.clone();
    for position in 1..starts.len() {
        let mut start = starts[position];
        while start > starts[position - 1] + 1
            && doc.lines[start - 1].text.trim_start().starts_with('#')
            && indent_of(&doc.lines[start - 1].text) == 0
        {
            start -= 1;
        }
        segment_start[position] = start;
    }

    let key_of = |line_index: usize| -> String {
        split_key(doc.lines[line_index].text.trim_start())
            .map(|(key, _)| key.trim().to_string())
            .unwrap_or_default()
    };
    let rank_of = |key: &str| -> usize {
        CLASH_TOP_LEVEL_ORDER
            .iter()
            .position(|known| *known == key)
            .unwrap_or(usize::MAX)
    };
    let mut order: Vec<usize> = (0..starts.len()).collect();
    order.sort_by_key(|&position| (rank_of(&key_of(starts[position])), position));
    if order
        .iter()
        .enumerate()
        .all(|(slot, &position)| slot == position)
    {
        notes.push(FormatSkipReason::NothingToOrder);
        return false;
    }

    let mut lines: Vec<Line> = doc.lines[..segment_start[0]].to_vec();
    for &position in &order {
        let from = segment_start[position];
        // Segment bounds are the next segment start in physical order; the
        // last segment runs to the end of the file.
        let end = segment_start
            .iter()
            .copied()
            .filter(|start| *start > from)
            .min()
            .unwrap_or(doc.lines.len());
        lines.extend_from_slice(&doc.lines[from..end]);
    }
    doc.lines = lines;
    true
}

/// Canonical, key-order-insensitive signature of a parsed document.
fn structure_signature(content: &str) -> Option<String> {
    let documents = YamlLoader::load_from_str(content).ok()?;
    let mut out = String::new();
    for document in &documents {
        canonical_node(document, &mut out);
        out.push('\n');
    }
    Some(out)
}

fn canonical_node(node: &Yaml, out: &mut String) {
    match node {
        Yaml::Hash(map) => {
            let mut entries: Vec<(String, &Yaml)> = map
                .iter()
                .map(|(key, value)| (canonical_scalar(key), value))
                .collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            out.push('{');
            for (key, value) in entries {
                out.push_str(&key);
                out.push(':');
                canonical_node(value, out);
                out.push(',');
            }
            out.push('}');
        }
        Yaml::Array(items) => {
            out.push('[');
            for item in items {
                canonical_node(item, out);
                out.push(',');
            }
            out.push(']');
        }
        other => out.push_str(&canonical_scalar(other)),
    }
}

fn canonical_scalar(node: &Yaml) -> String {
    match node {
        Yaml::Real(value) => format!("real:{value}"),
        Yaml::Integer(value) => format!("int:{value}"),
        Yaml::String(value) => format!("str:{value:?}"),
        Yaml::Boolean(value) => format!("bool:{value}"),
        Yaml::Null => "null".to_string(),
        Yaml::Alias(index) => format!("alias:{index}"),
        Yaml::BadValue => "bad".to_string(),
        Yaml::Array(_) | Yaml::Hash(_) => {
            let mut nested = String::new();
            canonical_node(node, &mut nested);
            nested
        }
    }
}

/// Refuse any layout that changed the parsed tree.
fn verify_structure_preserved(original: &str, formatted: &str) -> Result<(), YamlEditError> {
    let before = structure_signature(original).ok_or_else(|| {
        YamlEditError::Unsupported(
            "文档无法解析（语法错误），格式化拒绝执行，请先修复语法".to_string(),
        )
    })?;
    let after = structure_signature(formatted).ok_or_else(|| {
        YamlEditError::Unsupported("格式化结果无法解析，已拒绝并保留原文".to_string())
    })?;
    if before != after {
        return Err(YamlEditError::Unsupported(
            "格式化会改变文档结构，已拒绝并保留原文".to_string(),
        ));
    }
    Ok(())
}
