//! Engine/format/schema scoped native configuration transactions.

use crate::error::PortError;
use crate::profile_document_codec::ProfileDocumentCodecScope;
use crate::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use async_trait::async_trait;
use infiltrator_contract::error::Failure;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_domain::apply::ApplyStrategy;
use std::sync::{Arc, Mutex};

/// Exact codec scope serviced by one native configuration transaction owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileConfigurationTransactionScope {
    pub document: ProfileDocumentCodecScope,
}

impl ProfileConfigurationTransactionScope {
    pub fn new(document: ProfileDocumentCodecScope) -> Self {
        Self { document }
    }

    pub fn supports(&self, identity: &ProfileWorkspaceIdentity) -> bool {
        identity.document.engine_id == self.document.engine_id
            && identity.document.format_id == self.document.format_id
            && identity.document.schema_version == self.document.schema_version
    }
}

/// Source-bound request for a complete native apply transaction.
#[derive(Clone, Debug)]
pub struct ProfileConfigurationTransactionRequest {
    pub expected: ProfileWorkspaceIdentity,
    pub update: ProfileWorkspaceUpdate,
    pub strategy: ApplyStrategy,
}

/// Native candidate validation that does not require a live Core controller.
/// This is required before publishing a workspace when the Core is stopped or
/// failed; structural codec inspection alone does not prove the engine accepts
/// the configuration.
#[async_trait]
pub trait ProfileConfigurationCandidateValidatorPort: Send + Sync {
    fn scope(&self) -> ProfileConfigurationTransactionScope;

    async fn validate_candidate(
        &self,
        expected: &ProfileWorkspaceIdentity,
        document_bytes: &[u8],
    ) -> Result<(), PortError>;
}

/// Explicit fail-closed validator for compositions that cannot run the
/// engine's native config checker while its Core is stopped.
pub struct UnavailableProfileConfigurationCandidateValidator {
    scope: ProfileConfigurationTransactionScope,
    reason: String,
}

impl UnavailableProfileConfigurationCandidateValidator {
    pub fn new(scope: ProfileConfigurationTransactionScope, reason: impl Into<String>) -> Self {
        Self {
            scope,
            reason: reason.into(),
        }
    }
}

#[async_trait]
impl ProfileConfigurationCandidateValidatorPort
    for UnavailableProfileConfigurationCandidateValidator
{
    fn scope(&self) -> ProfileConfigurationTransactionScope {
        self.scope.clone()
    }

    async fn validate_candidate(
        &self,
        _expected: &ProfileWorkspaceIdentity,
        _document_bytes: &[u8],
    ) -> Result<(), PortError> {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
            self.reason.clone(),
        ))
    }
}

/// Where a committed workspace takes effect.
///
/// Inactive commits persist the validated document while preserving the
/// observed Core lifecycle. They never start the Core as a side effect of an
/// edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileConfigurationCommitEffect {
    /// The candidate was applied to a Core that was already Ready or Running.
    AppliedToRunningCore,
    /// The candidate was stored while Core remained Stopped.
    StoredWhileStopped,
    /// The candidate was stored while Core remained Failed.
    StoredWhileFailed,
}

/// The transaction's exhaustive terminal state.
#[derive(Clone, Debug, PartialEq)]
pub enum ProfileConfigurationTransactionOutcome {
    /// Native validation and persistence succeeded. `workspace` is the exact
    /// document/sidecar readback paired with its new revision; `effect`
    /// distinguishes a live apply from a saved inactive workspace.
    Committed {
        workspace: ProfileWorkspace,
        effect: ProfileConfigurationCommitEffect,
    },
    /// The transaction failed and the exact previous workspace was restored
    /// and read back successfully.
    RolledBack {
        restored: ProfileWorkspace,
        cause: Failure,
    },
    /// The transaction and recovery both failed; the live engine or stored
    /// workspace is uncertain and must not be reported as successful.
    RollbackFailed { cause: Failure, rollback: Failure },
}

