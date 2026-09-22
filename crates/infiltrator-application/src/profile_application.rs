//! Profile use-cases over the runtime-neutral profile store port.
//!
//! This module owns profile identity validation, projections, activation, and
//! the boundary between a stored profile and a live managed runtime. Concrete
//! config managers and filesystem details stay in outbound adapters.

use chrono::Utc;
use futures_util::stream::{self, StreamExt};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::subscription_import::{
    CoreReloadOutcome, SubscriptionBatchReport, SubscriptionImportChannel,
    SubscriptionImportReport, SubscriptionQuotaFacts, SubscriptionScheduleDraft,
    SubscriptionUpdateOutcome, SubscriptionUpdateReport,
};
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::filter::FilterReport;
use infiltrator_domain::profile_options::{FilterSpec, ProfileOptions};
use infiltrator_domain::profiles::{
    ProfileDetail, ProfileInfo, ProfileMetadata, sanitize_profile_name,
};
use infiltrator_domain::subscription::{CheckedSubscriptionUrl, SubscriptionUserInfo};
use infiltrator_domain::subscription_scheduler_policy::{
    CronSchedule, FormatDetector, QuotaWarningPolicy, SubscriptionSchedule,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use infiltrator_ports::subscription_source::{
    ConditionalDocumentResult, ConditionalFetchHeaders, SubscriptionSource,
};
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProfileApplication {
    store: Arc<dyn ProfileStore>,
}

impl ProfileApplication {
    pub fn new(store: Arc<dyn ProfileStore>) -> Self {
        Self { store }
    }

    pub fn config_dir(&self) -> PathBuf {
        self.store.config_dir()
    }

    pub async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, Failure> {
        self.store.list_profiles().await.map_err(Failure::from)
    }

    pub async fn current_profile(&self) -> Result<String, Failure> {
        self.store.get_current().await.map_err(Failure::from)
    }

    pub async fn current_content(&self) -> Result<(String, String), Failure> {
        let profile = self.current_profile().await?;
        let content = self.store.load(&profile).await.map_err(Failure::from)?;
        Ok((profile, content))
    }

    pub async fn load_profile_info(&self, name: &str) -> Result<ProfileInfo, Failure> {
        let name = valid_name(name)?;
        self.list_profiles()
            .await?
            .into_iter()
            .find(|profile| profile.name == name)
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::Configuration,
                    format!("profile `{name}` was not found"),
                    false,
                )
            })
    }

    pub async fn load_profile_detail(&self, name: &str) -> Result<ProfileDetail, Failure> {
        let profile = self.load_profile_info(name).await?;
        let content = self
            .store
            .load(&profile.name)
            .await
            .map_err(Failure::from)?;
        Ok(ProfileDetail {
            name: profile.name,
            active: profile.active,
            path: profile.path,
            content,
            subscription_url: profile.subscription_url,
            auto_update_enabled: profile.auto_update_enabled,
            update_interval_hours: profile.update_interval_hours,
            last_updated: profile.last_updated,
            next_update: profile.next_update,
            traffic_upload: profile.traffic_upload,
            traffic_download: profile.traffic_download,
            traffic_total: profile.traffic_total,
            expire_at: profile.expire_at,
        })
    }

    pub async fn select_profile(&self, name: &str) -> Result<ProfileInfo, Failure> {
        let name = valid_name(name)?;
        self.store.set_current(&name).await.map_err(Failure::from)?;
        self.load_profile_info(&name).await
    }

    pub async fn delete_profile(&self, name: &str) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store
            .delete_profile(&name)
            .await
            .map_err(Failure::from)?;
        self.store
            .delete_options(&name)
            .await
            .map_err(Failure::from)
    }

    pub async fn load_metadata(&self, name: &str) -> Result<ProfileMetadata, Failure> {
        let name = valid_name(name)?;
        self.store
            .get_profile_metadata(&name)
            .await
            .map_err(Failure::from)
    }

    pub async fn update_metadata(
        &self,
        name: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store
            .update_profile_metadata(&name, metadata)
            .await
            .map_err(Failure::from)
    }

    pub async fn save_profile(&self, name: &str, content: &str) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store.save(&name, content).await.map_err(Failure::from)
    }

    pub async fn clear_backup(&self, name: &str) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store.clear_backup(&name).await.map_err(Failure::from)
    }

    /// DUAL-07-13: restore the transient pre-save `.bak` copy of a profile over
    /// its current file, returning whether a backup existed and was restored.
    /// The store validates the backup before writing, so a corrupt backup is
    /// rejected instead of silently clobbering the live configuration.
    pub async fn restore_backup(&self, name: &str) -> Result<bool, Failure> {
        let name = valid_name(name)?;
        self.store
            .restore_backup(&name)
            .await
            .map_err(Failure::from)
    }

    pub async fn delete_subscription_credential(&self, name: &str) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store
            .delete_subscription_credential(&name)
            .await
            .map_err(Failure::from)
    }

    pub async fn import_subscription<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        name: &str,
        url: &str,
    ) -> Result<ProfileInfo, Failure> {
        let report = self.import_subscription_report(source, name, url).await?;
        self.load_profile_info(&report.profile_name).await
    }

    /// DUAL-07-01: import a subscription URL and report the detected format and
    /// node count alongside the saved profile name.
    pub async fn import_subscription_report<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        name: &str,
        url: &str,
    ) -> Result<SubscriptionImportReport, Failure> {
        let name = valid_name(name)?;
        let url = CheckedSubscriptionUrl::parse(url)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let document = source.fetch(&name, &url).await.map_err(Failure::from)?;
        let content_bytes = document.content.len();
        let format = FormatDetector::detect(&document.content);
        let node_count = FormatDetector::count_nodes(&document.content);
        commit_subscription_document(&self.store, &name, url.as_str(), document).await?;
        Ok(SubscriptionImportReport {
            profile_name: name,
            channel: SubscriptionImportChannel::Url,
            format,
            node_count,
            content_bytes,
        })
    }

    /// DUAL-07-01: import an already-read document (local file / clipboard),
    /// normalizing URI/Base64 payloads through the shared converter and
    /// validating the result before it is committed.
    pub async fn import_document(
        &self,
        name: &str,
        content: &str,
        channel: SubscriptionImportChannel,
    ) -> Result<SubscriptionImportReport, Failure> {
        let name = valid_name(name)?;
        let converted =
            infiltrator_domain::profile_converter::ProfileConverter::detect_and_convert(content)
                .unwrap_or_else(|_| content.to_string());
        if converted.trim().is_empty() {
            return Err(Failure::new(
                ErrorCode::Configuration,
                "imported document is empty",
                false,
            ));
        }
        infiltrator_domain::config::validate_yaml(&converted)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        let format = FormatDetector::detect(&converted);
        let node_count = FormatDetector::count_nodes(&converted);
        let content_bytes = converted.len();
        self.store
            .save(&name, &converted)
            .await
            .map_err(Failure::from)?;
        Ok(SubscriptionImportReport {
            profile_name: name,
            channel,
            format,
            node_count,
            content_bytes,
        })
    }

    /// DUAL-07-08: load a profile's option sidecar (filter + mixin).
    pub async fn load_options(&self, name: &str) -> Result<ProfileOptions, Failure> {
        let name = valid_name(name)?;
        self.store.load_options(&name).await.map_err(Failure::from)
    }

    /// DUAL-07-08: persist a profile's option sidecar.
    pub async fn save_options(&self, name: &str, options: &ProfileOptions) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store
            .save_options(&name, options)
            .await
            .map_err(Failure::from)
    }

    /// DUAL-08-13: load the persisted aggregation template library.
    pub async fn load_aggregation_templates(
        &self,
    ) -> Result<Vec<infiltrator_contract::aggregator::AggregationTemplate>, Failure> {
        self.store
            .load_aggregation_templates()
            .await
            .map_err(Failure::from)
    }

    /// DUAL-08-13: persist the aggregation template library.
    pub async fn save_aggregation_templates(
        &self,
        templates: &[infiltrator_contract::aggregator::AggregationTemplate],
    ) -> Result<(), Failure> {
        self.store
            .save_aggregation_templates(templates)
            .await
            .map_err(Failure::from)
    }

    /// DUAL-07-08: run the shared node-keyword filter pipeline over a profile's
    /// stored document, commit the filtered content through the managed
    /// runtime seam, and persist the spec so the next subscription update
    /// recomposes from the same node strategy.
    pub async fn apply_subscription_filter<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        name: &str,
        spec: FilterSpec,
    ) -> Result<FilterReport, Failure> {
        let name = valid_name(name)?;
        let rule = spec
            .to_rule()
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        let content = self.store.load(&name).await.map_err(Failure::from)?;
        let (filtered, report) = infiltrator_domain::filter::SubscriptionFilterPipeline::new(rule)
            .apply_to_yaml(&content)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        infiltrator_domain::config::validate_yaml(&filtered)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        self.save_profile_content(runtime, name.clone(), filtered, ApplyStrategy::PreferReload)
            .await?;
        let mut options = self
            .store
            .load_options(&name)
            .await
            .map_err(Failure::from)?;
        options.filter = Some(spec);
        self.store
            .save_options(&name, &options)
            .await
            .map_err(Failure::from)?;
        Ok(report)
    }

    pub async fn update_subscription<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        name: &str,
    ) -> Result<ProfileInfo, Failure> {
        let name = valid_name(name)?;
        self.update_subscription_conditional(source, &name).await?;
        self.load_profile_info(&name).await
    }

    /// Conditional subscription update: send the profile's stored validators
    /// (`ETag` / `If-Modified-Since`), its custom `User-Agent`, and its
    /// insecure-TLS preference, commit content only on `200`, and return the
    /// shared [`SubscriptionUpdateReport`]. A `304 Not Modified` refreshes
    /// quota facts and the next-update schedule without rewriting the file.
    pub async fn update_subscription_conditional<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        name: &str,
    ) -> Result<SubscriptionUpdateReport, Failure> {
        let name = valid_name(name)?;
        let mut metadata = self.load_metadata(&name).await?;
        let url = metadata.subscription_url.as_deref().ok_or_else(|| {
            Failure::new(
                ErrorCode::Configuration,
                "subscription URL is missing",
                false,
            )
        })?;
        let url = CheckedSubscriptionUrl::parse(url)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;

        let headers = ConditionalFetchHeaders {
            etag: metadata.etag.clone(),
            if_modified_since: metadata.last_modified.clone(),
            custom_user_agent: metadata.user_agent.clone(),
            insecure_skip_verify: metadata.insecure_skip_verify,
        };
        let result = source
            .fetch_conditional(&name, &url, &headers)
            .await
            .map_err(Failure::from)?;

        let now = Utc::now();
        match result {
            ConditionalDocumentResult::Modified {
                document,
                etag,
                last_modified,
            } => {
                if document.content.trim().is_empty() {
                    return Err(Failure::new(
                        ErrorCode::Configuration,
                        "subscription returned empty content",
                        false,
                    ));
                }
                infiltrator_domain::config::validate_yaml(&document.content).map_err(|error| {
                    Failure::new(ErrorCode::Configuration, error.to_string(), false)
                })?;
                let new_bytes = document.content.len();
                let node_count = FormatDetector::count_nodes(&document.content);
                self.store
                    .save(&name, &document.content)
                    .await
                    .map_err(Failure::from)?;

                apply_subscription_metadata(&mut metadata, document.userinfo, now);
                metadata.etag = etag.clone().or(metadata.etag);
                metadata.last_modified = last_modified.clone().or(metadata.last_modified);
                self.update_metadata(&name, &metadata).await?;

                Ok(report_from_metadata(
                    name,
                    metadata,
                    SubscriptionUpdateOutcome::Updated {
                        new_bytes,
                        node_count,
                    },
                    etag,
                    last_modified,
                    true,
                    now,
                ))
            }
            ConditionalDocumentResult::NotModified {
                userinfo,
                etag,
                last_modified,
            } => {
                // Content is unchanged: refresh quota facts and the schedule,
                // but keep `last_updated` pointing at the last real download.
                apply_subscription_quota(&mut metadata, userinfo);
                schedule_next_update(&mut metadata, now);
                metadata.etag = etag.clone().or(metadata.etag);
                metadata.last_modified = last_modified.clone().or(metadata.last_modified);
                self.update_metadata(&name, &metadata).await?;

                Ok(report_from_metadata(
                    name,
                    metadata,
                    SubscriptionUpdateOutcome::NotModified {
                        etag: etag.clone(),
                        last_modified: last_modified.clone(),
                    },
                    etag,
                    last_modified,
                    false,
                    now,
                ))
            }
        }
    }

    /// DUAL-07-11: one-click "update every subscription now" over the shared
    /// conditional path. Every profile carrying a non-empty subscription URL
    /// is refreshed regardless of its schedule, with bounded concurrency so a
    /// large profile set does not open an unbounded number of sockets. The
    /// per-profile outcomes are aggregated into one shared
    /// [`SubscriptionBatchReport`] so both surfaces render the same counts and
    /// neither re-implements the batch.
    pub async fn update_all_subscriptions<S: SubscriptionSource + ?Sized>(
        &self,
        source: &S,
        concurrency: usize,
    ) -> Result<SubscriptionBatchReport, Failure> {
        let profiles = self.list_profiles().await?;
        let total = profiles.len();
        let targets: Vec<String> = profiles
            .iter()
            .filter(|profile| {
                profile
                    .subscription_url
                    .as_deref()
                    .is_some_and(|url| !url.trim().is_empty())
            })
            .map(|profile| profile.name.clone())
            .collect();
        let mut report = SubscriptionBatchReport {
            total,
            skipped: total - targets.len(),
            ..Default::default()
        };

        let outcomes = stream::iter(targets.into_iter().map(|name| async move {
            let result = self.update_subscription_conditional(source, &name).await;
            (name, result)
        }))
        .buffer_unordered(concurrency.max(1))
        .collect::<Vec<_>>()
        .await;

        for (name, result) in outcomes {
            match result {
                Ok(outcome_report) => {
                    match &outcome_report.outcome {
                        SubscriptionUpdateOutcome::Updated { .. } => report.updated += 1,
                        SubscriptionUpdateOutcome::NotModified { .. } => report.not_modified += 1,
                        SubscriptionUpdateOutcome::Failed { .. } => report.failed += 1,
                    }
                    report.outcomes.push(outcome_report);
                }
                Err(failure) => {
                    report.failed += 1;
                    report.outcomes.push(SubscriptionUpdateReport {
                        profile_name: name,
                        outcome: SubscriptionUpdateOutcome::Failed {
                            error: failure.message,
                            attempts: 1,
                        },
                        etag: None,
                        last_modified: None,
                        quota: None,
                        usage_warning: false,
                        expiry_warning: false,
                        core_reload: CoreReloadOutcome::NotAttempted,
                        backed_up: false,
                    });
                }
            }
        }

        Ok(report)
    }

    /// Persist the per-profile subscription fetch options (custom User-Agent
    /// and insecure-TLS preference) used by the next conditional update.
    pub async fn update_subscription_fetch_settings(
        &self,
        name: &str,
        user_agent: Option<String>,
        insecure_skip_verify: bool,
    ) -> Result<(), Failure> {
        let name = valid_name(name)?;
        let mut metadata = self.load_metadata(&name).await?;
        metadata.user_agent = user_agent.filter(|value| !value.trim().is_empty());
        metadata.insecure_skip_verify = insecure_skip_verify;
        self.update_metadata(&name, &metadata).await
    }

    /// DUAL-07-14: persist a profile's subscription URL, auto-update flag,
    /// interval, and cron schedule as one validated draft.
    ///
    /// All validation happens here so every surface (and the command bus) sees
    /// the same typed rejection: a malformed cron expression, an auto-update
    /// request without a URL, or a zero interval never reaches the store. An
    /// empty URL clears the subscription, its schedule, and the last-update
    /// marker.
    pub async fn update_subscription_schedule(
        &self,
        name: &str,
        draft: &SubscriptionScheduleDraft,
    ) -> Result<(), Failure> {
        let name = valid_name(name)?;
        let cron_expression = match draft
            .cron_expression
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(raw) => Some(
                CronSchedule::parse(raw)
                    .map_err(|error| {
                        Failure::new(ErrorCode::InvalidInput, error.to_string(), false)
                    })?
                    .raw,
            ),
            None => None,
        };
        let url = draft.url.trim();
        if url.is_empty() && draft.auto_update_enabled {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "a subscription URL is required when auto update is enabled",
                false,
            ));
        }
        // The interval arrives as free text (the surface field); an
        // unparseable or zero value is a typed rejection, never a silent
        // default.
        let interval_text = draft.update_interval_hours.trim();
        let requested_interval = if interval_text.is_empty() {
            None
        } else {
            match interval_text.parse::<u32>() {
                Ok(0) => {
                    return Err(Failure::new(
                        ErrorCode::InvalidInput,
                        "update interval must be a positive number of hours",
                        false,
                    ));
                }
                Ok(hours) => Some(hours),
                Err(_) => {
                    return Err(Failure::new(
                        ErrorCode::InvalidInput,
                        "update interval must be a whole number of hours",
                        false,
                    ));
                }
            }
        };
        let mut metadata = self.load_metadata(&name).await?;

        if url.is_empty() {
            metadata.subscription_url = None;
            metadata.auto_update_enabled = false;
            metadata.update_interval_hours = None;
            metadata.cron_expression = None;
            metadata.last_updated = None;
            metadata.next_update = None;
        } else {
            // An interval-less schedule is only valid when a cron expression
            // carries the cadence; otherwise the 24h default applies, exactly
            // like the previous editor default.
            let interval_hours = if draft.auto_update_enabled {
                match requested_interval {
                    Some(hours) => Some(hours),
                    None if cron_expression.is_none() => Some(24),
                    None => None,
                }
            } else {
                None
            };
            metadata.subscription_url = Some(url.to_string());
            metadata.auto_update_enabled = draft.auto_update_enabled;
            metadata.update_interval_hours = interval_hours;
            metadata.cron_expression = cron_expression;
            metadata.next_update = None;
        }
        self.update_metadata(&name, &metadata).await
    }

    /// DUAL-07-09: persist whether a successful update of this profile is
    /// applied to the running core. The preference is consumed by the shared
    /// subscription refresh through the host's `CoreReloadPort`.
    pub async fn update_subscription_auto_reload(
        &self,
        name: &str,
        enabled: bool,
    ) -> Result<(), Failure> {
        let name = valid_name(name)?;
        let mut metadata = self.load_metadata(&name).await?;
        metadata.auto_reload_core = enabled;
        self.update_metadata(&name, &metadata).await
    }

    /// Commit an arbitrary profile document through the managed runtime when
    /// the document belongs to the active profile. Inactive profiles use the
    /// store writer and clear their transient backup after a successful save.
    pub async fn save_profile_content<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        profile: String,
        content: String,
        strategy: ApplyStrategy,
    ) -> Result<(), Failure> {
        let profile = valid_name(&profile)?;
        let current = self.store.get_current().await.map_err(Failure::from)?;
        if let Some(runtime) = runtime
            && current == profile
        {
            runtime
                .apply_profile_content(&content, strategy)
                .await
                .map(|_| ())
                .map_err(Failure::from)
        } else {
            self.store
                .save(&profile, &content)
                .await
                .map_err(Failure::from)?;
            self.store
                .clear_backup(&profile)
                .await
                .map_err(Failure::from)
        }
    }

    pub async fn save_current_profile_content<F, E, R>(
        &self,
        runtime: Option<Arc<R>>,
        strategy: ApplyStrategy,
        transform: F,
    ) -> Result<(), Failure>
    where
        F: FnOnce(&str) -> Result<String, E> + Send + 'static,
        E: Display + Send + 'static,
        R: ManagedRuntime + ?Sized,
    {
        let profile = self.store.get_current().await.map_err(Failure::from)?;
        let content = self.store.load(&profile).await.map_err(Failure::from)?;
        let updated = transform(&content)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        self.save_profile_content(runtime, profile, updated, strategy)
            .await
    }

    /// Switch the active profile and re-apply it to a running core. A failed
    /// apply restores both the profile pointer and the previous live config.
    pub async fn activate_profile<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        profile: &str,
    ) -> Result<bool, Failure> {
        let profile = valid_name(profile)?;
        let previous = self.store.get_current().await.map_err(Failure::from)?;
        if previous == profile {
            return Ok(runtime.is_some());
        }

        self.store
            .set_current(&profile)
            .await
            .map_err(Failure::from)?;

        let Some(runtime) = runtime else {
            return Ok(false);
        };

        if let Err(error) = runtime
            .apply_current_config(ApplyStrategy::AlwaysRestart)
            .await
        {
            let _ = self.store.set_current(&previous).await;
            if let Err(recovery) = runtime
                .apply_current_config(ApplyStrategy::AlwaysRestart)
                .await
            {
                let _ = self.store.clear_backup(&profile).await;
                return Err(Failure::new(
                    ErrorCode::Internal,
                    format!("profile switch failed: {error}; recovery also failed: {recovery}"),
                    false,
                ));
            }
            let _ = self.store.clear_backup(&profile).await;
            return Err(Failure::new(
                ErrorCode::Internal,
                format!("profile switch failed; previous profile restored: {error}"),
                false,
            ));
        }

        Ok(true)
    }
}

