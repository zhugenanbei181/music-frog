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

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_options::{ProfileOptionsSnapshot, publish_profile_options};
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::filter::FilterReport;
use infiltrator_domain::mixin::MixinConfig;
use infiltrator_domain::profile_options::{self, ProfileOptions};
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::sync::Arc;

use crate::profile_application::ProfileApplication;

/// Resolve the sidecar profile: an explicit name, or the active profile.
async fn resolve_profile(
    profiles: &ProfileApplication,
    profile: Option<&str>,
) -> Result<String, Failure> {
    match profile {
        Some(name) if !name.trim().is_empty() => Ok(name.to_string()),
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
        let options = self.profiles.load_options(&name).await?;
        let mixin_yaml = serde_yaml_ng::to_string(&options.mixin)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        let filter = options
            .filter
            .as_ref()
            .map(profile_options::filter_spec_to_draft)
            .unwrap_or_default();
        let snapshot = ProfileOptionsSnapshot::new(name, mixin_yaml, filter);
        publish_profile_options(snapshot.clone());
        Ok(snapshot)
    }

    /// Commit an edited Mixin overlay for `profile`.
    pub async fn save_mixin<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        profile: &str,
        mixin_yaml: &str,
    ) -> Result<ProfileOptionsSnapshot, Failure> {
        let mixin: MixinConfig = serde_yaml_ng::from_str(mixin_yaml).map_err(|error| {
            Failure::new(
                ErrorCode::InvalidInput,
                format!("Mixin 覆盖不是有效的 YAML: {error}"),
                false,
            )
        })?;
        let old = self.profiles.load_options(profile).await?;
        let content = self.profiles.load_profile_detail(profile).await?.content;
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
        let base = profile_options::strip_rule_lines(&content, &removals);
        let merged = infiltrator_domain::mixin::merge_profile_with_config_fidelity(&base, &mixin)
            .map_err(|error| {
            Failure::new(ErrorCode::Configuration, error.to_string(), false)
        })?;
        infiltrator_domain::config::validate_yaml(&merged)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        self.profiles
            .save_profile_content(
                runtime,
                profile.to_string(),
                merged,
                ApplyStrategy::PreferReload,
            )
            .await?;
        self.profiles
            .save_options(
                profile,
                &ProfileOptions {
                    mixin,
                    filter: old.filter,
                },
            )
            .await?;
        self.load(Some(profile)).await
    }

    /// Parse a surface filter draft and run the shared filter application.
    pub async fn save_filter<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        profile: &str,
        draft: &SubscriptionFilterDraft,
    ) -> Result<FilterReport, Failure> {
        let spec = profile_options::filter_spec_from_draft(draft)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let report = self
            .profiles
            .apply_subscription_filter(runtime, profile, spec)
            .await?;
        // Publish the stored draft so a surface that just applied it (and any
        // other surface reading the same projection) sees the persisted form.
        self.load(Some(profile)).await?;
        Ok(report)
    }
}
