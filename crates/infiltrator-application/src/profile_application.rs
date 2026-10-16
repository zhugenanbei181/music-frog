//! Profile use-cases over the runtime-neutral profile store port.
//!
//! This module owns profile identity validation, projections, activation, and
//! the boundary between a stored profile and a live Core owner. Concrete
//! config managers and filesystem details stay in outbound adapters.

use infiltrator_contract::core_identity::EngineId;
use infiltrator_ports::error::PortError;
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

mod active_selection;
mod age_credential;
mod document_codec;
mod workspace;
use workspace::{ProfileWorkspaceEffect, ProfileWorkspaceStoreReason};

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

    pub fn engine_id(&self) -> EngineId {
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
}

#[path = "profile_application/subscriptions.rs"]
mod subscriptions;

#[path = "profile_application/activation_save.rs"]
mod activation_save;

#[path = "profile_application/catalog_order.rs"]
mod catalog_order;
#[path = "profile_application/display_name.rs"]
mod display_name;
#[path = "profile_application/named_overrides.rs"]
mod named_overrides;

pub(super) fn selection_matches_target(
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

pub(super) fn ensure_bound_owner_authorization(
    authorization: ActiveProfileReferenceWriteAuthorization,
    expected_engine: &EngineId,
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

pub(super) fn target_from_reference_state(
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

pub(super) fn validate_committed_selection(
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

pub(super) fn restored_matches_previous(
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

pub(super) fn stale_reference_failure(
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

pub(super) fn core_apply_failure_kind(error: &PortError) -> CoreApplyFailureKind {
    match error {
        PortError::Rejected(failure) => match failure.reason.as_ref() {
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

pub(super) async fn ensure_profile_inactive(
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

pub(super) async fn commit_subscription_document(
    store: &Arc<dyn ProfileStore>,
    name: &str,
    source_url: &str,
    document: &SubscriptionDocument,
    content: &str,
) -> Result<(), Failure> {
    if content.trim().is_empty() {
        return Err(Failure::new(
            ErrorCode::Configuration,
            "subscription returned empty content",
            false,
        ));
    }
    validate_yaml(content)
        .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
    ensure_profile_inactive(store, name).await?;
    store
        .save_inactive_profile(name, content)
        .await
        .map_err(Failure::from)?;

    let mut metadata = store
        .get_profile_metadata(name)
        .await
        .map_err(Failure::from)?;
    metadata.subscription_url = Some(source_url.to_owned());
    apply_subscription_metadata(&mut metadata, document.userinfo.clone(), Utc::now());
    store
        .update_profile_metadata(name, &metadata)
        .await
        .map_err(Failure::from)
}

pub(super) fn profile_id_for_name(name: &str) -> Result<ProfileId, Failure> {
    ProfileId::new(name.to_owned()).map_err(|_| {
        Failure::new(
            ErrorCode::InvalidInput,
            "profile identity is invalid for subscription credential access",
            false,
        )
    })
}

pub(super) fn apply_subscription_metadata(
    metadata: &mut ProfileMetadata,
    userinfo: Option<SubscriptionUserInfo>,
    now: chrono::DateTime<Utc>,
) {
    apply_subscription_quota(metadata, userinfo);
    metadata.last_updated = Some(now);
    schedule_next_update(metadata, now);
}

pub(super) fn apply_subscription_quota(
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

pub(super) fn schedule_next_update(metadata: &mut ProfileMetadata, now: chrono::DateTime<Utc>) {
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

pub(super) fn report_from_metadata(
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
