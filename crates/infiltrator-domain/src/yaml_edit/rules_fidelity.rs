//! DUAL-09-01 / LEFT-05 L1: byte-faithful rule-list writes on [`SourceDoc`].
//!
//! [`apply_rule_list`] rewrites only the physical lines that hold rule items.
//! Every comment, blank line, anchor, key order, indentation and line ending
//! outside those item lines passes through verbatim, so saving an edited rule
//! list no longer washes the user's annotations away.
//!
//! The spliced result is verified against the same serde reader the structural
//! writer uses before it is committed. An edit the text layer cannot express
//! therefore falls back to the structural writer; it can never produce a rule
//! list that differs from the requested one.

use super::{SourceDoc, YamlEditError};
use crate::rules::{RuleEntry, format_rule_entry, parse_rule_entry};
use std::collections::{HashMap, VecDeque};

/// Replace the document's `rules` sequence with `rules`.
///
/// The callers keep their structural fallback: `Err` means "this shape is not
/// splice-safe", not "the edit is invalid".
pub fn apply_rule_list(doc: &mut SourceDoc, rules: &[RuleEntry]) -> Result<(), YamlEditError> {
    if rules.is_empty() {
        return Err(YamlEditError::Unsupported(
            "emptying the rules list removes the key; use the structural writer".into(),
        ));
    }
    for entry in rules {
        if entry.rule.trim().is_empty() {
            return Err(YamlEditError::Unsupported("empty rule entry".into()));
        }
    }

    let (header, slots) = read_rule_slots(doc)?;
    let old: Vec<RuleEntry> = slots.iter().map(|slot| slot.entry.clone()).collect();
    if old == rules {
        return Ok(());
    }
    let matched = match_order_preserving(&old, rules);
    let indent = slots.first().map(|slot| slot.indent).unwrap_or(2);

    let mut new_texts: Vec<String> = Vec::with_capacity(rules.len());
    for (index, entry) in rules.iter().enumerate() {
        match matched[index] {
            Some(old_index) if old[old_index].enabled == entry.enabled => {
                new_texts.push(slots[old_index].raw.clone());
            }
            Some(old_index) => {
                new_texts.push(retarget_item_line(&slots[old_index].raw, entry));
            }
            None => new_texts.push(render_item_line(entry, indent)),
        }
    }

    let mut candidate = doc.clone();
    match header {
        Some(header) => splice_into_block(&mut candidate, header, &slots, &new_texts)?,
        None => append_new_block(&mut candidate, &new_texts)?,
    }

    let rendered = candidate.render();
    match crate::rules::load_rules_from_yaml(&rendered) {
        Ok(parsed) if parsed == rules => {
            *doc = candidate;
            Ok(())
        }
        _ => Err(YamlEditError::Unsupported(
            "rule-list splice failed the structural verification; use the structural writer".into(),
        )),
    }
}

/// One physical `rules` item line.
struct RuleSlot {
    line_idx: usize,
    raw: String,
    indent: usize,
    entry: RuleEntry,
}

/// Read the existing `rules` header and its item lines without normalizing.
fn read_rule_slots(doc: &SourceDoc) -> Result<(Option<usize>, Vec<RuleSlot>), YamlEditError> {
    let Some(header) = doc.find_top_level_key("rules") else {
        return Ok((None, Vec::new()));
    };
    let spans = doc.block_scalar_spans();
    if let Some(error) = doc.block_scalar_error(&spans, header) {
        return Err(error);
    }
    let rest = split_key(doc.lines[header].text.trim_start())
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    if is_block_scalar_header(rest) {
        return Err(YamlEditError::BlockScalar(header + 1));
    }
    if !strip_inline_comment(rest).trim().is_empty() {
        return Err(YamlEditError::FlowSyntax("rules".into()));
    }

    let mut slots = Vec::new();
    for index in (header + 1)..doc.lines.len() {
        let text = &doc.lines[index].text;
        if is_blank(text) {
            continue;
        }
        let indent = indent_of(text);
        if indent == 0 {
            break;
        }
        let trimmed = text.trim_start();
        let Some(after) = trimmed.strip_prefix("- ") else {
            if trimmed == "-" {
                return Err(YamlEditError::Unsupported(
                    "bare `-` item (empty or nested) is not splice-safe".into(),
                ));
            }
            continue;
        };
        if after.trim().is_empty() {
            return Err(YamlEditError::Unsupported(
                "empty rule item is not splice-safe".into(),
            ));
        }
        if let Some(error) = doc.block_scalar_error(&spans, index) {
            return Err(error);
        }
        let value = strip_inline_comment(after).trim_end();
        slots.push(RuleSlot {
            line_idx: index,
            raw: text.clone(),
            indent,
            entry: parse_rule_entry(unquote(value)),
        });
    }
    Ok((Some(header), slots))
}

/// Order-preserving greedy match of new entries onto old identities.
///
/// Matching on the rule text (not the enabled flag) lets a toggle reuse its
/// original physical line, keeping the inline comment on that line.
fn match_order_preserving(old: &[RuleEntry], new: &[RuleEntry]) -> Vec<Option<usize>> {
    let mut available: HashMap<&str, VecDeque<usize>> = HashMap::new();
    for (index, entry) in old.iter().enumerate() {
        available
            .entry(entry.rule.as_str())
            .or_default()
            .push_back(index);
    }
    let mut matched = vec![None; new.len()];
    let mut cursor = 0usize;
    for (index, entry) in new.iter().enumerate() {
        let Some(queue) = available.get_mut(entry.rule.as_str()) else {
            continue;
        };
        let Some(position) = queue.iter().position(|&old_index| old_index >= cursor) else {
            continue;
        };
        if let Some(old_index) = queue.remove(position) {
            matched[index] = Some(old_index);
            cursor = old_index + 1;
        }
    }
    matched
}

