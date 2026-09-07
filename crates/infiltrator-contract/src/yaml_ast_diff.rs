//! Shared YAML AST visual diff, snapshot comparison and fidelity read model.

use serde::{Deserialize, Serialize};

/// Type of line difference in a Myers diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Equal,
    Insert,
    Delete,
    Modify,
}

impl DiffKind {
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Equal => " ",
            Self::Insert => "+",
            Self::Delete => "-",
            Self::Modify => "~",
        }
    }

    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Equal => "未变",
            Self::Insert => "+ 新增",
            Self::Delete => "- 移除",
            Self::Modify => "~ 修改",
        }
    }

    pub const fn is_changed(self) -> bool {
        !matches!(self, Self::Equal)
    }
}

/// A single line in a unified or split diff output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    /// 1-indexed line number in the original/left document (None for inserted lines).
    pub old_line: Option<usize>,
    /// 1-indexed line number in the modified/right document (None for deleted lines).
    pub new_line: Option<usize>,
    /// Operation kind.
    pub kind: DiffKind,
    /// Line text content without newline terminator.
    pub content: String,
}

impl DiffLine {
    pub fn new(old_line: Option<usize>, new_line: Option<usize>, kind: DiffKind, content: impl Into<String>) -> Self {
        Self {
            old_line,
            new_line,
            kind,
            content: content.into(),
        }
    }
}

/// A row in a side-by-side (split) visual diff.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitDiffRow {
    /// Left pane item (original version). None if line was inserted on the right.
    pub left: Option<DiffLine>,
    /// Right pane item (modified version). None if line was deleted from the left.
    pub right: Option<DiffLine>,
    /// Aggregate row kind.
    pub kind: DiffKind,
}

impl SplitDiffRow {
    pub fn new(left: Option<DiffLine>, right: Option<DiffLine>, kind: DiffKind) -> Self {
        Self { left, right, kind }
    }
}

/// Summary statistics of changes between two YAML configuration states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffStats {
    pub additions: usize,
    pub deletions: usize,
    pub modifications: usize,
    pub unchanged: usize,
}

impl DiffStats {
    pub const fn total_changes(&self) -> usize {
        self.additions + self.deletions + self.modifications
    }

    pub const fn is_identical(&self) -> bool {
        self.total_changes() == 0
    }
}

/// YAML formatting and syntax fidelity guarantee level achieved by the AST operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FidelityGrade {
    /// L1: Comment Fidelity (standalone and inline comments preserved verbatim).
    L1Comments,
    /// L2: Layout & Key Order Fidelity (indentation, blank lines, quotes, CRLF/LF, BOM preserved).
    L2Layout,
    /// L3: Anchor Consistency (anchors and aliases preserved and namespace-checked).
    #[default]
    L3Anchors,
}

impl FidelityGrade {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::L1Comments => "L1-Comments",
            Self::L2Layout => "L2-Layout",
            Self::L3Anchors => "L3-Anchors",
        }
    }

    pub const fn description_zh(self) -> &'static str {
        match self {
            Self::L1Comments => "L1级 注释逐字保留",
            Self::L2Layout => "L2级 排版与键序零漂移",
            Self::L3Anchors => "L3级 锚点一致性隔离保障",
        }
    }
}

/// Read model snapshot representing visual AST diff between two configurations.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct YamlAstDiffSnapshot {
    /// Identifier or timestamp label of the source (older/base) configuration.
    pub source_id: String,
    /// Identifier or timestamp label of the target (newer/modified) configuration.
    pub target_id: String,
    /// Change count metrics.
    pub stats: DiffStats,
    /// Unified diff line sequence (for inline diff view).
    pub unified_lines: Vec<DiffLine>,
    /// Paired side-by-side row sequence (for split diff view).
    pub split_rows: Vec<SplitDiffRow>,
    /// AST fidelity guarantee level.
    pub fidelity_grade: FidelityGrade,
    /// Whether 100% comment, blank line, and anchor fidelity was preserved.
    pub fidelity_preserved: bool,
}

