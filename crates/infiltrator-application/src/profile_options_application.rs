//! DUAL-09-14: per-profile option sidecar use-cases (Mixin overlay + filter).
//!
//! Both surfaces edit the same two stored documents, so both call this one
//! application service:
//!
//! * [`ProfileOptionsApplication::load`] reads the sidecar, renders the stored
//!   `MixinConfig` back into the editor's YAML buffer, converts the stored
//!   filter spec into the shared surface draft and publishes the snapshot for
//!   the surface projection;
//! * [`ProfileOptionsApplication::save_mixin`] is the Mixin editor's commit:
//!   strip the outgoing mixin's injected rule lines (so repeated edits stay
//!   idempotent), merge with the byte-faithful engine
//!   ([`infiltrator_domain::mixin::merge_profile_with_config_fidelity`]),
//!   validate, apply through the shared transaction and persist the sidecar;
//! * [`ProfileOptionsApplication::save_filter`] is the filter editor's commit:
//!   parse the shared draft and run the shared filter application, which
//!   re-runs the pipeline in place and persists the spec.
//!
//! Keeping the composition here is what retires the surface-local copy: a
//! surface may render the buffers, but it never owns the strip/merge/persist
//! rules or the regex compilation.
use infiltrator_domain::filter_policy_form;

use crate::profile_application::{ProfileApplication, valid_name};
use crate::profile_document_application::document_snapshot;
use crate::profile_editor_observations::EditorFacet;
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason};
use infiltrator_contract::profile_document::ProfileWorkspaceReceipt;
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_contract::subscription_filter_result::SubscriptionFilterApplied;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_domain::config::validate_yaml;
use infiltrator_domain::mixin::{MixinConfig, merge_profile_with_config_fidelity};
use infiltrator_domain::profile_options;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::sync::Arc;

/// Resolve the sidecar profile: an explicit name, or the active profile.
async fn resolve_profile(
    profiles: &ProfileApplication,
    profile: Option<&str>,
) -> Result<String, Failure> {
    match profile {
        Some(name) if !name.trim().is_empty() => valid_name(name),
        _ => profiles.current_profile().await,
    }
}

#[derive(Clone)]
pub struct ProfileOptionsApplication {
    profiles: ProfileApplication,
}

impl ProfileOptionsApplication {
    pub fn new(profiles: ProfileApplication) -> Self {
        Self { profiles }
    }

    /// Load the sidecar, publish it for the surface projection and return it.
    /// `profile = None` selects the active profile.
    pub async fn load(&self, profile: Option<&str>) -> Result<ProfileOptionsSnapshot, Failure> {
        let name = resolve_profile(&self.profiles, profile).await?;
        let read = self.profiles.begin_editor_read(&name, EditorFacet::Options);
        self.profiles.mark_editor_loading(&read);
        let result = self
            .profiles
            .load_workspace(&name)
            .await
            .and_then(project_options);
        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(failure) => {
                self.profiles.fail_editor_read(&read, failure.clone());
                return Err(failure);
            }
        };
        if !self.profiles.observe_options(&read, snapshot.clone()) {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Profile options read was superseded",
                false,
            ));
        }
        Ok(snapshot)
    }

    /// Commit an edited Mixin overlay for `profile`.
    pub async fn save_mixin<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        expected: &ProfileSourceIdentity,
        mixin_yaml: &str,
    ) -> Result<ProfileWorkspaceReceipt, Failure> {
        let mixin: MixinConfig = serde_yaml_ng::from_str(mixin_yaml).map_err(|error| {
            Failure::new(ErrorCode::InvalidInput, error.to_string(), false)
                .with_reason(FailureReason::MixinYaml)
        })?;
        let profile = valid_name(&expected.profile)?;
        let read = self
            .profiles
            .begin_editor_read(&profile, EditorFacet::Options);
        let document_read = self
            .profiles
            .begin_editor_read(&profile, EditorFacet::Document);
        let workspace = self.profiles.load_workspace(&profile).await?;
        if workspace.source != *expected {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The observed Mixin document or options changed; inspect before saving",
                true,
            ));
        }
        let old = &workspace.options;
        // Mixin rule injection is cumulative (`prepend ++ base ++ append`), so
        // re-applying an edited mixin onto an already-composed profile would
        // duplicate the old lines unless the outgoing mixin's lines are
        // removed first — exactly what the Iced editor has always done.
        let removals: Vec<String> = old
            .mixin
            .rules
            .iter()
            .flat_map(|rules| rules.prepend.iter().chain(rules.append.iter()).cloned())
            .collect();
        let base = profile_options::strip_rule_lines(&workspace.content, &removals);
        let merged = merge_profile_with_config_fidelity(&base, &mixin)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        validate_yaml(&merged)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            content: merged,
            options: ProfileOptions {
                mixin,
                filter: old.filter.clone(),
            },
        };
        let committed = self
            .profiles
            .commit_workspace(runtime, expected, &update)
            .await?;
        let document = document_snapshot(committed.clone());
        let options = project_options(committed)?;
        self.profiles
            .observe_document(&document_read, document.clone());
        self.profiles.observe_options(&read, options.clone());
        Ok(ProfileWorkspaceReceipt {
            previous: expected.clone(),
            submitted_mixin_yaml: mixin_yaml.into(),
            document,
            options,
        })
    }

    /// Parse a surface filter draft and run the shared filter application.
    pub async fn save_filter<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        expected: &ProfileSourceIdentity,
        draft: &SubscriptionFilterDraft,
    ) -> Result<SubscriptionFilterApplied, Failure> {
        let spec = filter_policy_form::filter_spec_from_draft(draft)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let read = self
            .profiles
            .begin_editor_read(&expected.profile, EditorFacet::Options);
        let workspace = self.profiles.load_workspace(&expected.profile).await?;
        if workspace.source != *expected {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The filter source changed; inspect the current profile and options",
                true,
            ));
        }
        let spec = filter_policy_form::filter_spec_with_form_fields(
            workspace.options.filter.as_ref(),
            spec,
            draft.advanced_policy.is_some(),
        );
        let mixin_yaml = serde_yaml_ng::to_string(&workspace.options.mixin)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        let stored_draft = filter_policy_form::filter_spec_to_draft(&spec)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let report = self
            .profiles
            .apply_subscription_filter(runtime, expected, spec)
            .await?;
        // Publish the stored draft so a surface that just applied it (and any
        // other surface reading the same projection) sees the persisted form.
        self.profiles.observe_options(
            &read,
            ProfileOptionsSnapshot::new(report.source.clone(), mixin_yaml, stored_draft),
        );
        Ok(report)
    }
}

fn project_options(workspace: ProfileWorkspace) -> Result<ProfileOptionsSnapshot, Failure> {
    let mixin_yaml = serde_yaml_ng::to_string(&workspace.options.mixin)
        .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
    let filter = workspace
        .options
        .filter
        .as_ref()
        .map(filter_policy_form::filter_spec_to_draft)
        .transpose()
        .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?
        .unwrap_or_default();
    Ok(ProfileOptionsSnapshot::new(
        workspace.source,
        mixin_yaml,
        filter,
    ))
}
