use super::*;

impl ProfileApplication {
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
        ensure_profile_inactive(&self.store, &name).await?;
        let url = CheckedSubscriptionUrl::parse(url)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let document = source.fetch(&name, &url).await.map_err(Failure::from)?;
        let profile_id = profile_id_for_name(&name)?;
        let prepared_content = self
            .prepare_subscription_content(&profile_id, &document.content)
            .await?;
        let content = prepared_content.as_str();
        let content_bytes = content.len();
        let format = FormatDetector::detect(content);
        let node_count = FormatDetector::count_nodes(content);
        commit_subscription_document(&self.store, &name, url.as_str(), &document, content).await?;
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
        ensure_profile_inactive(&self.store, &name).await?;
        let converted =
            ProfileConverter::detect_and_convert(content).unwrap_or_else(|_| content.to_string());
        if converted.trim().is_empty() {
            return Err(Failure::new(
                ErrorCode::Configuration,
                "imported document is empty",
                false,
            ));
        }
        validate_yaml(&converted)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        let format = FormatDetector::detect(&converted);
        let node_count = FormatDetector::count_nodes(&converted);
        let content_bytes = converted.len();
        self.store
            .save_inactive_profile(&name, &converted)
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

    /// DUAL-08-13: load the persisted aggregation template library.
    pub async fn load_aggregation_templates(&self) -> Result<Vec<AggregationTemplate>, Failure> {
        self.store
            .load_aggregation_templates()
            .await
            .map_err(Failure::from)
    }

    /// DUAL-08-13: persist the aggregation template library.
    pub async fn save_aggregation_templates(
        &self,
        templates: &[AggregationTemplate],
    ) -> Result<(), Failure> {
        self.store
            .save_aggregation_templates(templates)
            .await
            .map_err(Failure::from)
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
        self.update_subscription_conditional_with_access(
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            source,
            name,
        )
        .await
    }

