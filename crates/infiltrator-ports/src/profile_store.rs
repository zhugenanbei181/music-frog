//! Runtime-neutral profile/configuration persistence port.

use crate::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::aggregator::AggregationTemplate;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::named_override::NamedOverrideCatalog;
use infiltrator_contract::profile_age_credential::{
    ProfileAgeContentDecryption, ProfileAgeCredentialMutation, ProfileAgeCredentialReadOutcome,
    ProfileAgeCredentialReadRequest, ProfileAgeCredentialWriteRequest, ProfileAgeEncryptedContent,
};
use infiltrator_contract::profile_catalog_order::{
    ProfileCatalogOrderMutation, ProfileCatalogOrderRequest,
};
use infiltrator_contract::profile_display_name::{
    ProfileDisplayNameMutation, ProfileDisplayNameObservation, ProfileDisplayNameTarget,
};
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use std::path::PathBuf;

/// Active-workspace transaction outcome when a conditional restoration finds
/// that another writer has already changed the published workspace.
pub enum ActiveProfileWorkspaceRestoreOutcome<'a> {
    Restored(Box<dyn ActiveProfileWorkspaceRestoredLease + 'a>),
    Superseded { observed: ProfileWorkspaceIdentity },
    RecoveryRequired(Box<dyn ActiveProfileWorkspaceRecoveryLease + 'a>),
}

/// Durable phase of an active-workspace operation that could not yet be
/// classified by readback and conditional recovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileWorkspaceRecoveryState {
    ConfirmPublished {
        previous: ProfileWorkspaceIdentity,
        target: ProfileWorkspaceIdentity,
    },
    RestorePrevious {
        previous: ProfileWorkspaceIdentity,
        published: ProfileWorkspaceIdentity,
    },
}

