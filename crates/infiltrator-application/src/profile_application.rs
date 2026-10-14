//! Profile use-cases over the runtime-neutral profile store port.
//!
//! This module owns profile identity validation, projections, activation, and
//! the boundary between a stored profile and a live Core owner. Concrete
//! config managers and filesystem details stay in outbound adapters.

use crate::profile_editor_observations::EditorObservations;
use chrono::Utc;
use futures_util::stream;
use futures_util::stream::StreamExt;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileRecoveryOutcome, ActiveProfileReferenceCasOutcome, ActiveProfileReferenceRevision,
    ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization, CoreApplyFailureKind,
};
use infiltrator_contract::aggregator::AggregationTemplate;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::error::{ErrorCode, Failure, FailureReason};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::snapshot::{CoreInactiveLifecycle, CoreLifecycle};
use infiltrator_contract::subscription_import::{
    StoreForNextStartReason, SubscriptionBatchReport, SubscriptionContentEffectOutcome,
    SubscriptionImportChannel, SubscriptionImportReport, SubscriptionQuotaFacts,
    SubscriptionScheduleDraft, SubscriptionUpdateOutcome, SubscriptionUpdateReport,
};
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::config::validate_yaml;
use infiltrator_domain::profile_converter::ProfileConverter;
use infiltrator_domain::profiles::{
    ProfileDetail, ProfileInfo, ProfileMetadata, sanitize_profile_name,
};
use infiltrator_domain::subscription::{CheckedSubscriptionUrl, SubscriptionUserInfo};
use infiltrator_domain::subscription_scheduler_policy::{
    CronSchedule, FormatDetector, QuotaWarningPolicy, SubscriptionSchedule,
};
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::subscription_source::{
    ConditionalDocumentResult, ConditionalFetchHeaders, SubscriptionDocument, SubscriptionSource,
};
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod document_codec;
mod workspace;
pub use workspace::{ProfileWorkspaceEffect, ProfileWorkspaceStoreReason};

#[derive(Clone)]
pub struct ProfileApplication {
    store: Arc<dyn ProfileStore>,
    document_codec: Option<Arc<dyn ProfileDocumentCodecPort>>,
    pub(crate) editor_observations: Arc<Mutex<EditorObservations>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileActivationOutcome {
    AlreadySelectedWithoutRuntime,
    AlreadySelectedWithRuntime,
    AlreadySelectedWhileInactive(CoreInactiveLifecycle),
    SelectedWithoutRuntime,
    SelectedWhileInactive(CoreInactiveLifecycle),
    SelectedAndApplied,
    CurrentProfileUpdatedAndApplied,
    CurrentProfileUpdatedWhileInactive(CoreInactiveLifecycle),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileContentSaveOutcome {
    Applied,
    StoredWhileInactive(CoreInactiveLifecycle),
    StoredOffline,
    StoredInactive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedProfileApplyOutcome {
    Applied { generation: u64 },
    NotActive(CoreInactiveLifecycle),
}

enum SubscriptionProfileTarget {
    Selected {
        state: ActiveProfileReferenceState,
        workspace: ProfileWorkspace,
        effect: ProfileWorkspaceEffect,
        outcome: SubscriptionContentEffectOutcome,
    },
    Inactive,
}

/// Explicit permission boundary for profile content and activation work.
/// Offline writes are never represented as an absent runtime value.
pub enum ProfileRuntimeAccess<R: ?Sized> {
    Offline,
    CoreOwner(Arc<R>),
}

pub(crate) enum ProfileRuntimeAccessRef<'a, R: ?Sized> {
    Offline,
    CoreOwner(&'a R),
}

impl<R: ?Sized> ProfileRuntimeAccess<R> {
    pub(crate) fn as_ref(&self) -> ProfileRuntimeAccessRef<'_, R> {
        match self {
            Self::Offline => ProfileRuntimeAccessRef::Offline,
            Self::CoreOwner(runtime) => ProfileRuntimeAccessRef::CoreOwner(runtime.as_ref()),
        }
    }
}

impl<R: ?Sized> Clone for ProfileRuntimeAccess<R> {
    fn clone(&self) -> Self {
        match self {
            Self::Offline => Self::Offline,
            Self::CoreOwner(runtime) => Self::CoreOwner(Arc::clone(runtime)),
        }
    }
}

enum ActivationMode<'a, R: ?Sized> {
    Offline,
    Running {
        runtime: &'a R,
        authorization: ActiveProfileReferenceWriteAuthorization,
    },
    Inactive {
        authorization: ActiveProfileReferenceWriteAuthorization,
        lifecycle: CoreInactiveLifecycle,
    },
}

impl<R: ?Sized> ActivationMode<'_, R> {
    fn authorization(&self) -> ActiveProfileReferenceWriteAuthorization {
        match self {
            Self::Offline => ActiveProfileReferenceWriteAuthorization::Unowned,
            Self::Running { authorization, .. } | Self::Inactive { authorization, .. } => {
                authorization.clone()
            }
        }
    }
}

