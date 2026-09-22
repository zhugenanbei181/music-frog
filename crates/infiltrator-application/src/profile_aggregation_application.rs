//! DUAL-08 shared aggregation application.
//!
//! Owns the only code path that turns an [`AggregationDraft`] into an
//! [`AggregationReport`]: it reads the selected source profiles through the
//! runtime-neutral [`ProfileApplication`], feeds them to the domain
//! aggregator, and publishes the resulting structured plan. Both surfaces
//! consume the same process-wide report, so neither re-implements
//! deduplication, geo clustering, or group synthesis.
//!
//! The report is cached process-wide (like the subscription single-flight
//! registry) because the desktop service outlives any single surface window:
//! a preview produced from either surface is projected to both.

use std::sync::{Mutex, OnceLock};

use chrono::Utc;
use infiltrator_contract::aggregator::{
    AggregatedProfileOutcome, AggregationDraft, AggregationReport, AggregationTemplate,
    GeneratedGroupSnapshot, RegionalClusterSnapshot,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::filter::{ContentDedupStrategy, DeduplicationStrategy, NodeSortOrder};
use infiltrator_domain::profile_aggregator::{AggregationPlan, ProfileAggregator};
use infiltrator_domain::profile_converter::{AggregationOptions, SourceSubscription};
use infiltrator_domain::profiles::sanitize_profile_name;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::sync::Arc;

use crate::profile_application::ProfileApplication;

#[cfg(test)]
#[path = "profile_aggregation_application_test.rs"]
mod profile_aggregation_application_test;

/// Process-wide cache of the most recent aggregation preview.
fn report_cache() -> &'static Mutex<Option<AggregationReport>> {
    static REPORT: OnceLock<Mutex<Option<AggregationReport>>> = OnceLock::new();
    REPORT.get_or_init(|| Mutex::new(None))
}

/// The last aggregation report previewed in this process, if any.
pub fn last_aggregation_report() -> Option<AggregationReport> {
    report_cache().lock().ok().and_then(|cache| cache.clone())
}

/// Replace the process-wide aggregation preview.
pub fn publish_aggregation_report(report: AggregationReport) {
    if let Ok(mut cache) = report_cache().lock() {
        *cache = Some(report);
    }
}

/// Drop the cached preview (used when the draft no longer previews).
pub fn clear_aggregation_report() {
    if let Ok(mut cache) = report_cache().lock() {
        *cache = None;
    }
}

/// DUAL-08: translate the surface-neutral draft into domain options. The
/// toggles are additive: disabling every cleaning switch still merges the
/// sources, it just does not clean them.
pub fn aggregation_options(draft: &AggregationDraft) -> AggregationOptions {
    AggregationOptions {
        content_dedup: if draft.deduplicate {
            ContentDedupStrategy::KeepFirst
        } else {
            ContentDedupStrategy::Disabled
        },
        name_dedup: if draft.deduplicate_names {
            DeduplicationStrategy::AppendIndex
        } else {
            DeduplicationStrategy::Disabled
        },
        normalize_country_code: draft.geo_cluster,
        remove_emojis: draft.remove_emojis,
        sort_by: if draft.geo_cluster {
            NodeSortOrder::CountryCode
        } else {
            NodeSortOrder::Preserve
        },
        generate_proxy_groups: draft.generate_groups,
        rename_rules: draft.rename_rules.clone(),
        custom_groups: draft.custom_groups.clone(),
        availability_precheck: draft.availability_precheck,
    }
}

/// Shared aggregation use-cases.
#[derive(Clone)]
pub struct ProfileAggregationApplication {
    profile: ProfileApplication,
}

impl ProfileAggregationApplication {
    pub fn new(profile: ProfileApplication) -> Self {
        Self { profile }
    }