/// Write the new item lines into the existing block, keeping non-item lines.
fn splice_into_block(
    doc: &mut SourceDoc,
    header: usize,
    slots: &[RuleSlot],
    new_texts: &[String],
) -> Result<(), YamlEditError> {
    if slots.is_empty() {
        for (index, text) in new_texts.iter().enumerate() {
            doc.lines.insert(
                header + 1 + index,
                super::Line {
                    text: text.clone(),
                    eol: doc.default_eol(),
                },
            );
        }
        return Ok(());
    }

    for (index, text) in new_texts.iter().enumerate() {
        if let Some(slot) = slots.get(index) {
            doc.lines[slot.line_idx].text = text.clone();
        }
    }

    if new_texts.len() > slots.len() {
        let anchor = slots.last().map(|slot| slot.line_idx).unwrap_or(header);
        let eol = slots
            .last()
            .map(|slot| doc.lines[slot.line_idx].eol)
            .unwrap_or_else(|| doc.default_eol());
        for (offset, text) in new_texts[slots.len()..].iter().enumerate() {
            doc.lines.insert(
                anchor + 1 + offset,
                super::Line {
                    text: text.clone(),
                    eol,
                },
            );
        }
    } else if new_texts.len() < slots.len() {
        for slot in slots[new_texts.len()..].iter().rev() {
            doc.lines.remove(slot.line_idx);
        }
    }
    Ok(())
}

/// Create a `rules:` block at the end of a document that has none.
fn append_new_block(doc: &mut SourceDoc, new_texts: &[String]) -> Result<(), YamlEditError> {
    if doc.root_is_sequence() {
        return Err(YamlEditError::Unsupported(
            "top-level sequence document has no mapping to host `rules`".into(),
        ));
    }
    let eol = doc.default_eol();
    if let Some(last) = doc.lines.last_mut()
        && last.eol == super::Eol::None
    {
        last.eol = eol;
    }
    doc.lines.push(super::Line {
        text: "rules:".into(),
        eol,
    });
    for text in new_texts {
        doc.lines.push(super::Line {
            text: text.clone(),
            eol,
        });
    }
    Ok(())
}

/// Format a brand-new item line, quoting payloads YAML would misread.
fn render_item_line(entry: &RuleEntry, indent: usize) -> String {
    let value = format_rule_entry(entry);
    let payload = if value.starts_with('#') || value.starts_with(['\'', '"']) {
        format!("'{}'", value.replace('\'', "''"))
    } else {
        value
    };
    format!("{}- {payload}", " ".repeat(indent))
}

/// Rewrite only the payload of an existing item line, keeping its comment.
fn retarget_item_line(raw: &str, entry: &RuleEntry) -> String {
    let indent = indent_of(raw);
    let body = raw.trim_start();
    let after = body.strip_prefix("- ").unwrap_or(body);
    let comment = trailing_comment(after);
    let value = format_rule_entry(entry);
    let payload = if value.starts_with('#') || value.starts_with(['\'', '"']) {
        format!("'{}'", value.replace('\'', "''"))
    } else {
        value
    };
    format!("{}- {payload}{comment}", " ".repeat(indent))
}

/// The inline-comment suffix (including its leading whitespace) of an item.
fn trailing_comment(after: &str) -> &str {
    let bytes = after.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if in_double => index += 1,
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'#' if !in_single
                && !in_double
                && (index == 0 || bytes[index - 1].is_ascii_whitespace()) =>
            {
                let start = after[..index].trim_end().len();
                return &after[start..];
            }
            _ => {}
        }
        index += 1;
    }
    ""
}

// The line-level helpers below mirror the parent module's private helpers.
// Re-implemented locally so this module stays independent of their privacy.

fn is_blank(text: &str) -> bool {
    text.trim().is_empty()
}

fn indent_of(text: &str) -> usize {
    text.bytes().take_while(|byte| *byte == b' ').count()
}

fn split_key(trimmed: &str) -> Option<(&str, &str)> {
    let bytes = trimmed.as_bytes();
    match bytes.first()? {
        b'"' | b'\'' => {
            let quote = bytes[0];
            let mut index = 1;
            while index < bytes.len() {
                match bytes[index] {
                    b'\\' if quote == b'"' => index += 1,
                    byte if byte == quote => {
                        let after = trimmed[index + 1..].trim_start();
                        return after
                            .strip_prefix(':')
                            .map(|value| (&trimmed[..index + 1], value));
                    }
                    _ => {}
                }
                index += 1;
            }
            None
        }
        _ => bytes
            .iter()
            .enumerate()
            .find(|&(index, byte)| {
                *byte == b':'
                    && (index + 1 == bytes.len()
                        || bytes[index + 1] == b' '
                        || bytes[index + 1] == b'\t')
            })
            .map(|(index, _)| (&trimmed[..index], &trimmed[index + 1..])),
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

fn strip_inline_comment(text: &str) -> &str {
    let bytes = text.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if in_double => index += 1,
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'#' if !in_single
                && !in_double
                && (index == 0 || bytes[index - 1].is_ascii_whitespace()) =>
            {
                return &text[..index];
            }
            _ => {}
        }
        index += 1;
    }
    text
}

fn is_block_scalar_header(rest: &str) -> bool {
    let rest = strip_inline_comment(rest).trim();
    let mut chars = rest.chars();
    match chars.next() {
        Some('|') | Some('>') => chars.all(|c| c.is_ascii_digit() || c == '+' || c == '-'),
        _ => false,
    }
}