impl ProfileApplication {
    pub fn new(store: Arc<dyn ProfileStore>) -> Self {
        Self {
            store,
            document_codec: None,
            editor_observations: Arc::new(Mutex::new(EditorObservations::default())),
        }
    }

    pub fn apply_transaction(&self, profile: &str) -> Option<ApplyTransactionSnapshot> {
        self.store.apply_transaction(profile)
    }

    pub fn config_dir(&self) -> PathBuf {
        self.store.config_dir()
    }

    pub fn engine_id(&self) -> infiltrator_contract::core_identity::EngineId {
        self.store.engine_id()
    }

    pub async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, Failure> {
        self.store.list_profiles().await.map_err(Failure::from)
    }

    pub async fn current_profile(&self) -> Result<String, Failure> {
        self.store.get_current().await.map_err(Failure::from)
    }

    pub async fn active_profile_reference(&self) -> Result<ActiveProfileReferenceState, Failure> {
        self.store
            .read_active_profile_reference()
            .await
            .map_err(Failure::from)
    }

    /// Apply a previously observed selected reference only if the complete
    /// revisioned state is still current. This path does not own a selection
    /// write, so failures never attempt to restore another operation's state.
    pub async fn apply_selected_profile_reference(
        &self,
        applier: Arc<dyn ActiveProfileReferenceApplier>,
        expected: &ActiveProfileReferenceState,
        strategy: ApplyStrategy,
    ) -> Result<SelectedProfileApplyOutcome, Failure> {
        self.apply_selected_profile_reference_owned(applier.as_ref(), expected, strategy)
            .await
    }

