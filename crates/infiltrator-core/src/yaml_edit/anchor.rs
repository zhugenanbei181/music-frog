//! L3 Anchor and Alias scanning and namespace rewriting.

use std::collections::{BTreeMap, HashMap, HashSet};
use super::{SourceDoc, YamlEditError};

/// Kind of YAML anchor token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnchorKind {
    /// Anchor definition: `&name`
    Anchor,
    /// Alias reference: `*name`
    Alias,
}

/// A scanned anchor definition or alias reference located in a [`SourceDoc`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorOccurrence {
    /// The anchor or alias identifier (without leading `&` or `*`).
    pub name: String,
    /// 0-indexed physical line in [`SourceDoc`].
    pub line_idx: usize,
    /// 0-indexed byte column on the line where `&` or `*` begins.
    pub col_idx: usize,
    /// Total byte length of the token including `&` or `*` (`1 + name.len()`).
    pub len: usize,
    /// Whether this occurrence is an anchor definition or an alias reference.
    pub kind: AnchorKind,
}

/// Validate whether a name consists of valid YAML anchor characters.
pub fn is_valid_anchor_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.bytes().all(is_anchor_char)
}

pub(crate) fn is_anchor_char(b: u8) -> bool {
    !b.is_ascii_whitespace()
        && !b.is_ascii_control()
        && !matches!(
            b,
            b',' | b'[' | b']' | b'{' | b'}' | b':' | b'#' | b'"' | b'\''
        )
}