    /// DUAL-08-01/08-11: read the selected sources, run the shared domain
    /// aggregator, cache and return the structured preview.
    pub async fn preview(&self, draft: &AggregationDraft) -> Result<AggregationReport, Failure> {
        if !draft.is_previewable() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "select at least one source profile to aggregate",
                false,
            ));
        }

        let mut sources = Vec::new();
        let mut missing_sources = Vec::new();
        for name in &draft.source_profiles {
            match self.profile.load_profile_detail(name).await {
                Ok(detail) if !detail.content.trim().is_empty() => {
                    sources.push(SourceSubscription::new(name.clone(), detail.content));
                }
                _ => missing_sources.push(name.clone()),
            }
        }

        if sources.is_empty() {
            return Err(Failure::new(
                ErrorCode::Configuration,
                "none of the selected source profiles could be read",
                false,
            ));
        }

        let mut report = Self::plan_documents(draft, &sources)?;
        report.missing_sources = missing_sources;
        publish_aggregation_report(report.clone());
        Ok(report)
    }

    /// Pure plan step: no store access, no cache. Callers that need the
    /// published preview use [`Self::preview`].
    pub fn plan_documents(
        draft: &AggregationDraft,
        sources: &[SourceSubscription],
    ) -> Result<AggregationReport, Failure> {
        let plan = ProfileAggregator::plan(sources, &aggregation_options(draft))
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        Ok(report_from_plan(draft, plan))
    }

    /// DUAL-08-06: materialise the draft into a brand new profile without
    /// touching any source profile. The target must be a fresh name; an
    /// existing profile is rejected rather than silently overwritten.
    pub async fn create_profile(
        &self,
        draft: &AggregationDraft,
    ) -> Result<AggregatedProfileOutcome, Failure> {
        self.create_profile_with_runtime::<dyn ManagedRuntime>(None, draft)
            .await
    }

    /// DUAL-08-06/08-12: materialise the draft, optionally make the result the
    /// active profile, and remember the draft as a reusable template so the
    /// generated profile keeps a re-aggregation link (DUAL-08-07).
    ///
    /// Activation reuses the shared [`ProfileApplication::activate_profile`]
    /// path (switch + hot reload + rollback); a host without a managed-runtime
    /// seam switches the active profile without hot reload exactly like the
    /// shared `SwitchProfile` route.
    pub async fn create_profile_with_runtime<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        draft: &AggregationDraft,
    ) -> Result<AggregatedProfileOutcome, Failure> {
        let name = sanitize_profile_name(&draft.target_name)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let existing = self.profile.list_profiles().await?;
        if existing.iter().any(|profile| profile.name == name) {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!("profile `{name}` already exists"),
                false,
            ));
        }

        let report = self.preview(draft).await?;
        // The rendered YAML is validated by the store on save; a rejected save
        // leaves no partial profile behind.
        self.profile.save_profile(&name, &report.yaml).await?;

        let (activated, core_reloaded) = if draft.activate_after_create {
            self.activate_generated_profile(runtime, &name).await?
        } else {
            (false, false)
        };

        // Remember the wizard configuration under the generated profile name.
        // A store without a template sidecar honestly reports "not saved"
        // instead of failing the profile creation.
        let template_name = match self.save_template(&name, draft).await {
            Ok(template) => Some(template.name),
            Err(failure) if failure.code == ErrorCode::Unsupported => None,
            Err(failure) => return Err(failure),
        };

        Ok(AggregatedProfileOutcome {
            profile_name: name,
            report,
            activated,
            core_reloaded,
            template_name,
        })
    }

    /// DUAL-08-13: the persisted aggregation templates, newest field first.
    pub async fn list_templates(&self) -> Result<Vec<AggregationTemplate>, Failure> {
        self.profile.load_aggregation_templates().await
    }

    /// DUAL-08-13: upsert a template under `name` from the live draft.
    pub async fn save_template(
        &self,
        name: &str,
        draft: &AggregationDraft,
    ) -> Result<AggregationTemplate, Failure> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "aggregation template name is required",
                false,
            ));
        }
        if !draft.is_previewable() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "select at least one source profile to save as a template",
                false,
            ));
        }

        let template = AggregationTemplate {
            name: name.to_owned(),
            draft: draft.clone(),
            updated_at: Utc::now().to_rfc3339(),
        };
        let mut templates = self.list_templates().await?;
        match templates
            .iter_mut()
            .find(|existing| existing.name == template.name)
        {
            Some(existing) => *existing = template.clone(),
            None => templates.push(template.clone()),
        }
        self.profile.save_aggregation_templates(&templates).await?;
        Ok(template)
    }

    /// DUAL-08-13: delete a saved template, reporting whether one existed.
    pub async fn delete_template(&self, name: &str) -> Result<bool, Failure> {
        let mut templates = self.list_templates().await?;
        let before = templates.len();
        templates.retain(|template| template.name != name);
        if templates.len() == before {
            return Ok(false);
        }
        self.profile.save_aggregation_templates(&templates).await?;
        Ok(true)
    }

    /// The saved template with this exact name, if any.
    pub async fn find_template(&self, name: &str) -> Result<Option<AggregationTemplate>, Failure> {
        Ok(self
            .list_templates()
            .await?
            .into_iter()
            .find(|template| template.name == name))
    }

    /// DUAL-08-07: re-read every source of a saved template, re-run the shared
    /// aggregation and overwrite the previously generated profile in place.
    /// Only profiles that the template itself produced are touched.
    pub async fn reaggregate<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        template_name: &str,
    ) -> Result<AggregatedProfileOutcome, Failure> {
        let template = self.find_template(template_name).await?.ok_or_else(|| {
            Failure::new(
                ErrorCode::Configuration,
                format!("aggregation template `{template_name}` is not saved"),
                false,
            )
        })?;
        let name = sanitize_profile_name(&template.draft.target_name)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let existing = self.profile.list_profiles().await?;
        if !existing.iter().any(|profile| profile.name == name) {
            return Err(Failure::new(
                ErrorCode::Configuration,
                format!("aggregated profile `{name}` no longer exists"),
                false,
            ));
        }

        let report = self.preview(&template.draft).await?;
        self.profile.save_profile(&name, &report.yaml).await?;
        let (activated, core_reloaded) = if template.draft.activate_after_create {
            self.activate_generated_profile(runtime, &name).await?
        } else {
            (false, false)
        };

        Ok(AggregatedProfileOutcome {
            profile_name: name,
            report,
            activated,
            core_reloaded,
            template_name: Some(template.name),
        })
    }

    /// Switch to a freshly generated profile through the shared activation
    /// path, returning `(activated, core_reloaded)`.
    async fn activate_generated_profile<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        name: &str,
    ) -> Result<(bool, bool), Failure> {
        match runtime {
            Some(runtime) => Ok((
                true,
                self.profile.activate_profile(Some(runtime), name).await?,
            )),
            None => {
                self.profile.select_profile(name).await?;
                Ok((true, false))
            }
        }
    }
}

fn report_from_plan(draft: &AggregationDraft, plan: AggregationPlan) -> AggregationReport {
    AggregationReport {
        draft: draft.clone(),
        source_count: plan.source_count,
        missing_sources: Vec::new(),
        input_nodes: plan.input_nodes,
        total_nodes: plan.total_nodes,
        duplicates_removed: plan.duplicates_removed,
        renamed_nodes: plan.renamed_nodes,
        rule_renamed_nodes: plan.rule_renamed_nodes,
        invalid_nodes_removed: plan.invalid_nodes_removed,
        invalid_node_samples: plan.invalid_node_samples,
        regions: plan
            .regions
            .into_iter()
            .map(|region| {
                let group_name = region.group_name();
                RegionalClusterSnapshot {
                    iso: region.iso,
                    label: region.label,
                    flag: region.flag,
                    group_name,
                    node_names: region.node_names,
                }
            })
            .collect(),
        groups: plan
            .groups
            .into_iter()
            .map(|group| GeneratedGroupSnapshot {
                name: group.name,
                group_type: group.group_type,
                is_master: group.is_master,
                is_custom: group.is_custom,
                members: group.members,
            })
            .collect(),
        yaml: plan.yaml,
        generated_at: Utc::now().to_rfc3339(),
    }
}