    async fn apply_selected_profile_reference_owned(
        &self,
        applier: &dyn ActiveProfileReferenceApplier,
        expected: &ActiveProfileReferenceState,
        strategy: ApplyStrategy,
    ) -> Result<SelectedProfileApplyOutcome, Failure> {
        let observed = self
            .store
            .read_active_profile_reference()
            .await
            .map_err(Failure::from)?;
        if &observed != expected {
            return Err(stale_reference_failure(expected, &observed));
        }
        let ActiveProfileReferenceState::Selected(reference) = expected else {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "cannot apply a profile while the active reference is unselected",
                false,
            ));
        };
        if reference.engine_id != self.store.engine_id()
            || applier.engine_id().as_ref() != Some(&reference.engine_id)
        {
            return Err(Failure::unsupported(
                "active profile reference does not belong to the selected runtime",
            ));
        }
        ensure_bound_owner_authorization(applier.write_authorization()?, &reference.engine_id)?;
        match applier.core_lifecycle() {
            CoreLifecycle::Ready | CoreLifecycle::Running => applier
                .apply_active_profile_reference(reference, strategy)
                .await
                .map(|generation| SelectedProfileApplyOutcome::Applied { generation })
                .map_err(Failure::from),
            CoreLifecycle::Stopped => Ok(SelectedProfileApplyOutcome::NotActive(
                CoreInactiveLifecycle::Stopped,
            )),
            CoreLifecycle::Failed => Ok(SelectedProfileApplyOutcome::NotActive(
                CoreInactiveLifecycle::Failed,
            )),
            CoreLifecycle::Starting | CoreLifecycle::Stopping => Err(Failure::new(
                ErrorCode::InvalidState,
                "Core lifecycle is transitioning; active-profile apply was not dispatched",
                true,
            )),
        }
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

    /// Resolve a profile selection without publishing it. A live product owner
    /// must apply this target through its scoped Core command or owner-switch
    /// transaction; offline `select_profile` is reserved for unowned flows.
    pub async fn active_reference_target_for(
        &self,
        name: &str,
    ) -> Result<ActiveProfileReferenceTarget, Failure> {
        let profile = self.load_profile_info(name).await?;
        let profile_id = ProfileId::new(profile.name)
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        Ok(ActiveProfileReferenceTarget::selected(
            self.store.engine_id(),
            profile_id,
        ))
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

    /// Persist an offline profile selection when no live Core owner is bound.
    /// Live hosts must resolve a target and submit it through the bound owner.
    pub async fn select_profile(&self, name: &str) -> Result<ProfileInfo, Failure> {
        let name = valid_name(name)?;
        let profile_id = ProfileId::new(name.clone())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let target = ActiveProfileReferenceTarget::selected(self.store.engine_id(), profile_id);
        let expected = self
            .store
            .read_active_profile_reference()
            .await
            .map_err(Failure::from)?;
        if selection_matches_target(&expected, &target) {
            return self.load_profile_info(&name).await;
        }
        match self
            .store
            .compare_and_set_active_profile_reference(
                expected.clone(),
                target.clone(),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
            .map_err(Failure::from)?
        {
            ActiveProfileReferenceCasOutcome::Committed { previous, current } => {
                validate_committed_selection(&expected, &previous, &current, &target)?;
            }
            ActiveProfileReferenceCasOutcome::Stale { expected, observed } => {
                return Err(stale_reference_failure(&expected, &observed));
            }
        }
        self.load_profile_info(&name).await
    }

    pub async fn delete_profile(&self, name: &str) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store
            .delete_profile(&name)
            .await
            .map_err(Failure::from)?;
        self.retire_editor_profile(&name);
        Ok(())
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
        ensure_profile_inactive(&self.store, &name).await?;
        self.store
            .save_inactive_profile(&name, content)
            .await
            .map_err(Failure::from)
    }

    pub async fn clear_backup(&self, name: &str) -> Result<(), Failure> {
        let name = valid_name(name)?;
        self.store.clear_backup(&name).await.map_err(Failure::from)
    }

    /// DUAL-07-13: restore the transient pre-save `.bak` copy of a profile over
    /// its current file, returning whether a backup existed and was restored.
    /// The store validates the backup before writing, so a corrupt backup is
    /// rejected instead of silently clobbering the live configuration.
    pub async fn restore_inactive_profile_backup(&self, name: &str) -> Result<bool, Failure> {
        let name = valid_name(name)?;
        ensure_profile_inactive(&self.store, &name).await?;
        self.store
            .restore_inactive_profile_backup(&name)
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
        ensure_profile_inactive(&self.store, &name).await?;
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
                if document.content.trim().is_empty() {
                    return Err(Failure::new(
                        ErrorCode::Configuration,
                        "subscription returned empty content",
                        false,
                    ));
                }
                validate_yaml(&document.content).map_err(|error| {
                    Failure::new(ErrorCode::Configuration, error.to_string(), false)
                })?;
                let new_bytes = document.content.len();
                let node_count = FormatDetector::count_nodes(&document.content);
                match &target {
                    SubscriptionProfileTarget::Selected {
                        state,
                        workspace,
                        effect,
                        ..
                    } => {
                        let update = ProfileWorkspaceUpdate {
                            purpose: ProfileWorkspacePurpose::Derived,
                            document_bytes: document.content.as_bytes().to_vec(),
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
                            .save_inactive_profile(&name, &document.content)
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

    /// Commit an arbitrary profile document through an explicit offline or
    /// Core-owner access path. Active owner writes are revision-bound; inactive
    /// profiles use the store writer and clear their transient backup.
    pub async fn save_profile_content<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        profile: String,
        content: String,
        strategy: ApplyStrategy,
    ) -> Result<ProfileContentSaveOutcome, Failure> {
        let profile = valid_name(&profile)?;
        let expected = self.active_profile_reference().await?;
        self.save_profile_content_at_reference(runtime, profile, content, strategy, expected)
            .await
    }

    /// Save profile content against the complete observed active-reference
    /// state. Active owner writes apply through a native transaction bound
    /// to that exact reference, while offline/stopped writes use the exact
    /// selected-workspace persistence transaction.
    pub async fn save_profile_content_at_reference<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        profile: String,
        content: String,
        strategy: ApplyStrategy,
        expected: ActiveProfileReferenceState,
    ) -> Result<ProfileContentSaveOutcome, Failure> {
        let profile = valid_name(&profile)?;
        self.save_profile_content_for_reference(runtime, profile, &content, strategy, &expected)
            .await
    }

    /// DUAL-09-12: direct-edit protection derived from the stored subscription
    /// URL. An empty URL means the profile is local (imported by hand/file).
    pub async fn write_protection(&self, name: &str) -> Result<ProfileWriteProtection, Failure> {
        let info = self.load_profile_info(name).await?;
        Ok(ProfileWriteProtection::from_subscription_url(
            info.subscription_url.as_deref().unwrap_or_default(),
        ))
    }

    pub async fn save_current_profile_content<F, E, R>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        strategy: ApplyStrategy,
        transform: F,
    ) -> Result<ProfileContentSaveOutcome, Failure>
    where
        F: FnOnce(&str) -> Result<String, E> + Send + 'static,
        E: Display + Send + 'static,
        R: ActiveProfileReferenceApplier + ?Sized,
    {
        let expected = self.active_profile_reference().await?;
        let ActiveProfileReferenceState::Selected(reference) = &expected else {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "cannot edit current profile while active profile is unselected",
                false,
            ));
        };
        if reference.engine_id != self.store.engine_id() {
            return Err(Failure::unsupported(
                "active profile belongs to another engine namespace",
            ));
        }
        let workspace = self
            .store
            .load_workspace(&reference.profile_id)
            .await
            .map_err(Failure::from)?;
        if workspace.document.identity.engine_id != reference.engine_id
            || workspace.document.identity.profile_id != reference.profile_id
        {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "selected reference does not match the current profile workspace",
                true,
            ));
        }
        let content = workspace
            .document
            .utf8_text()
            .map_err(|error| Failure::new(ErrorCode::Storage, error.to_string(), false))?;
        let updated = transform(&content)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        self.save_selected_profile_content(runtime, &expected, &workspace, &updated, strategy)
            .await
    }

    async fn save_profile_content_for_reference<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        profile: String,
        content: &str,
        strategy: ApplyStrategy,
        expected: &ActiveProfileReferenceState,
    ) -> Result<ProfileContentSaveOutcome, Failure> {
        let observed = self.active_profile_reference().await?;
        if &observed != expected {
            return Err(stale_reference_failure(expected, &observed));
        }
        match expected {
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.store.engine_id()
                    && reference.profile_id.as_str() == profile =>
            {
                let workspace = self
                    .store
                    .load_workspace(&reference.profile_id)
                    .await
                    .map_err(Failure::from)?;
                if workspace.identity().document.engine_id != reference.engine_id
                    || workspace.identity().document.profile_id != reference.profile_id
                {
                    return Err(Failure::new(
                        ErrorCode::NotReady,
                        "selected reference does not match the current profile workspace",
                        true,
                    ));
                }
                self.save_selected_profile_content(runtime, expected, &workspace, content, strategy)
                    .await
            }
            _ => self
                .persist_profile_content_if_reference_current(&profile, content, expected)
                .await
                .map(|()| ProfileContentSaveOutcome::StoredInactive),
        }
    }

    async fn save_selected_profile_content<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        expected_reference: &ActiveProfileReferenceState,
        previous: &ProfileWorkspace,
        content: &str,
        strategy: ApplyStrategy,
    ) -> Result<ProfileContentSaveOutcome, Failure> {
        let ActiveProfileReferenceState::Selected(reference) = expected_reference else {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "cannot save selected-profile content while the active reference is unselected",
                false,
            ));
        };
        let expected_workspace = previous.identity();
        if expected_workspace.document.engine_id != reference.engine_id
            || expected_workspace.document.profile_id != reference.profile_id
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "active reference does not identify the submitted profile workspace",
                false,
            ));
        }
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: content.as_bytes().to_vec(),
            sidecar_bytes: previous.sidecar_bytes.clone(),
        };
        let outcome = match runtime.as_ref() {
            ProfileRuntimeAccessRef::Offline => {
                self.persist_active_workspace(
                    reference,
                    &expected_workspace,
                    &update,
                    ActiveProfileReferenceWriteAuthorization::Unowned,
                )
                .await?;
                ProfileContentSaveOutcome::StoredOffline
            }
            ProfileRuntimeAccessRef::CoreOwner(runtime) => {
                if runtime.engine_id().as_ref() != Some(&reference.engine_id) {
                    return Err(Failure::unsupported(
                        "profile content runtime does not own the active profile engine",
                    ));
                }
                let authorization = runtime.write_authorization()?;
                ensure_bound_owner_authorization(authorization.clone(), &reference.engine_id)?;
                match runtime.core_lifecycle() {
                    CoreLifecycle::Ready | CoreLifecycle::Running => {
                        runtime
                            .apply_active_profile_workspace(
                                reference,
                                &expected_workspace,
                                &update,
                                authorization,
                                strategy,
                            )
                            .await
                            .map_err(Failure::from)?;
                        ProfileContentSaveOutcome::Applied
                    }
                    CoreLifecycle::Stopped => {
                        self.persist_active_workspace(
                            reference,
                            &expected_workspace,
                            &update,
                            authorization,
                        )
                        .await?;
                        ProfileContentSaveOutcome::StoredWhileInactive(
                            CoreInactiveLifecycle::Stopped,
                        )
                    }
                    CoreLifecycle::Failed => {
                        self.persist_active_workspace(
                            reference,
                            &expected_workspace,
                            &update,
                            authorization,
                        )
                        .await?;
                        ProfileContentSaveOutcome::StoredWhileInactive(
                            CoreInactiveLifecycle::Failed,
                        )
                    }
                    CoreLifecycle::Starting | CoreLifecycle::Stopping => {
                        return Err(Failure::new(
                            ErrorCode::InvalidState,
                            "Core lifecycle is transitioning; profile content was not changed",
                            true,
                        ));
                    }
                }
            }
        };
        Ok(outcome)
    }

    async fn persist_profile_content_if_reference_current(
        &self,
        profile: &str,
        content: &str,
        expected: &ActiveProfileReferenceState,
    ) -> Result<(), Failure> {
        let observed = self.active_profile_reference().await?;
        if &observed != expected {
            return Err(stale_reference_failure(expected, &observed));
        }
        self.store
            .save_inactive_profile(profile, content)
            .await
            .map_err(Failure::from)?;
        self.store
            .clear_backup(profile)
            .await
            .map_err(Failure::from)
    }

    /// Switch the revisioned active-profile reference and apply that exact
    /// committed profile. A failed apply restores the previous reference only
    /// while this activation's revision is still current.
    pub async fn activate_profile<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        profile: &str,
    ) -> Result<ProfileActivationOutcome, Failure> {
        self.activate_profile_using(runtime.as_ref(), profile).await
    }

    /// Activation entry for owners that can apply an exact reference but do
    /// not implement the full runtime controller surface.
    pub async fn activate_profile_with_applier(
        &self,
        applier: Arc<dyn ActiveProfileReferenceApplier>,
        profile: &str,
    ) -> Result<ProfileActivationOutcome, Failure> {
        self.activate_profile_using(
            ProfileRuntimeAccessRef::CoreOwner(applier.as_ref()),
            profile,
        )
        .await
    }

    async fn activate_profile_using<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccessRef<'_, R>,
        profile: &str,
    ) -> Result<ProfileActivationOutcome, Failure> {
        let profile = valid_name(profile)?;
        let engine_id = self.store.engine_id();
        let profile_id = ProfileId::new(profile.clone())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let mode = match runtime {
            ProfileRuntimeAccessRef::Offline => ActivationMode::Offline,
            ProfileRuntimeAccessRef::CoreOwner(runtime) => {
                if runtime.engine_id().as_ref() != Some(&engine_id) {
                    return Err(Failure::unsupported(
                        "profile activation runtime does not own the profile engine",
                    ));
                }
                let authorization = runtime.write_authorization().map_err(Failure::from)?;
                ensure_bound_owner_authorization(authorization.clone(), &engine_id)?;
                match runtime.core_lifecycle() {
                    CoreLifecycle::Ready | CoreLifecycle::Running => ActivationMode::Running {
                        runtime,
                        authorization,
                    },
                    CoreLifecycle::Stopped => ActivationMode::Inactive {
                        authorization,
                        lifecycle: CoreInactiveLifecycle::Stopped,
                    },
                    CoreLifecycle::Failed => ActivationMode::Inactive {
                        authorization,
                        lifecycle: CoreInactiveLifecycle::Failed,
                    },
                    CoreLifecycle::Starting | CoreLifecycle::Stopping => {
                        return Err(Failure::new(
                            ErrorCode::InvalidState,
                            "Core lifecycle is transitioning; profile selection was not changed",
                            true,
                        ));
                    }
                }
            }
        };
        let authorization = mode.authorization();
        let target = ActiveProfileReferenceTarget::selected(engine_id, profile_id);
        let previous = self
            .store
            .read_active_profile_reference()
            .await
            .map_err(Failure::from)?;
        if matches!(&mode, ActivationMode::Running { .. })
            && matches!(&previous, ActiveProfileReferenceState::Unselected { .. })
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "a runtime profile activation requires an already selected profile for conditional recovery",
                false,
            ));
        }
        if selection_matches_target(&previous, &target) {
            return Ok(match mode {
                ActivationMode::Offline => ProfileActivationOutcome::AlreadySelectedWithoutRuntime,
                ActivationMode::Running { .. } => {
                    ProfileActivationOutcome::AlreadySelectedWithRuntime
                }
                ActivationMode::Inactive { lifecycle, .. } => {
                    ProfileActivationOutcome::AlreadySelectedWhileInactive(lifecycle)
                }
            });
        }

        let committed = match self
            .store
            .compare_and_set_active_profile_reference(
                previous.clone(),
                target.clone(),
                authorization.clone(),
            )
            .await
            .map_err(Failure::from)?
        {
            ActiveProfileReferenceCasOutcome::Committed {
                previous: observed_previous,
                current,
            } => {
                validate_committed_selection(&previous, &observed_previous, &current, &target)?;
                current
            }
            ActiveProfileReferenceCasOutcome::Stale { expected, observed } => {
                return Err(stale_reference_failure(&expected, &observed));
            }
        };

        match mode {
            ActivationMode::Offline => {
                return Ok(ProfileActivationOutcome::SelectedWithoutRuntime);
            }
            ActivationMode::Inactive { lifecycle, .. } => {
                return Ok(ProfileActivationOutcome::SelectedWhileInactive(lifecycle));
            }
            ActivationMode::Running {
                runtime,
                authorization,
            } => {
                let ActiveProfileReferenceState::Selected(committed_reference) = &committed else {
                    return Err(Failure::new(
                        ErrorCode::Internal,
                        "profile activation compare-and-set committed an unselected reference",
                        false,
                    ));
                };
                if let Err(apply_error) = runtime
                    .apply_active_profile_reference(
                        committed_reference,
                        ApplyStrategy::AlwaysRestart,
                    )
                    .await
                {
                    return Err(self
                        .recover_failed_activation(
                            runtime,
                            &previous,
                            &committed,
                            &authorization,
                            apply_error,
                        )
                        .await);
                }
                Ok(ProfileActivationOutcome::SelectedAndApplied)
            }
        }
    }

    async fn recover_failed_activation<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: &R,
        previous: &ActiveProfileReferenceState,
        committed: &ActiveProfileReferenceState,
        authorization: &ActiveProfileReferenceWriteAuthorization,
        apply_error: infiltrator_ports::error::PortError,
    ) -> Failure {
        let expected_revision = committed.revision();
        let apply_failure = core_apply_failure_kind(&apply_error);
        let recovery = self
            .store
            .compare_and_set_active_profile_reference(
                committed.clone(),
                target_from_reference_state(previous),
                authorization.clone(),
            )
            .await;
        match recovery {
            Ok(ActiveProfileReferenceCasOutcome::Committed {
                previous: observed_previous,
                current: restored,
            }) => {
                if observed_previous != *committed
                    || !restored_matches_previous(previous, &restored, expected_revision)
                {
                    return Failure::new(
                        ErrorCode::Internal,
                        format!(
                            "profile apply failed: {apply_error}; conditional recovery returned an invalid selection receipt"
                        ),
                        false,
                    )
                    .with_reason(FailureReason::ActiveProfileRecovery(
                        ActiveProfileRecoveryOutcome::CompareFailed {
                            expected_revision: expected_revision.value(),
                            message: "compare-and-set returned an unexpected recovery receipt"
                                .to_owned(),
                            apply_failure,
                        },
                    ));
                }
                match (previous, &restored) {
                    (
                        ActiveProfileReferenceState::Unselected { .. },
                        ActiveProfileReferenceState::Unselected { .. },
                    ) => Failure::new(
                        ErrorCode::Internal,
                        format!(
                            "profile apply failed from an unselected state; runtime recovery is not authorized: {apply_error}"
                        ),
                        false,
                    )
                    .with_reason(FailureReason::ActiveProfileRecovery(
                        ActiveProfileRecoveryOutcome::CompareFailed {
                            expected_revision: expected_revision.value(),
                            message: "runtime activation must not recover to Unselected".to_owned(),
                            apply_failure,
                        },
                    )),
                    (
                        ActiveProfileReferenceState::Selected(previous_reference),
                        ActiveProfileReferenceState::Selected(restored_reference),
                    ) if runtime.engine_id().as_ref() == Some(&previous_reference.engine_id) => {
                        match runtime.core_lifecycle() {
                            CoreLifecycle::Stopped => {
                                Failure::new(
                                    ErrorCode::Internal,
                                    format!(
                                        "profile switch failed; the previous reference was restored while Core remains stopped: {apply_error}"
                                    ),
                                    false,
                                )
                                .with_reason(FailureReason::ActiveProfileRecovery(
                                    ActiveProfileRecoveryOutcome::RestoredButRuntimeUnavailable {
                                        restored_revision: restored.revision().value(),
                                        engine_id: previous_reference.engine_id.clone(),
                                        lifecycle: CoreInactiveLifecycle::Stopped,
                                        apply_failure,
                                    },
                                ))
                            }
                            CoreLifecycle::Failed => {
                                Failure::new(
                                    ErrorCode::Internal,
                                    format!(
                                        "profile switch failed; the previous reference was restored while Core remains failed: {apply_error}"
                                    ),
                                    false,
                                )
                                .with_reason(FailureReason::ActiveProfileRecovery(
                                    ActiveProfileRecoveryOutcome::RestoredButRuntimeUnavailable {
                                        restored_revision: restored.revision().value(),
                                        engine_id: previous_reference.engine_id.clone(),
                                        lifecycle: CoreInactiveLifecycle::Failed,
                                        apply_failure,
                                    },
                                ))
                            }
                            CoreLifecycle::Starting | CoreLifecycle::Stopping => Failure::new(
                                ErrorCode::Internal,
                                format!(
                                    "profile switch failed; the previous reference was restored but Core is transitioning: {apply_error}"
                                ),
                                false,
                            )
                            .with_reason(FailureReason::ActiveProfileRecovery(
                                ActiveProfileRecoveryOutcome::ReapplyFailed {
                                    restored_revision: restored.revision().value(),
                                    message: "Core lifecycle is transitioning; reapply was not dispatched".to_owned(),
                                    apply_failure,
                                },
                            )),
                            CoreLifecycle::Ready | CoreLifecycle::Running => match runtime
                                .apply_active_profile_reference(
                                    restored_reference,
                                    ApplyStrategy::AlwaysRestart,
                                )
                                .await
                            {
                                Ok(_) => Failure::new(
                                ErrorCode::Internal,
                                format!(
                                    "profile switch failed; the previous profile was restored and reapplied: {apply_error}"
                                ),
                                false,
                            )
                            .with_reason(FailureReason::ActiveProfileRecovery(
                                ActiveProfileRecoveryOutcome::RestoredAndReapplied {
                                    restored_revision: restored.revision().value(),
                                    apply_failure,
                                },
                            )),
                                Err(recovery_error) => Failure::new(
                                ErrorCode::Internal,
                                format!(
                                    "profile switch failed and the restored profile could not be reapplied: {apply_error}; {recovery_error}"
                                ),
                                false,
                            )
                            .with_reason(FailureReason::ActiveProfileRecovery(
                                ActiveProfileRecoveryOutcome::ReapplyFailed {
                                    restored_revision: restored.revision().value(),
                                    message: recovery_error.to_string(),
                                    apply_failure,
                                },
                            )),
                            },
                        }
                    }
                    (
                        ActiveProfileReferenceState::Selected(previous_reference),
                        ActiveProfileReferenceState::Selected(_),
                    ) => Failure::new(
                        ErrorCode::Internal,
                        format!(
                            "profile switch failed; the previous engine reference was restored but this runtime cannot reapply it: {apply_error}"
                        ),
                        false,
                    )
                    .with_reason(FailureReason::ActiveProfileRecovery(
                        ActiveProfileRecoveryOutcome::RestoredButRuntimeMismatch {
                            restored_revision: restored.revision().value(),
                            engine_id: previous_reference.engine_id.clone(),
                            apply_failure,
                        },
                    )),
                    _ => Failure::new(
                        ErrorCode::Internal,
                        format!(
                            "profile apply failed: {apply_error}; conditional recovery returned a state inconsistent with the previous selection"
                        ),
                        false,
                    )
                    .with_reason(FailureReason::ActiveProfileRecovery(
                        ActiveProfileRecoveryOutcome::CompareFailed {
                            expected_revision: expected_revision.value(),
                            message: "recovery state did not match the previous selection"
                                .to_owned(),
                            apply_failure,
                        },
                    )),
                }
            }
            Ok(ActiveProfileReferenceCasOutcome::Stale { observed, .. }) => Failure::new(
                ErrorCode::NotReady,
                format!(
                    "profile apply failed: {apply_error}; recovery skipped because selection revision {} superseded committed revision {}",
                    observed.revision().value(),
                    expected_revision.value()
                ),
                true,
            )
            .with_reason(FailureReason::ActiveProfileRecovery(
                ActiveProfileRecoveryOutcome::Superseded {
                    expected_revision: expected_revision.value(),
                    observed_revision: observed.revision().value(),
                    apply_failure,
                },
            )),
            Err(recovery_error) => Failure::new(
                ErrorCode::Internal,
                format!(
                    "profile apply failed: {apply_error}; conditional recovery failed without changing the active reference: {recovery_error}"
                ),
                false,
            )
            .with_reason(FailureReason::ActiveProfileRecovery(
                ActiveProfileRecoveryOutcome::CompareFailed {
                    expected_revision: expected_revision.value(),
                    message: recovery_error.to_string(),
                    apply_failure,
                },
            )),
        }
    }
}

