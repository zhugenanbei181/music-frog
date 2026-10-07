//! DUAL-09-03/14: shared profile-document read model for the editor surfaces.
//!
//! Both editors edit the same fact — the *stored* profile document — and both
//! run the same shared syntax preflight (`infiltrator_domain::config::
//! preflight_yaml_syntax`). This module carries the loaded document and its
//! parsed diagnostic across the surface boundary so the Bevy editor starts from
//! the application's own read instead of reading the file itself.
//!
//! The live per-keystroke check stays in the surface: it calls the same domain
//! function on the buffer, so a keystroke never waits for a round trip while
//! the *rules* stay single-sourced.

use crate::profile_options::ProfileOptionsSnapshot;
use crate::profile_protection::ProfileWriteProtection;
use crate::profile_source::ProfileSourceIdentity;
use serde::{Deserialize, Serialize};

/// Shared form of `infiltrator_domain::config::SyntaxDiagnostic`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxDiagnosticSnapshot {
    /// 1-based line number the parser blamed.
    pub line: usize,
    /// 1-based column on that line.
    pub column: usize,
    pub message: String,
}

impl SyntaxDiagnosticSnapshot {
    /// Badge text both surfaces can render verbatim.
    pub fn line_label(&self) -> String {
        format!("Line {} : Col {}", self.line, self.column)
    }
}

/// The stored document of one profile plus its shared preflight verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDocumentSnapshot {
    pub profile: String,
    #[serde(default)]
    pub source: Option<ProfileSourceIdentity>,
    pub content: String,
    /// The same write classification the save guard enforces (DUAL-09-12).
    pub write_protection: ProfileWriteProtection,
    /// `Some` only when the stored document currently fails the preflight.
    pub syntax: Option<SyntaxDiagnosticSnapshot>,
    pub line_count: usize,
}

impl ProfileDocumentSnapshot {
    pub fn new(
        profile: impl Into<String>,
        content: impl Into<String>,
        write_protection: ProfileWriteProtection,
    ) -> Self {
        let content = content.into();
        let line_count = content.lines().count();
        Self {
            profile: profile.into(),
            source: None,
            content,
            write_protection,
            syntax: None,
            line_count,
        }
    }

    pub fn with_syntax(mut self, syntax: Option<SyntaxDiagnosticSnapshot>) -> Self {
        self.syntax = syntax;
        self
    }

    pub fn is_clean(&self) -> bool {
        self.syntax.is_none()
    }

    /// Line numbers rendered in the editor gutter.
    pub fn line_numbers(&self) -> impl Iterator<Item = usize> + '_ {
        1..=self.line_count.max(1)
    }
}

/// Both facets returned by one actual document/sidecar transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileWorkspaceReceipt {
    pub previous: ProfileSourceIdentity,
    pub submitted_mixin_yaml: String,
    pub document: ProfileDocumentSnapshot,
    pub options: ProfileOptionsSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDocumentSaved {
    pub previous: ProfileSourceIdentity,
    pub document: ProfileDocumentSnapshot,
}