#[must_use = "a recovery-required workspace lease must be resolved while its fences remain held"]
pub enum ActiveProfileWorkspacePublishOutcome<'a> {
    Published(Box<dyn ActiveProfileWorkspacePublishedLease + 'a>),
    Unpublished(PortError),
    Superseded { observed: ProfileWorkspaceIdentity },
    RecoveryRequired(Box<dyn ActiveProfileWorkspaceRecoveryLease + 'a>),
}

#[must_use = "a recovery-required workspace lease must be resolved while its fences remain held"]
pub enum ActiveProfileWorkspaceRecoveryResolution<'a> {
    Published(Box<dyn ActiveProfileWorkspacePublishedLease + 'a>),
    Restored(Box<dyn ActiveProfileWorkspaceRestoredLease + 'a>),
    Unpublished(PortError),
    Superseded { observed: ProfileWorkspaceIdentity },
    RecoveryRequired(Box<dyn ActiveProfileWorkspaceRecoveryLease + 'a>),
}

/// The observed phase of a managed active-workspace transaction. Acquisition
/// validates the full active reference, workspace identity and owner
/// authorization before returning; dropping this lease performs no writes.
#[must_use = "an observed active-workspace lease must be published or explicitly dropped"]
#[async_trait]
pub trait ActiveProfileWorkspaceWriteLease: Send {
    fn observed_workspace(&self) -> &ProfileWorkspace;

    /// Publish the new files while retaining all transaction fences. A failed
    /// or cancelled publication is kept alive by the managed consumer's owned
    /// continuation. Process/runtime shutdown recovery remains an OPEN
    /// boundary and must rely on the separate durable owner terminal proof.
    async fn publish(
        self: Box<Self>,
        update: ProfileWorkspaceUpdate,
    ) -> ActiveProfileWorkspacePublishOutcome<'async_trait>;
}

/// Recovery continuation returned when an I/O error prevents the adapter from
/// proving whether publication/restoration completed. It retains the original
/// cross-process fences; callers must keep resolving it inside the owned
/// transaction continuation and must never translate it into a plain error.
#[must_use = "dropping this lease releases fences before active workspace recovery is proven"]
#[async_trait]
pub trait ActiveProfileWorkspaceRecoveryLease: Send {
    fn state(&self) -> ActiveProfileWorkspaceRecoveryState;
    fn last_error(&self) -> &str;

    async fn resolve(self: Box<Self>) -> ActiveProfileWorkspaceRecoveryResolution<'async_trait>;
}

/// The published phase remains fenced through runtime apply and readback.
/// Call `commit` only after runtime success; otherwise restore conditionally
/// and keep the returned restored lease through native recovery. Managed
/// callers must put the whole sequence in an owned continuation: dropping a
/// pending lease releases its kernel locks and does not report a commit.
#[must_use = "a published active-workspace lease must be committed or restored"]
#[async_trait]
pub trait ActiveProfileWorkspacePublishedLease: Send {
    fn previous_workspace(&self) -> &ProfileWorkspace;
    fn published_workspace(&self) -> &ProfileWorkspace;

    fn commit(self: Box<Self>) -> ProfileWorkspace;

    async fn restore_if_current(
        self: Box<Self>,
    ) -> Result<ActiveProfileWorkspaceRestoreOutcome<'async_trait>, PortError>;
}

/// Restored files remain fenced until the runtime has confirmed the old
/// configuration or the caller explicitly accepts the recovered state.
#[must_use = "a restored active-workspace lease must be finished after native recovery"]
#[async_trait]
pub trait ActiveProfileWorkspaceRestoredLease: Send {
    fn restored_workspace(&self) -> &ProfileWorkspace;
    fn finish(self: Box<Self>) -> ProfileWorkspace;
}

/// Profile persistence operations needed by application/inbound surfaces.
///
/// The implementation may use a filesystem, database, or platform sync store;
/// callers receive only domain values and owned strings. No `ConfigManager`,
/// keyring implementation, Tokio channel, or controller type crosses this
/// boundary.
#[async_trait]
pub trait ProfileStore: Send + Sync {
    /// Namespace of every operation in this store, including string-keyed
    /// legacy domain metadata, the active pointer, credentials and backups.
    fn engine_id(&self) -> EngineId;
    /// Filesystem root already scoped to `engine_id()`.
    fn config_dir(&self) -> PathBuf;
    /// A prior actual transaction from this host instance for this profile, if observed.
    fn apply_transaction(&self, _profile: &str) -> Option<ApplyTransactionSnapshot> {
        None
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError>;
    /// Compare the complete observed identity order before publishing a new
    /// engine-scoped catalog order. Stale requests return the current readback.
    async fn set_profile_catalog_order(
        &self,
        _request: ProfileCatalogOrderRequest,
    ) -> Result<ProfileCatalogOrderMutation, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "snapshot-fenced profile catalog ordering is unavailable",
        ))
    }
    async fn get_current(&self) -> Result<String, PortError>;
    /// Read the product-wide engine/profile selector, including its revision.
    /// Activation and rollback paths use this complete revisioned state as the
    /// expected value for every active-selection transition.
    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "revisioned active-profile selection is unavailable",
        ))
    }
    /// Compare the complete observed state by revision before publishing a
    /// target. The authorization is part of the write contract: offline
    /// selection requires an unowned Core, while a live Core write must name
    /// its exact bound identity and generation. A stale compare is an explicit
    /// result with the readback; adapter failures remain errors.
    async fn compare_and_set_active_profile_reference(
        &self,
        _expected: ActiveProfileReferenceState,
        _target: ActiveProfileReferenceTarget,
        _authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceCasOutcome, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "revision-fenced active-profile selection is unavailable",
        ))
    }
    async fn load(&self, profile: &str) -> Result<String, PortError>;
    /// Create or replace a profile only while it is inactive under the exact
    /// product-wide reference fence. Active-profile content changes require
    /// the revisioned workspace transaction lease below.
    async fn save_inactive_profile(&self, profile: &str, content: &str) -> Result<(), PortError>;
    async fn delete_profile(&self, profile: &str) -> Result<(), PortError>;
    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError>;
    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError>;
    /// Observe the profile's presentation label without changing its stable
    /// file identity. `Identity` means the label follows the `ProfileId`.
    async fn profile_display_name(
        &self,
        _profile: &ProfileId,
    ) -> Result<ProfileDisplayNameObservation, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "profile display-name observation is unavailable",
        ))
    }
    /// Set or reset one profile's presentation label and return the exact
    /// persisted observation. This mutation never changes profile identity,
    /// active selection, Core, or VPN lifecycle.
    async fn set_profile_display_name(
        &self,
        _profile: &ProfileId,
        _target: ProfileDisplayNameTarget,
    ) -> Result<ProfileDisplayNameMutation, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "profile display-name mutation is unavailable",
        ))
    }
    /// Read whether an engine-scoped Age private key exists and return the
    /// exact active-reference state fenced during the read. The secret itself
    /// is never part of the profile metadata or presentation snapshot.
    async fn profile_age_credential(
        &self,
        _request: ProfileAgeCredentialReadRequest,
    ) -> Result<ProfileAgeCredentialReadOutcome, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "owner-authorized Age credential observation is unavailable",
        ))
    }
    /// Change one profile's Age key only against the expected credential
    /// revision, selected-reference snapshot, and exact Core-owner proof.
    async fn mutate_profile_age_credential(
        &self,
        _request: ProfileAgeCredentialWriteRequest,
    ) -> Result<ProfileAgeCredentialMutation, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "owner-authorized Age credential mutation is unavailable",
        ))
    }
    /// Decrypt one armored Age subscription payload within the profile owner.
    /// The private key never leaves the injected secure store owner; callers
    /// receive only a redacted, zeroizing content value for immediate
    /// validation and commit.
    async fn decrypt_age_subscription_content(
        &self,
        _profile: &ProfileId,
        _content: ProfileAgeEncryptedContent,
    ) -> Result<ProfileAgeContentDecryption, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "Age subscription decryption is unavailable",
        ))
    }
    async fn delete_subscription_credential(&self, profile: &str) -> Result<(), PortError>;
    /// Both documents must be read under the same persistence boundary.
    async fn load_workspace(&self, _profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "coherent profile editing is unavailable",
        ))
    }
    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "coherent active profile editing is unavailable",
        ))
    }
    /// Acquire the complete active content transaction fence. This must fail
    /// before any content write if reference, workspace identity or owner
    /// authorization differs. The returned lease keeps the root reference,
    /// owner-state and profile-write fences until a typed terminal method is
    /// consumed.
    async fn acquire_active_workspace_write_lease(
        &self,
        _expected_reference: ActiveProfileReference,
        _expected_workspace: ProfileWorkspaceIdentity,
        _authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + '_>, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "fenced active profile workspace transactions are unavailable",
        ))
    }
    /// Publish one engine-derived active document while the Core owner is
    /// unbound. This path may change no sidecar bytes and performs exactly one
    /// atomic document replacement, so cancellation leaves either the prior
    /// document or the complete new document. User edits and runtime-applied
    /// changes must use the owned workspace transaction lease above.
    async fn compare_and_save_unowned_active_derived_document(
        &self,
        _expected_reference: ActiveProfileReference,
        _expected_workspace: ProfileWorkspaceIdentity,
        _update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "atomic unowned derived active-document writes are unavailable",
        ))
    }
    /// Compare and publish a document plus sidecar only while that profile is
    /// inactive under the product-wide reference fence. Active content changes
    /// require the held transaction lease above.
    async fn compare_and_save_inactive_workspace(
        &self,
        _expected: &ProfileWorkspaceIdentity,
        _update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "source-bound inactive profile editing is unavailable",
        ))
    }
    /// Restore exact prior bytes of an inactive workspace only while its
    /// published source is still current. Active workspace recovery remains
    /// part of the held active transaction lease.
    async fn restore_inactive_workspace(
        &self,
        _expected: &ProfileWorkspaceIdentity,
        _previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "source-bound inactive profile recovery is unavailable",
        ))
    }
    /// DUAL-08-13: load the saved aggregation templates, empty when none where
    /// stored. Stores without a template sidecar answer with a typed
    /// unsupported instead of pretending the library is empty.
    async fn load_aggregation_templates(&self) -> Result<Vec<AggregationTemplate>, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "this profile store keeps no aggregation-template sidecar",
        ))
    }
    /// DUAL-08-13: persist the aggregation template library; an empty library
    /// removes the sidecar.
    async fn save_aggregation_templates(
        &self,
        _templates: &[AggregationTemplate],
    ) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "this profile store keeps no aggregation-template sidecar",
        ))
    }
    /// SLINT-004: load the named YAML override catalog.
    async fn load_named_override_catalog(&self) -> Result<NamedOverrideCatalog, PortError> {
        Ok(NamedOverrideCatalog::default())
    }
    /// SLINT-004: persist the named YAML override catalog.
    async fn save_named_override_catalog(
        &self,
        _catalog: &NamedOverrideCatalog,
    ) -> Result<(), PortError> {
        Ok(())
    }
    async fn clear_backup(&self, profile: &str) -> Result<(), PortError>;
    async fn restore_inactive_profile_backup(&self, profile: &str) -> Result<bool, PortError>;
}
