//! The snapshot diff chunks, the error banner state and the self-heal
//! rotation signal (EFFECTS-013 #45, #48, #50).
//!
//! A configuration snapshot is compared line by line; what a reader wants is
//! not 400 diff lines but the *blocks* of change: "the proxies section was
//! replaced", "one port was added". The chunk projection groups the lines
//! into those blocks, so a surface draws a few coloured bands instead of a
//! wall of markers.

use infiltrator_contract::dns_self_heal::{DnsSelfHealFix, DnsSelfHealSnapshot};
use infiltrator_contract::yaml_ast_diff::{DiffKind, DiffLine};

/// One block of consecutive changes in a snapshot diff (EFFECTS-013 #45).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffChunk {
    /// The kind of the change the block stands for.
    pub kind: DiffKind,
    /// The first line of the block in the original document.
    pub old_line: Option<usize>,
    /// The first line of the block in the modified document.
    pub new_line: Option<usize>,
    /// How many lines the block spans.
    pub lines: usize,
    /// The text of the block, one entry per line.
    pub content: Vec<String>,
}

/// The blocks of consecutive changes in a snapshot diff.
///
/// Two neighbouring lines of the same kind belong to one block; a line of a
/// different kind starts a new one. An unchanged line never appears: a
/// block is a change, not a region.
pub fn project_diff_chunks(lines: &[DiffLine]) -> Vec<DiffChunk> {
    let mut chunks: Vec<DiffChunk> = Vec::new();
    for line in lines {
        if !line.kind.is_changed() {
            continue;
        }
        let continues = chunks.last().is_some_and(|chunk| chunk.kind == line.kind);
        if continues {
            let chunk = chunks.last_mut().expect("the chunk was just matched");
            chunk.lines += 1;
            chunk.content.push(line.content.clone());
            if chunk.old_line.is_none() {
                chunk.old_line = line.old_line;
            }
            if chunk.new_line.is_none() {
                chunk.new_line = line.new_line;
            }
        } else {
            chunks.push(DiffChunk {
                kind: line.kind,
                old_line: line.old_line,
                new_line: line.new_line,
                lines: 1,
                content: vec![line.content.clone()],
            });
        }
    }
    chunks
}

/// The severity of an error banner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorBannerSeverity {
    /// A neutral notice.
    Info,
    /// Something the user should look at, but nothing is broken.
    Warning,
    /// Something failed.
    Error,
}

/// The state of the error banner of a page (EFFECTS-013 #48).
///
/// The banner is one typed value: a page that shows a banner and a page
/// that does not are two states of the same thing, never a banner plus a
/// boolean that says whether to draw it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ErrorBannerState {
    /// No banner is shown.
    #[default]
    Hidden,
    /// A banner is shown.
    Shown {
        /// How loud the banner is.
        severity: ErrorBannerSeverity,
        /// The headline of the banner.
        title: String,
        /// The detail of the banner.
        detail: String,
        /// Whether the banner offers a retry.
        retry: bool,
    },
}

impl ErrorBannerState {
    /// Whether the banner is shown.
    pub const fn is_shown(&self) -> bool {
        matches!(self, Self::Shown { .. })
    }

    /// The severity of the banner, when it is shown.
    pub const fn severity(&self) -> Option<ErrorBannerSeverity> {
        match self {
            Self::Shown { severity, .. } => Some(*severity),
            Self::Hidden => None,
        }
    }

    /// Whether the banner offers a retry.
    pub const fn offers_retry(&self) -> bool {
        match self {
            Self::Shown { retry, .. } => *retry,
            Self::Hidden => false,
        }
    }
}

/// The rotation signal of the self-heal steps (EFFECTS-013 #50).
///
/// The repair steps of the self-heal run one after another; the signal says
/// which step is the current one, so a surface draws one spinner instead of
/// three.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelfHealRotation {
    /// The step that is running.
    pub step: usize,
    /// How many steps the repair has.
    pub steps: usize,
}

impl SelfHealRotation {
    /// The rotation of the repair steps of one snapshot.
    ///
    /// The steps are the distinct repairs the owner suggested, in the order
    /// it suggested them; a snapshot with nothing to repair has no step at
    /// all.
    pub fn project(snapshot: &DnsSelfHealSnapshot) -> Self {
        Self {
            step: 0,
            steps: snapshot.fixes().len(),
        }
    }

    /// Whether the repair has a step to show.
    pub const fn is_running(&self) -> bool {
        self.steps > 0
    }