#[path = "profile_application/catalog_order.rs"]
mod catalog_order;
#[path = "profile_application/display_name.rs"]
mod display_name;

fn selection_matches_target(
    state: &ActiveProfileReferenceState,
    target: &ActiveProfileReferenceTarget,
) -> bool {
    match (state, target) {
        (
            ActiveProfileReferenceState::Selected(reference),
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            },
        ) => reference.engine_id == *engine_id && reference.profile_id == *profile_id,
        _ => false,
    }
}

fn ensure_bound_owner_authorization(
    authorization: ActiveProfileReferenceWriteAuthorization,
    expected_engine: &infiltrator_contract::core_identity::EngineId,
) -> Result<(), Failure> {
    match authorization {
        ActiveProfileReferenceWriteAuthorization::BoundOwner { identity, .. }
            if &identity.engine == expected_engine =>
        {
            Ok(())
        }
        ActiveProfileReferenceWriteAuthorization::BoundOwner { .. } => Err(Failure::unsupported(
            "active profile owner authorization belongs to a different engine",
        )),
        ActiveProfileReferenceWriteAuthorization::Unowned => Err(Failure::new(
            ErrorCode::InvalidState,
            "Core-owner profile activation requires an exact bound owner",
            false,
        )),
    }
}

fn target_from_reference_state(
    state: &ActiveProfileReferenceState,
) -> ActiveProfileReferenceTarget {
    match state {
        ActiveProfileReferenceState::Unselected { .. } => ActiveProfileReferenceTarget::Unselected,
        ActiveProfileReferenceState::Selected(reference) => ActiveProfileReferenceTarget::selected(
            reference.engine_id.clone(),
            reference.profile_id.clone(),
        ),
    }
}