impl YamlAstDiffSnapshot {
    /// Deterministic fixture for headless UI tests and mock renders.
    pub fn demo_fixture() -> Self {
        let stats = DiffStats {
            additions: 1,
            deletions: 1,
            modifications: 1,
            unchanged: 2,
        };

        let unified_lines = vec![
            DiffLine::new(Some(1), Some(1), DiffKind::Equal, "mixed-port: 7890"),
            DiffLine::new(Some(2), Some(2), DiffKind::Equal, "mode: rule"),
            DiffLine::new(Some(3), None, DiffKind::Delete, "rules: [DOMAIN-SUFFIX,google.com,DIRECT]"),
            DiffLine::new(None, Some(3), DiffKind::Insert, "proxies: [SS-Tokyo, VLESS-Reality-US, HK-01]"),
            DiffLine::new(Some(4), Some(4), DiffKind::Modify, "tun: { enable: false → true, stack: gvisor }"),
        ];

        let split_rows = vec![
            SplitDiffRow::new(
                Some(DiffLine::new(Some(1), None, DiffKind::Equal, "mixed-port: 7890")),
                Some(DiffLine::new(None, Some(1), DiffKind::Equal, "mixed-port: 7890")),
                DiffKind::Equal,
            ),
            SplitDiffRow::new(
                Some(DiffLine::new(Some(2), None, DiffKind::Equal, "mode: rule")),
                Some(DiffLine::new(None, Some(2), DiffKind::Equal, "mode: rule")),
                DiffKind::Equal,
            ),
            SplitDiffRow::new(
                Some(DiffLine::new(Some(3), None, DiffKind::Delete, "rules: [DOMAIN-SUFFIX,google.com,DIRECT]")),
                None,
                DiffKind::Delete,
            ),
            SplitDiffRow::new(
                None,
                Some(DiffLine::new(None, Some(3), DiffKind::Insert, "proxies: [SS-Tokyo, VLESS-Reality-US, HK-01]")),
                DiffKind::Insert,
            ),
            SplitDiffRow::new(
                Some(DiffLine::new(Some(4), None, DiffKind::Modify, "tun: { enable: false, stack: gvisor }")),
                Some(DiffLine::new(None, Some(4), DiffKind::Modify, "tun: { enable: true, stack: gvisor }")),
                DiffKind::Modify,
            ),
        ];

        Self {
            source_id: "snapshot-1735489200000".to_string(),
            target_id: "current-profile".to_string(),
            stats,
            unified_lines,
            split_rows,
            fidelity_grade: FidelityGrade::L3Anchors,
            fidelity_preserved: true,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.unified_lines.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.stats.total_changes()
    }

    pub fn has_differences(&self) -> bool {
        !self.stats.is_identical()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_kind_symbols_and_labels_are_sound() {
        assert_eq!(DiffKind::Insert.symbol(), "+");
        assert_eq!(DiffKind::Delete.symbol(), "-");
        assert_eq!(DiffKind::Modify.symbol(), "~");
        assert_eq!(DiffKind::Equal.symbol(), " ");

        assert!(DiffKind::Insert.is_changed());
        assert!(!DiffKind::Equal.is_changed());
    }

    #[test]
    fn diff_stats_change_calculations() {
        let stats = DiffStats {
            additions: 5,
            deletions: 3,
            modifications: 2,
            unchanged: 100,
        };
        assert_eq!(stats.total_changes(), 10);
        assert!(!stats.is_identical());

        let empty = DiffStats::default();
        assert!(empty.is_identical());
    }

    #[test]
    fn demo_fixture_properties_and_serde() {
        let fixture = YamlAstDiffSnapshot::demo_fixture();
        assert_eq!(fixture.total_changes(), 3);
        assert!(fixture.fidelity_preserved);
        assert_eq!(fixture.fidelity_grade, FidelityGrade::L3Anchors);
        assert_eq!(fixture.unified_lines.len(), 5);
        assert_eq!(fixture.split_rows.len(), 5);

        let serialized = serde_json::to_string(&fixture).expect("serialize diff snapshot");
        let deserialized: YamlAstDiffSnapshot =
            serde_json::from_str(&serialized).expect("deserialize diff snapshot");
        assert_eq!(fixture, deserialized);
    }
}
