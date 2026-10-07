//! DUAL-09-03/14: profile document use-cases for the editor surfaces.
//!
//! Both editors edit the stored profile document through this application:
//! [`ProfileDocumentApplication::load`] reads the document, classifies the
//! write protection and runs the *shared* syntax preflight
//! ([`infiltrator_domain::config::preflight_yaml_syntax`]) once;
//! [`ProfileDocumentApplication::save`] rejects a syntactically invalid buffer
//! and commits the exact caller-observed workspace through the protected, source-bound transaction.
//!
//! Live per-keystroke diagnostics stay in the surface: they call the same
//! domain function on the local buffer, so both surfaces share one rule set
//! without a round trip per character.

use crate::profile_application::{ProfileApplication, valid_name};
use crate::profile_editor_observations::EditorFacet;
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason};
use infiltrator_contract::profile_document::{ProfileDocumentSnapshot, SyntaxDiagnosticSnapshot};
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_contract::yaml_snippets::{SnippetCaret, SnippetInsertion, insert_at_caret};
use infiltrator_domain::config::preflight_yaml_syntax;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProfileDocumentApplication {
    profiles: ProfileApplication,
}

impl ProfileDocumentApplication {
    pub fn new(profiles: ProfileApplication) -> Self {
        Self { profiles }
    }

    /// Load the stored document (defaults to the active profile), preflight it
    /// with the shared checker and publish it for the surface read model.
    pub async fn load(&self, profile: Option<&str>) -> Result<ProfileDocumentSnapshot, Failure> {
        let name = match profile {
            Some(name) if !name.trim().is_empty() => name.to_string(),
            _ => self.profiles.current_profile().await?,
        };
        let name = valid_name(&name)?;
        let read = self
            .profiles
            .begin_editor_read(&name, EditorFacet::Document);
        self.profiles.mark_editor_loading(&read);
        let workspace = match self.profiles.load_workspace(&name).await {
            Ok(workspace) => workspace,
            Err(failure) => {
                self.profiles.fail_editor_read(&read, failure.clone());
                return Err(failure);
            }
        };
        let document = document_snapshot(workspace);
        if !self.profiles.observe_document(&read, document.clone()) {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Profile document read was superseded",
                false,
            ));
        }
        Ok(document)
    }

    /// Commit an editor buffer as `profile`'s stored document.
    ///
    /// The buffer must pass the shared preflight first: the apply transaction
    /// validates too, but a typed syntax error belongs to the editor, not to a
    /// rollback. `allow_protected` carries the editor's explicit unlock; the
    /// application re-checks the stored subscription metadata.
    pub async fn save<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        expected: &ProfileSourceIdentity,
        content: &str,
        allow_protected: bool,
    ) -> Result<ProfileDocumentSnapshot, Failure> {
        if let Err(diagnostic) = preflight_yaml_syntax(content) {
            return Err(
                Failure::new(ErrorCode::Configuration, diagnostic.message, false).with_reason(
                    FailureReason::YamlSyntax {
                        line: diagnostic.line,
                        column: diagnostic.column,
                    },
                ),
            );
        }
        let name = valid_name(&expected.profile)?;
        let read = self
            .profiles
            .begin_editor_read(&name, EditorFacet::Document);
        let workspace = self.profiles.load_workspace(&name).await?;
        if workspace.source != *expected {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The observed profile document or options changed; inspect before saving",
                true,
            ));
        }
        let committed = self
            .profiles
            .commit_workspace(
                runtime,
                expected,
                &ProfileWorkspaceUpdate {
                    purpose: ProfileWorkspacePurpose::DirectEdit { allow_protected },
                    content: content.to_owned(),
                    options: workspace.options,
                },
            )
            .await?;
        let document = document_snapshot(committed);
        self.profiles.observe_document(&read, document.clone());
        Ok(document)
    }
}

pub(crate) fn document_snapshot(workspace: ProfileWorkspace) -> ProfileDocumentSnapshot {
    let syntax = preflight_yaml_syntax(&workspace.content)
        .err()
        .map(|diagnostic| SyntaxDiagnosticSnapshot {
            line: diagnostic.line,
            column: diagnostic.column,
            message: diagnostic.message,
        });
    let mut document = ProfileDocumentSnapshot::new(
        workspace.source.profile.clone(),
        workspace.content,
        workspace.write_protection,
    )
    .with_syntax(syntax);
    document.source = Some(workspace.source);
    document
}

/// DUAL-09-04: splice a shared catalogue snippet at a caret.
///
/// This is the one snippet use-case both editors call. The catalogue and the
/// byte-faithful splice live in the contract
/// ([`infiltrator_contract::yaml_snippets`]); the application adds the rule a
/// snippet must respect — the spliced document is re-checked with the *same*
/// shared preflight the save path enforces, so a rule snippet dropped into the
/// `proxies:` list is refused with a typed, line/column-carrying failure
/// instead of being applied and only then rejected on save.
///
/// A document that was already invalid keeps that verdict attached to the
/// insertion (`SnippetInsertion::syntax`) instead of refusing the edit: the
/// user is mid-repair, and the surface renders the shared diagnostic.
pub fn insert_snippet(
    content: &str,
    snippet_id: &str,
    line: usize,
    column: usize,
) -> Result<SnippetInsertion, Failure> {
    let was_clean = preflight_yaml_syntax(content).is_ok();
    let insertion = insert_at_caret(content, snippet_id, SnippetCaret { line, column })
        .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
    let syntax = preflight_yaml_syntax(&insertion.content)
        .err()
        .map(|diagnostic| SyntaxDiagnosticSnapshot {
            line: diagnostic.line,
            column: diagnostic.column,
            message: diagnostic.message,
        });
    if was_clean && let Some(diagnostic) = syntax.as_ref() {
        return Err(Failure::new(
            ErrorCode::Configuration,
            format!(
                "片段「{}」插入后配置无法解析（第 {} 行，第 {} 列）：{}",
                insertion.snippet_id, diagnostic.line, diagnostic.column, diagnostic.message
            ),
            false,
        ));
    }
    Ok(insertion.with_syntax(syntax))
}