fn validate_committed_selection(
    expected: &ActiveProfileReferenceState,
    previous: &ActiveProfileReferenceState,
    current: &ActiveProfileReferenceState,
    target: &ActiveProfileReferenceTarget,
) -> Result<(), Failure> {
    let expected_next_revision = expected.revision().next().ok_or_else(|| {
        Failure::new(
            ErrorCode::Internal,
            "active profile reference revision overflowed after compare-and-set",
            false,
        )
    })?;
    if previous != expected
        || current.revision() != expected_next_revision
        || !selection_matches_target(current, target)
    {
        return Err(Failure::new(
            ErrorCode::Internal,
            "active profile reference compare-and-set returned an invalid commit receipt",
            false,
        ));
    }
    Ok(())
}

fn restored_matches_previous(
    previous: &ActiveProfileReferenceState,
    restored: &ActiveProfileReferenceState,
    expected_revision: ActiveProfileReferenceRevision,
) -> bool {
    let Some(restored_revision) = expected_revision.next() else {
        return false;
    };
    match (previous, restored) {
        (
            ActiveProfileReferenceState::Unselected { .. },
            ActiveProfileReferenceState::Unselected { revision },
        ) => *revision == restored_revision,
        (
            ActiveProfileReferenceState::Selected(previous),
            ActiveProfileReferenceState::Selected(restored),
        ) => {
            previous.engine_id == restored.engine_id
                && previous.profile_id == restored.profile_id
                && restored.revision == restored_revision
        }
        _ => false,
    }
}