fn valid_name(name: &str) -> Result<String, Failure> {
    sanitize_profile_name(name)
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))
}

async fn commit_subscription_document(
    store: &Arc<dyn ProfileStore>,
    name: &str,
    source_url: &str,
    document: infiltrator_ports::subscription_source::SubscriptionDocument,
) -> Result<(), Failure> {
    if document.content.trim().is_empty() {
        return Err(Failure::new(
            ErrorCode::Configuration,
            "subscription returned empty content",
            false,
        ));
    }
    infiltrator_domain::config::validate_yaml(&document.content)
        .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
    store
        .save(name, &document.content)
        .await
        .map_err(Failure::from)?;

    let mut metadata = store
        .get_profile_metadata(name)
        .await
        .map_err(Failure::from)?;
    metadata.subscription_url = Some(source_url.to_owned());
    apply_subscription_metadata(&mut metadata, document.userinfo, Utc::now());
    store
        .update_profile_metadata(name, &metadata)
        .await
        .map_err(Failure::from)
}

fn apply_subscription_metadata(
    metadata: &mut infiltrator_domain::profiles::ProfileMetadata,
    userinfo: Option<infiltrator_domain::subscription::SubscriptionUserInfo>,
    now: chrono::DateTime<Utc>,
) {
    apply_subscription_quota(metadata, userinfo);
    metadata.last_updated = Some(now);
    schedule_next_update(metadata, now);
}