/// Engine adapter for native validate/apply/readback/rollback behavior.
///
/// Foreign scope, stale workspace revision, and invalid candidates must be
/// rejected before persistent or live-Core write effects. A committed result
/// must carry byte-exact readback, a revised workspace identity, and an
/// exhaustive effect. A Core observed as Stopped or Failed stays inactive;
/// this transaction never starts it. Any post-write failure must end in
/// `RolledBack` or `RollbackFailed`.
#[async_trait]
pub trait ProfileConfigurationTransactionPort: Send + Sync {
    fn scope(&self) -> ProfileConfigurationTransactionScope;

    async fn transact(
        &self,
        request: ProfileConfigurationTransactionRequest,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError>;
}

/// Explicitly unavailable engine transaction adapter. Product compositions
/// use it when the engine/format codec is present but native apply is not.
pub struct UnavailableProfileConfigurationTransaction {
    scope: ProfileConfigurationTransactionScope,
    reason: String,
}

impl UnavailableProfileConfigurationTransaction {
    pub fn new(scope: ProfileConfigurationTransactionScope, reason: impl Into<String>) -> Self {
        Self {
            scope,
            reason: reason.into(),
        }
    }
}

#[async_trait]
impl ProfileConfigurationTransactionPort for UnavailableProfileConfigurationTransaction {
    fn scope(&self) -> ProfileConfigurationTransactionScope {
        self.scope.clone()
    }

    async fn transact(
        &self,
        _request: ProfileConfigurationTransactionRequest,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError> {
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::Profiles,
            self.reason.clone(),
        ))
    }
}

/// A product composition may create its engine bundle before the shared
/// `CoreApplication` session owner exists. This binding puts a scope-checked,
/// fail-closed transaction port in the bundle and allows the composition root
/// to bind its concrete owner exactly once before publishing commands.
pub struct ProfileConfigurationTransactionBinding {
    scope: ProfileConfigurationTransactionScope,
    state: Mutex<BindingState>,
}

enum BindingState {
    Unbound,
    Bound(Arc<dyn ProfileConfigurationTransactionPort>),
}

impl ProfileConfigurationTransactionBinding {
    pub fn new(scope: ProfileConfigurationTransactionScope) -> Self {
        Self {
            scope,
            state: Mutex::new(BindingState::Unbound),
        }
    }

    pub fn bind(
        &self,
        transaction: Arc<dyn ProfileConfigurationTransactionPort>,
    ) -> Result<(), PortError> {
        if transaction.scope() != self.scope {
            return Err(PortError::Failed(
                "configuration transaction adapter scope differs from its binding".to_owned(),
            ));
        }
        let mut state = self.state.lock().map_err(|_| {
            PortError::Failed("configuration transaction binding lock is poisoned".to_owned())
        })?;
        match &*state {
            BindingState::Unbound => {
                *state = BindingState::Bound(transaction);
                Ok(())
            }
            BindingState::Bound(_) => Err(PortError::Failed(
                "configuration transaction binding is already sealed".to_owned(),
            )),
        }
    }
}

#[async_trait]
impl ProfileConfigurationTransactionPort for ProfileConfigurationTransactionBinding {
    fn scope(&self) -> ProfileConfigurationTransactionScope {
        self.scope.clone()
    }

    async fn transact(
        &self,
        request: ProfileConfigurationTransactionRequest,
    ) -> Result<ProfileConfigurationTransactionOutcome, PortError> {
        let transaction = {
            let state = self.state.lock().map_err(|_| {
                PortError::Failed("configuration transaction binding lock is poisoned".to_owned())
            })?;
            match &*state {
                BindingState::Unbound => {
                    return Err(PortError::Rejected(
                        infiltrator_contract::error::Failure::new(
                            infiltrator_contract::error::ErrorCode::NotReady,
                            "configuration transaction owner is not composed",
                            false,
                        ),
                    ));
                }
                BindingState::Bound(transaction) => Arc::clone(transaction),
            }
        };
        transaction.transact(request).await
    }
}