fn stale_reference_failure(
    expected: &ActiveProfileReferenceState,
    observed: &ActiveProfileReferenceState,
) -> Failure {
    Failure::new(
        ErrorCode::NotReady,
        format!(
            "active profile selection changed from revision {} to {} before the requested update",
            expected.revision().value(),
            observed.revision().value()
        ),
        true,
    )
    .with_reason(FailureReason::ActiveProfileReferenceStale {
        expected_revision: expected.revision().value(),
        observed_revision: observed.revision().value(),
    })
}

fn core_apply_failure_kind(error: &infiltrator_ports::error::PortError) -> CoreApplyFailureKind {
    match error {
        infiltrator_ports::error::PortError::Rejected(failure) => match failure.reason.as_ref() {
            Some(FailureReason::CoreApplyFailure(kind)) => *kind,
            Some(FailureReason::ActiveProfileReferenceStale { .. }) => {
                CoreApplyFailureKind::ActiveProfileReferenceStale
            }
            _ => CoreApplyFailureKind::Adapter,
        },
        _ => CoreApplyFailureKind::Adapter,
    }
}

pub(crate) fn valid_name(name: &str) -> Result<String, Failure> {
    sanitize_profile_name(name)
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))
}

async fn ensure_profile_inactive(
    store: &Arc<dyn ProfileStore>,
    profile: &str,
) -> Result<(), Failure> {
    match store
        .read_active_profile_reference()
        .await
        .map_err(Failure::from)?
    {
        ActiveProfileReferenceState::Unselected { .. } => Ok(()),
        ActiveProfileReferenceState::Selected(reference)
            if reference.engine_id == store.engine_id()
                && reference.profile_id.as_str() == profile =>
        {
            Err(Failure::new(
                ErrorCode::InvalidState,
                "selected profile content requires its revision-bound Core-owner apply path",
                true,
            ))
        }
        ActiveProfileReferenceState::Selected(_) => Ok(()),
    }
}

