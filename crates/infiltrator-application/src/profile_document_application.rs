//! DUAL-09-03/14: profile document use-cases for the editor surfaces.
//!
//! Both editors edit the stored profile document through this application:
//! [`ProfileDocumentApplication::load`] reads the document, classifies the
//! write protection and runs the *shared* syntax preflight
//! ([`infiltrator_domain::config::preflight_yaml_syntax`]) once;
//! [`ProfileDocumentApplication::save`] rejects a syntactically invalid buffer
//! and then commits through the same guarded write path and apply transaction
//! the Iced editor uses (`save_edited_profile_content`).
//!
//! Live per-keystroke diagnostics stay in the surface: they call the same
//! domain function on the local buffer, so both surfaces share one rule set
//! without a round trip per character.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document::{
    ProfileDocumentSnapshot, SyntaxDiagnosticSnapshot, publish_profile_document,
};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::config::preflight_yaml_syntax;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::sync::Arc;

use crate::profile_application::ProfileApplication;

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
        let detail = self.profiles.load_profile_detail(&name).await?;
        let protection = ProfileWriteProtection::from_subscription_url(
            detail.subscription_url.as_deref().unwrap_or_default(),
        );
        let syntax = preflight_yaml_syntax(&detail.content)
            .err()
            .map(|diagnostic| SyntaxDiagnosticSnapshot {
                line: diagnostic.line,
                column: diagnostic.column,
                message: diagnostic.message,
            });
        let document = ProfileDocumentSnapshot::new(detail.name, detail.content, protection)
            .with_syntax(syntax);
        publish_profile_document(document.clone());
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
        profile: &str,
        content: &str,
        allow_protected: bool,
    ) -> Result<ProfileDocumentSnapshot, Failure> {
        if let Err(diagnostic) = preflight_yaml_syntax(content) {
            return Err(Failure::new(
                ErrorCode::Configuration,
                format!(
                    "YAML 语法错误（第 {} 行，第 {} 列）：{}",
                    diagnostic.line, diagnostic.column, diagnostic.message
                ),
                false,
            ));
        }
        self.profiles
            .save_edited_profile_content(
                runtime,
                profile.to_string(),
                content.to_string(),
                ApplyStrategy::PreferReload,
                allow_protected,
            )
            .await?;
        // Re-read what actually landed: the transaction may have reloaded or
        // restarted the core, and the surfaces must render the stored bytes.
        self.load(Some(profile)).await
    }
}