impl SourceDoc {
    /// Scan and return all anchor definitions (`&name`) and alias references
    /// (`*name`) across the document in order of appearance.
    pub fn scan_anchors_and_aliases(&self) -> Vec<AnchorOccurrence> {
        let spans = self.block_scalar_spans();
        let mut occurrences = Vec::new();

        for (line_idx, line) in self.lines.iter().enumerate() {
            if spans.iter().any(|&(s, e)| line_idx >= s && line_idx <= e) {
                continue;
            }
            let text = &line.text;
            let bytes = text.as_bytes();
            let len = bytes.len();
            if len == 0 {
                continue;
            }

            let mut i = 0;
            let mut in_single = false;
            let mut in_double = false;
            let mut flow_depth: usize = 0;
            let mut at_node_start = true;

            while i < len {
                let b = bytes[i];
                if in_double {
                    if b == b'\\' {
                        i += 2;
                    } else {
                        if b == b'"' {
                            in_double = false;
                        }
                        i += 1;
                    }
                    continue;
                }
                if in_single {
                    if b == b'\'' {
                        if i + 1 < len && bytes[i + 1] == b'\'' {
                            i += 2;
                            continue;
                        }
                        in_single = false;
                    }
                    i += 1;
                    continue;
                }
                if b == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
                    break;
                }

                match b {
                    b'"' => {
                        in_double = true;
                        at_node_start = false;
                        i += 1;
                    }
                    b'\'' => {
                        in_single = true;
                        at_node_start = false;
                        i += 1;
                    }
                    b'[' | b'{' => {
                        flow_depth += 1;
                        at_node_start = true;
                        i += 1;
                    }
                    b']' | b'}' => {
                        flow_depth = flow_depth.saturating_sub(1);
                        at_node_start = false;
                        i += 1;
                    }
                    b',' => {
                        at_node_start = flow_depth > 0;
                        i += 1;
                    }
                    b':' => {
                        if i + 1 == len
                            || bytes[i + 1].is_ascii_whitespace()
                            || (flow_depth > 0 && matches!(bytes[i + 1], b',' | b']' | b'}'))
                        {
                            at_node_start = true;
                        }
                        i += 1;
                    }
                    b'-' => {
                        if i + 1 == len || bytes[i + 1].is_ascii_whitespace() {
                            at_node_start = true;
                        }
                        i += 1;
                    }
                    b' ' | b'\t' => i += 1,
                    b'&' if at_node_start => {
                        let col_idx = i;
                        i += 1;
                        let name_start = i;
                        while i < len && is_anchor_char(bytes[i]) {
                            i += 1;
                        }
                        let name = &text[name_start..i];
                        if !name.is_empty() {
                            occurrences.push(AnchorOccurrence {
                                name: name.to_string(),
                                line_idx,
                                col_idx,
                                len: i - col_idx,
                                kind: AnchorKind::Anchor,
                            });
                            at_node_start = true;
                        } else {
                            at_node_start = false;
                        }
                    }
                    b'*' if at_node_start => {
                        let col_idx = i;
                        i += 1;
                        let name_start = i;
                        while i < len && is_anchor_char(bytes[i]) {
                            i += 1;
                        }
                        let name = &text[name_start..i];
                        if !name.is_empty() {
                            occurrences.push(AnchorOccurrence {
                                name: name.to_string(),
                                line_idx,
                                col_idx,
                                len: i - col_idx,
                                kind: AnchorKind::Alias,
                            });
                        }
                        at_node_start = false;
                    }
                    _ => {
                        at_node_start = false;
                        i += 1;
                    }
                }
            }
        }
        occurrences
    }

    /// Return all distinct defined anchor names (`&name`) in document order.
    pub fn anchor_definitions(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut defs = Vec::new();
        for occ in self.scan_anchors_and_aliases() {
            if occ.kind == AnchorKind::Anchor && seen.insert(occ.name.clone()) {
                defs.push(occ.name);
            }
        }
        defs
    }

    /// Return all alias references (`*name`) in document order.
    pub fn alias_references(&self) -> Vec<String> {
        self.scan_anchors_and_aliases()
            .into_iter()
            .filter(|occ| occ.kind == AnchorKind::Alias)
            .map(|occ| occ.name)
            .collect()
    }

    /// Return all alias names that do not have a preceding anchor definition.
    pub fn find_unresolved_aliases(&self) -> Vec<String> {
        let occurrences = self.scan_anchors_and_aliases();
        let mut defined = HashSet::new();
        let mut unresolved = Vec::new();
        for occ in occurrences {
            match occ.kind {
                AnchorKind::Anchor => {
                    defined.insert(occ.name);
                }
                AnchorKind::Alias => {
                    if !defined.contains(&occ.name) {
                        unresolved.push(occ.name);
                    }
                }
            }
        }
        unresolved
    }

    /// Rewrite anchor definitions and alias references matching keys in `mapping`.
    pub fn rewrite_anchors_with_map(
        &mut self,
        mapping: &HashMap<String, String>,
    ) -> Result<usize, YamlEditError> {
        if mapping.is_empty() {
            return Ok(0);
        }
        for (old_name, new_name) in mapping {
            if !is_valid_anchor_name(old_name) {
                return Err(YamlEditError::Unsupported(format!(
                    "invalid source anchor name: '{old_name}'"
                )));
            }
            if !is_valid_anchor_name(new_name) {
                return Err(YamlEditError::Unsupported(format!(
                    "invalid target anchor name: '{new_name}'"
                )));
            }
        }

        let occurrences = self.scan_anchors_and_aliases();
        if occurrences.is_empty() {
            return Ok(0);
        }

        let mut line_groups: BTreeMap<usize, Vec<AnchorOccurrence>> = BTreeMap::new();
        for occ in occurrences {
            if mapping.contains_key(&occ.name) {
                line_groups.entry(occ.line_idx).or_default().push(occ);
            }
        }

        let mut rewritten_count = 0;
        for (line_idx, mut occs) in line_groups {
            occs.sort_by_key(|o| std::cmp::Reverse(o.col_idx));
            let mut text = self.lines[line_idx].text.clone();
            for occ in occs {
                let Some(new_name) = mapping.get(&occ.name) else {
                    continue;
                };
                let col = occ.col_idx;
                let len = occ.len;
                let prefix_char = match occ.kind {
                    AnchorKind::Anchor => '&',
                    AnchorKind::Alias => '*',
                };
                let replacement = format!("{prefix_char}{new_name}");
                if col + len <= text.len() {
                    text.replace_range(col..col + len, &replacement);
                    rewritten_count += 1;
                }
            }
            self.lines[line_idx].text = text;
        }

        Ok(rewritten_count)
    }

    /// Rewrite all anchor definitions and alias references with `prefix`.
    pub fn rewrite_anchor_namespace(&mut self, prefix: &str) -> Result<usize, YamlEditError> {
        let prefix = prefix.trim();
        if prefix.is_empty() {
            return Ok(0);
        }
        if !is_valid_anchor_name(prefix) {
            return Err(YamlEditError::Unsupported(format!(
                "invalid anchor namespace prefix: '{prefix}'"
            )));
        }

        let occurrences = self.scan_anchors_and_aliases();
        if occurrences.is_empty() {
            return Ok(0);
        }

        let mut distinct_names = HashSet::new();
        for occ in &occurrences {
            distinct_names.insert(occ.name.clone());
        }

        let mut mapping = HashMap::new();
        for name in distinct_names {
            let new_name = if prefix.ends_with('_') || prefix.ends_with('-') {
                format!("{prefix}{name}")
            } else {
                format!("{prefix}_{name}")
            };
            mapping.insert(name, new_name);
        }

        self.rewrite_anchors_with_map(&mapping)
    }

    /// Rewrite all occurrences of a single anchor/alias.
    pub fn rewrite_anchor_name(
        &mut self,
        old_name: &str,
        new_name: &str,
    ) -> Result<usize, YamlEditError> {
        let mut mapping = HashMap::new();
        mapping.insert(old_name.to_string(), new_name.to_string());
        self.rewrite_anchors_with_map(&mapping)
    }
}