    /// Refresh a subscription through one exact active-workspace transaction.
    /// The selected reference and full opening-time workspace identity are
    /// captured before the network request; stale selection or content edits
    /// therefore fail before a write or native apply can be redirected.
    pub async fn update_subscription_conditional_with_access<
        R: ActiveProfileReferenceApplier + ?Sized,
        S: SubscriptionSource + ?Sized,
    >(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        source: &S,
        name: &str,
    ) -> Result<SubscriptionUpdateReport, Failure> {
        let name = valid_name(name)?;
        let mut metadata = self.load_metadata(&name).await?;
        let expected_reference = self.active_profile_reference().await?;
        let target = match &expected_reference {
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.store.engine_id()
                    && reference.profile_id.as_str() == name =>
            {
                let workspace = self.load_workspace(&reference.profile_id).await?;
                if workspace.identity().document.engine_id != reference.engine_id
                    || workspace.identity().document.profile_id != reference.profile_id
                {
                    return Err(Failure::new(
                        ErrorCode::NotReady,
                        "selected reference does not match the subscription workspace",
                        true,
                    ));
                }
                let observed = self.active_profile_reference().await?;
                if observed != expected_reference {
                    return Err(stale_reference_failure(&expected_reference, &observed));
                }
                let (effect, outcome) = match runtime.as_ref() {
                    ProfileRuntimeAccessRef::Offline => (
                        ProfileWorkspaceEffect::StoreForNextStart(
                            ProfileWorkspaceStoreReason::Offline,
                        ),
                        SubscriptionContentEffectOutcome::StoredForNextStart {
                            reason: StoreForNextStartReason::Offline,
                        },
                    ),
                    ProfileRuntimeAccessRef::CoreOwner(runtime) => {
                        let runtime_engine = runtime.engine_id().ok_or_else(|| {
                            Failure::unsupported(
                                "selected subscription refresh requires an authoritative Core owner",
                            )
                        })?;
                        if runtime_engine != reference.engine_id {
                            return Err(Failure::unsupported(
                                "subscription runtime does not own the selected profile engine",
                            ));
                        }
                        let authorization = runtime.write_authorization().map_err(Failure::from)?;
                        ensure_bound_owner_authorization(authorization, &reference.engine_id)?;
                        match (runtime.core_lifecycle(), metadata.auto_reload_core) {
                            (CoreLifecycle::Ready | CoreLifecycle::Running, true) => (
                                ProfileWorkspaceEffect::Apply(ApplyStrategy::PreferReload),
                                SubscriptionContentEffectOutcome::Applied,
                            ),
                            (CoreLifecycle::Ready | CoreLifecycle::Running, false) => (
                                ProfileWorkspaceEffect::StoreForNextStart(
                                    ProfileWorkspaceStoreReason::AutoReloadDisabled,
                                ),
                                SubscriptionContentEffectOutcome::StoredForNextStart {
                                    reason: StoreForNextStartReason::AutoReloadDisabled,
                                },
                            ),
                            (CoreLifecycle::Stopped, _) => (
                                ProfileWorkspaceEffect::StoreForNextStart(
                                    ProfileWorkspaceStoreReason::CoreStopped,
                                ),
                                SubscriptionContentEffectOutcome::StoredForNextStart {
                                    reason: StoreForNextStartReason::CoreStopped,
                                },
                            ),
                            (CoreLifecycle::Failed, _) => (
                                ProfileWorkspaceEffect::StoreForNextStart(
                                    ProfileWorkspaceStoreReason::CoreFailed,
                                ),
                                SubscriptionContentEffectOutcome::StoredForNextStart {
                                    reason: StoreForNextStartReason::CoreFailed,
                                },
                            ),
                            (CoreLifecycle::Starting | CoreLifecycle::Stopping, _) => {
                                return Err(Failure::new(
                                    ErrorCode::InvalidState,
                                    "Core lifecycle is transitioning; subscription content was not changed",
                                    true,
                                ));
                            }
                        }
                    }
                };
                SubscriptionProfileTarget::Selected {
                    state: expected_reference.clone(),
                    workspace,
                    effect,
                    outcome,
                }
            }
            ActiveProfileReferenceState::Unselected { .. }
            | ActiveProfileReferenceState::Selected(_) => SubscriptionProfileTarget::Inactive,
        };
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
                let profile_id = profile_id_for_name(&name)?;
                let prepared_content = self
                    .prepare_subscription_content(&profile_id, &document.content)
                    .await?;
                let content = prepared_content.as_str();
                if content.trim().is_empty() {
                    return Err(Failure::new(
                        ErrorCode::Configuration,
                        "subscription returned empty content",
                        false,
                    ));
                }
                validate_yaml(content).map_err(|error| {
                    Failure::new(ErrorCode::Configuration, error.to_string(), false)
                })?;
                let new_bytes = content.len();
                let node_count = FormatDetector::count_nodes(content);
                match &target {
                    SubscriptionProfileTarget::Selected {
                        state,
                        workspace,
                        effect,
                        ..
                    } => {
                        let update = ProfileWorkspaceUpdate {
                            purpose: ProfileWorkspacePurpose::Derived,
                            document_bytes: content.as_bytes().to_vec(),
                            sidecar_bytes: workspace.sidecar_bytes.clone(),
                        };
                        self.commit_workspace_at_reference_with_effect(
                            runtime,
                            state.clone(),
                            &workspace.identity(),
                            &update,
                            *effect,
                        )
                        .await?;
                    }
                    SubscriptionProfileTarget::Inactive => {
                        self.store
                            .save_inactive_profile(&name, content)
                            .await
                            .map_err(Failure::from)?;
                    }
                }

                apply_subscription_metadata(&mut metadata, document.userinfo, now);
                metadata.etag = etag.clone().or(metadata.etag);
                metadata.last_modified = last_modified.clone().or(metadata.last_modified);
                self.update_metadata(&name, &metadata).await?;

                let mut report = report_from_metadata(
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
                );
                report.content_effect = match &target {
                    SubscriptionProfileTarget::Inactive => {
                        SubscriptionContentEffectOutcome::NotActive
                    }
                    SubscriptionProfileTarget::Selected { outcome, .. } => outcome.clone(),
                };
                Ok(report)
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
                        content_effect: SubscriptionContentEffectOutcome::NotAttempted,
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

    /// Persist whether a selected subscription content update should apply to
    /// the owned Core or be stored for the next start.
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
}