fn apply_subscription_quota(
    metadata: &mut infiltrator_domain::profiles::ProfileMetadata,
    userinfo: Option<infiltrator_domain::subscription::SubscriptionUserInfo>,
) {
    if let Some(info) = userinfo {
        metadata.traffic_upload = info.upload;
        metadata.traffic_download = info.download;
        metadata.traffic_total = info.total;
        metadata.expire_at = info.expire;
    }
}

fn schedule_next_update(
    metadata: &mut infiltrator_domain::profiles::ProfileMetadata,
    now: chrono::DateTime<Utc>,
) {
    if !metadata.auto_update_enabled {
        metadata.next_update = None;
        return;
    }
    // DUAL-07-03: a profile may carry either a fixed interval or a cron
    // expression; the domain schedule picks cron first and reports a malformed
    // expression as an error instead of silently falling back to the interval.
    metadata.next_update = SubscriptionSchedule::from_metadata(
        metadata.update_interval_hours,
        metadata.cron_expression.as_deref(),
    )
    .ok()
    .and_then(|schedule| schedule.next_run(now));
}

fn report_from_metadata(
    profile_name: String,
    metadata: ProfileMetadata,
    outcome: SubscriptionUpdateOutcome,
    etag: Option<String>,
    last_modified: Option<String>,
    backed_up: bool,
    now: chrono::DateTime<Utc>,
) -> SubscriptionUpdateReport {
    let info = SubscriptionUserInfo {
        upload: metadata.traffic_upload,
        download: metadata.traffic_download,
        total: metadata.traffic_total,
        expire: metadata.expire_at,
    };
    let (usage_warning, expiry_warning) = QuotaWarningPolicy::evaluate(&info, now.timestamp());
    SubscriptionUpdateReport {
        profile_name,
        outcome,
        etag,
        last_modified,
        quota: Some(SubscriptionQuotaFacts {
            upload_bytes: metadata.traffic_upload.unwrap_or_default(),
            download_bytes: metadata.traffic_download.unwrap_or_default(),
            total_bytes: metadata.traffic_total.unwrap_or_default(),
            expire_at_unix: metadata.expire_at,
        }),
        usage_warning,
        expiry_warning,
        core_reload: CoreReloadOutcome::NotAttempted,
        backed_up,
    }
}

#[cfg(test)]
#[path = "profile_application_tests.rs"]
mod tests;