async fn commit_subscription_document(
    store: &Arc<dyn ProfileStore>,
    name: &str,
    source_url: &str,
    document: SubscriptionDocument,
) -> Result<(), Failure> {
    if document.content.trim().is_empty() {
        return Err(Failure::new(
            ErrorCode::Configuration,
            "subscription returned empty content",
            false,
        ));
    }
    validate_yaml(&document.content)
        .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
    ensure_profile_inactive(store, name).await?;
    store
        .save_inactive_profile(name, &document.content)
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
    metadata: &mut ProfileMetadata,
    userinfo: Option<SubscriptionUserInfo>,
    now: chrono::DateTime<Utc>,
) {
    apply_subscription_quota(metadata, userinfo);
    metadata.last_updated = Some(now);
    schedule_next_update(metadata, now);
}

fn apply_subscription_quota(
    metadata: &mut ProfileMetadata,
    userinfo: Option<SubscriptionUserInfo>,
) {
    if let Some(info) = userinfo {
        metadata.traffic_upload = info.upload;
        metadata.traffic_download = info.download;
        metadata.traffic_total = info.total;
        metadata.expire_at = info.expire;
    }
}

fn schedule_next_update(metadata: &mut ProfileMetadata, now: chrono::DateTime<Utc>) {
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
        content_effect: SubscriptionContentEffectOutcome::NotAttempted,
        backed_up,
    }
}

#[cfg(test)]
#[path = "profile_application_tests.rs"]
mod tests;