    /// The repair of the current step.
    pub fn current_fix(&self, snapshot: &DnsSelfHealSnapshot) -> Option<DnsSelfHealFix> {
        if !self.is_running() {
            return None;
        }
        snapshot.fixes().get(self.step).copied()
    }

    /// Move to the next step and report whether one is left.
    pub fn advance(&mut self) -> bool {
        if self.step + 1 < self.steps {
            self.step += 1;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::dns_self_heal::{
        DnsSelfHealCheck, DnsSelfHealKind, DnsSelfHealState,
    };

    fn line(
        kind: DiffKind,
        old_line: Option<usize>,
        new_line: Option<usize>,
        text: &str,
    ) -> DiffLine {
        DiffLine::new(old_line, new_line, kind, text)
    }

    #[test]
    fn the_chunks_group_the_changes_and_forget_the_unchanged_lines() {
        let lines = vec![
            line(DiffKind::Equal, Some(1), Some(1), "proxies:"),
            line(DiffKind::Delete, Some(2), None, "  - old"),
            line(DiffKind::Delete, Some(3), None, "  - older"),
            line(DiffKind::Insert, None, Some(2), "  - new"),
            line(DiffKind::Equal, Some(4), Some(3), "rules:"),
            line(DiffKind::Modify, Some(5), Some(4), "  - changed"),
        ];
        let chunks = project_diff_chunks(&lines);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].kind, DiffKind::Delete);
        assert_eq!(chunks[0].lines, 2);
        assert_eq!(chunks[0].old_line, Some(2));
        assert_eq!(chunks[0].content, vec!["  - old", "  - older"]);
        assert_eq!(chunks[1].kind, DiffKind::Insert);
        assert_eq!(chunks[1].new_line, Some(2));
        assert_eq!(chunks[2].kind, DiffKind::Modify);
        assert_eq!(chunks[2].lines, 1);
    }

    #[test]
    fn an_unchanged_document_has_no_chunk() {
        let lines = vec![
            line(DiffKind::Equal, Some(1), Some(1), "a"),
            line(DiffKind::Equal, Some(2), Some(2), "b"),
        ];
        assert!(project_diff_chunks(&lines).is_empty());
        assert!(project_diff_chunks(&[]).is_empty());
    }

    #[test]
    fn the_error_banner_is_one_typed_state() {
        let hidden = ErrorBannerState::default();
        assert!(!hidden.is_shown());
        assert_eq!(hidden.severity(), None);
        assert!(!hidden.offers_retry());

        let shown = ErrorBannerState::Shown {
            severity: ErrorBannerSeverity::Error,
            title: "core unreachable".to_owned(),
            detail: "the core did not answer".to_owned(),
            retry: true,
        };
        assert!(shown.is_shown());
        assert_eq!(shown.severity(), Some(ErrorBannerSeverity::Error));
        assert!(shown.offers_retry());
    }

    #[test]
    fn the_self_heal_rotation_walks_the_repair_steps() {
        let snapshot = DnsSelfHealSnapshot::default();
        let mut rotation = SelfHealRotation::project(&snapshot);
        assert!(!rotation.is_running());
        assert_eq!(rotation.current_fix(&snapshot), None);
        assert!(!rotation.advance());
    }

    #[test]
    fn the_self_heal_rotation_counts_the_suggested_repairs() {
        let snapshot = DnsSelfHealSnapshot::new(vec![
            DnsSelfHealCheck {
                kind: DnsSelfHealKind::ListenPort,
                state: DnsSelfHealState::Critical,
                detail: "the port is taken".to_owned(),
                fix: Some(DnsSelfHealFix::RepairDnsListenPort),
            },
            DnsSelfHealCheck {
                kind: DnsSelfHealKind::UpstreamResolution,
                state: DnsSelfHealState::Warning,
                detail: "the upstream is slow".to_owned(),
                fix: Some(DnsSelfHealFix::RecheckUpstreams),
            },
        ]);
        let mut rotation = SelfHealRotation::project(&snapshot);
        assert!(rotation.is_running());
        assert_eq!(rotation.steps, 2);
        assert_eq!(
            rotation.current_fix(&snapshot),
            Some(DnsSelfHealFix::RepairDnsListenPort)
        );
        assert!(rotation.advance());
        assert_eq!(
            rotation.current_fix(&snapshot),
            Some(DnsSelfHealFix::RecheckUpstreams)
        );
        assert!(!rotation.advance(), "the last step is the last step");
    }
}
