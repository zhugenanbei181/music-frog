//! Typed Android owner IPC for engine-keyed Core version management.

use super::{AndroidOwnerDataMutation, AndroidOwnerRevisionTransition};
use crate::core_version_owner::core_version_management_application;
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_support::map_application_failure;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::version::{
    CoreRelease, CoreReleaseSummary, CoreVersionInstallCancelOutcome, CoreVersionInstallContext,
    CoreVersionInstallId, CoreVersionInstallObservation, CoreVersionInstallState,
    CoreVersionManagementAction, CoreVersionManagementRequest, CoreVersionManagementResult,
    EngineNeutralReleaseChannel, InstalledCoreVersion, VersionDownloadProgress,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
pub enum AndroidEngineNeutralReleaseChannel {
    Stable,
    Alpha,
}

impl From<AndroidEngineNeutralReleaseChannel> for EngineNeutralReleaseChannel {
    fn from(channel: AndroidEngineNeutralReleaseChannel) -> Self {
        match channel {
            AndroidEngineNeutralReleaseChannel::Stable => Self::Stable,
            AndroidEngineNeutralReleaseChannel::Alpha => Self::Alpha,
        }
    }
}

/// Version commands are bound to one engine. Install observation/cancellation
/// use the globally unique install ID owned by `CoreVersionManagementApplication`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(deny_unknown_fields)]
pub enum AndroidCoreVersionManagementOperation {
    ListInstalled {
        engine_id: String,
    },
    ListReleases {
        engine_id: String,
        limit: u32,
    },
    Latest {
        engine_id: String,
        channel: AndroidEngineNeutralReleaseChannel,
    },
    Install {
        engine_id: String,
        version: String,
    },
    Activate {
        engine_id: String,
        version: String,
    },
    Uninstall {
        engine_id: String,
        version: String,
    },
    Rollback {
        engine_id: String,
    },
    ObserveInstall {
        install_id: u64,
    },
    CancelInstall {
        install_id: u64,
    },
}

impl AndroidCoreVersionManagementOperation {
    pub(super) fn data_mutation(&self) -> AndroidOwnerDataMutation {
        match self {
            Self::ListInstalled { .. }
            | Self::ListReleases { .. }
            | Self::Latest { .. }
            | Self::ObserveInstall { .. } => AndroidOwnerDataMutation::ReadOnly,
            Self::Install { .. }
            | Self::Activate { .. }
            | Self::Uninstall { .. }
            | Self::Rollback { .. }
            | Self::CancelInstall { .. } => AndroidOwnerDataMutation::CommitOnSuccess,
        }
    }

    fn into_application_request(self) -> Result<CoreVersionManagementRequest, FfiStatus> {
        let (engine_id, action) = match self {
            Self::ListInstalled { engine_id } => {
                (engine_id, CoreVersionManagementAction::ListInstalled)
            }
            Self::ListReleases { engine_id, limit } => (
                engine_id,
                CoreVersionManagementAction::ListReleases { limit },
            ),
            Self::Latest { engine_id, channel } => (
                engine_id,
                CoreVersionManagementAction::Latest {
                    channel: channel.into(),
                },
            ),
            Self::Install { engine_id, version } => {
                (engine_id, CoreVersionManagementAction::Install { version })
            }
            Self::Activate { engine_id, version } => {
                (engine_id, CoreVersionManagementAction::Activate { version })
            }
            Self::Uninstall { engine_id, version } => (
                engine_id,
                CoreVersionManagementAction::Uninstall { version },
            ),
            Self::Rollback { engine_id } => (engine_id, CoreVersionManagementAction::Rollback),
            Self::ObserveInstall { .. } | Self::CancelInstall { .. } => {
                return Err(FfiStatus::err(
                    FfiErrorCode::InvalidState,
                    "install task operations do not have an engine version request",
                ));
            }
        };
        let engine_id = EngineId::new(engine_id).map_err(|error| {
            FfiStatus::err(
                FfiErrorCode::InvalidInput,
                format!("Core version operation has an invalid engine ID: {error}"),
            )
        })?;
        Ok(CoreVersionManagementRequest { engine_id, action })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidInstalledCoreVersion {
    pub engine_id: String,
    pub version: String,
    pub path: String,
    pub is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidCoreRelease {
    pub engine_id: String,
    pub version: String,
    pub release_date: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidCoreReleaseSummary {
    pub engine_id: String,
    pub version: String,
    pub name: String,
    pub published_at: String,
    pub prerelease: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidCoreVersionInstallContext {
    pub install_id: u64,
    pub engine_id: String,
    pub version: String,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Record)]
#[serde(deny_unknown_fields)]
pub struct AndroidVersionDownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(deny_unknown_fields)]
pub enum AndroidCoreVersionInstallState {
    Running {
        context: AndroidCoreVersionInstallContext,
        progress: Option<AndroidVersionDownloadProgress>,
    },
    CancelRequested {
        context: AndroidCoreVersionInstallContext,
        progress: Option<AndroidVersionDownloadProgress>,
    },
    Completed {
        context: AndroidCoreVersionInstallContext,
    },
    Cancelled {
        context: AndroidCoreVersionInstallContext,
    },
    Failed {
        context: AndroidCoreVersionInstallContext,
        failure: FfiStatus,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(deny_unknown_fields)]
pub enum AndroidCoreVersionInstallObservation {
    Found {
        state: AndroidCoreVersionInstallState,
    },
    NotFound {
        install_id: u64,
    },
    Unavailable {
        failure: FfiStatus,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(deny_unknown_fields)]
pub enum AndroidCoreVersionInstallCancellation {
    Requested { install_id: u64 },
    AlreadyRequested { install_id: u64 },
    AlreadyFinished { install_id: u64 },
    NotFound { install_id: u64 },
    Unavailable { failure: FfiStatus },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, uniffi::Enum)]
#[serde(deny_unknown_fields)]
pub enum AndroidCoreVersionManagementReadback {
    InstalledVersions {
        engine_id: String,
        versions: Vec<AndroidInstalledCoreVersion>,
    },
    Releases {
        engine_id: String,
        releases: Vec<AndroidCoreReleaseSummary>,
    },
    Latest {
        engine_id: String,
        release: AndroidCoreRelease,
    },
    InstallAccepted {
        install_id: u64,
        state: AndroidCoreVersionInstallState,
    },
    Activated {
        engine_id: String,
        version: String,
    },
    Uninstalled {
        engine_id: String,
        version: String,
    },
    RolledBack {
        engine_id: String,
        selected_version: String,
    },
    InstallObservation {
        observation: AndroidCoreVersionInstallObservation,
    },
    InstallCancellation {
        outcome: AndroidCoreVersionInstallCancellation,
    },
    Failed {
        failure: FfiStatus,
    },
}

impl AndroidCoreVersionManagementReadback {
    pub(super) fn status(&self) -> FfiStatus {
        match self {
            Self::InstallObservation {
                observation: AndroidCoreVersionInstallObservation::Unavailable { failure },
            }
            | Self::InstallCancellation {
                outcome: AndroidCoreVersionInstallCancellation::Unavailable { failure },
            }
            | Self::Failed { failure } => failure.clone(),
            _ => FfiStatus::ok(),
        }
    }

    pub(super) fn revision_transition(&self) -> AndroidOwnerRevisionTransition {
        match self {
            Self::InstallAccepted { .. }
            | Self::Activated { .. }
            | Self::Uninstalled { .. }
            | Self::RolledBack { .. }
            | Self::InstallCancellation {
                outcome: AndroidCoreVersionInstallCancellation::Requested { .. },
            } => AndroidOwnerRevisionTransition::Advance,
            Self::InstalledVersions { .. }
            | Self::Releases { .. }
            | Self::Latest { .. }
            | Self::InstallObservation { .. }
            | Self::InstallCancellation { .. }
            | Self::Failed { .. } => AndroidOwnerRevisionTransition::Unchanged,
        }
    }
}

impl From<InstalledCoreVersion> for AndroidInstalledCoreVersion {
    fn from(version: InstalledCoreVersion) -> Self {
        Self {
            engine_id: version.engine_id.as_str().to_owned(),
            version: version.version,
            path: version.path,
            is_default: version.is_default,
        }
    }
}

impl From<CoreRelease> for AndroidCoreRelease {
    fn from(release: CoreRelease) -> Self {
        Self {
            engine_id: release.engine_id.as_str().to_owned(),
            version: release.version,
            release_date: release.release_date,
        }
    }
}

impl From<CoreReleaseSummary> for AndroidCoreReleaseSummary {
    fn from(release: CoreReleaseSummary) -> Self {
        Self {
            engine_id: release.engine_id.as_str().to_owned(),
            version: release.version,
            name: release.name,
            published_at: release.published_at,
            prerelease: release.prerelease,
        }
    }
}

impl From<CoreVersionInstallContext> for AndroidCoreVersionInstallContext {
    fn from(context: CoreVersionInstallContext) -> Self {
        Self {
            install_id: context.install_id.get(),
            engine_id: context.engine_id.as_str().to_owned(),
            version: context.version,
            revision: context.revision,
        }
    }
}

impl From<VersionDownloadProgress> for AndroidVersionDownloadProgress {
    fn from(progress: VersionDownloadProgress) -> Self {
        Self {
            downloaded: progress.downloaded,
            total: progress.total,
        }
    }
}

impl From<CoreVersionInstallState> for AndroidCoreVersionInstallState {
    fn from(state: CoreVersionInstallState) -> Self {
        match state {
            CoreVersionInstallState::Running { context, progress } => Self::Running {
                context: context.into(),
                progress: progress.map(Into::into),
            },
            CoreVersionInstallState::CancelRequested { context, progress } => {
                Self::CancelRequested {
                    context: context.into(),
                    progress: progress.map(Into::into),
                }
            }
            CoreVersionInstallState::Completed { context } => Self::Completed {
                context: context.into(),
            },
            CoreVersionInstallState::Cancelled { context } => Self::Cancelled {
                context: context.into(),
            },
            CoreVersionInstallState::Failed { context, failure } => Self::Failed {
                context: context.into(),
                failure: map_application_failure(failure),
            },
        }
    }
}

impl From<CoreVersionInstallObservation> for AndroidCoreVersionInstallObservation {
    fn from(observation: CoreVersionInstallObservation) -> Self {
        match observation {
            CoreVersionInstallObservation::Found { state } => Self::Found {
                state: state.into(),
            },
            CoreVersionInstallObservation::NotFound { install_id } => Self::NotFound {
                install_id: install_id.get(),
            },
            CoreVersionInstallObservation::Unavailable { failure } => Self::Unavailable {
                failure: map_application_failure(failure),
            },
        }
    }
}

impl From<CoreVersionInstallCancelOutcome> for AndroidCoreVersionInstallCancellation {
    fn from(outcome: CoreVersionInstallCancelOutcome) -> Self {
        match outcome {
            CoreVersionInstallCancelOutcome::Requested { install_id } => Self::Requested {
                install_id: install_id.get(),
            },
            CoreVersionInstallCancelOutcome::AlreadyRequested { install_id } => {
                Self::AlreadyRequested {
                    install_id: install_id.get(),
                }
            }
            CoreVersionInstallCancelOutcome::AlreadyFinished { install_id } => {
                Self::AlreadyFinished {
                    install_id: install_id.get(),
                }
            }
            CoreVersionInstallCancelOutcome::NotFound { install_id } => Self::NotFound {
                install_id: install_id.get(),
            },
            CoreVersionInstallCancelOutcome::Unavailable { failure } => Self::Unavailable {
                failure: map_application_failure(failure),
            },
        }
    }
}

impl From<CoreVersionManagementResult> for AndroidCoreVersionManagementReadback {
    fn from(result: CoreVersionManagementResult) -> Self {
        match result {
            CoreVersionManagementResult::InstalledVersions {
                engine_id,
                versions,
            } => Self::InstalledVersions {
                engine_id: engine_id.as_str().to_owned(),
                versions: versions.into_iter().map(Into::into).collect(),
            },
            CoreVersionManagementResult::Releases {
                engine_id,
                releases,
            } => Self::Releases {
                engine_id: engine_id.as_str().to_owned(),
                releases: releases.into_iter().map(Into::into).collect(),
            },
            CoreVersionManagementResult::Latest { engine_id, release } => Self::Latest {
                engine_id: engine_id.as_str().to_owned(),
                release: release.into(),
            },
            CoreVersionManagementResult::InstallAccepted { receipt, state } => {
                Self::InstallAccepted {
                    install_id: receipt.install_id.get(),
                    state: state.into(),
                }
            }
            CoreVersionManagementResult::Activated { engine_id, version } => Self::Activated {
                engine_id: engine_id.as_str().to_owned(),
                version,
            },
            CoreVersionManagementResult::Uninstalled { engine_id, version } => Self::Uninstalled {
                engine_id: engine_id.as_str().to_owned(),
                version,
            },
            CoreVersionManagementResult::RolledBack {
                engine_id,
                selected_version,
            } => Self::RolledBack {
                engine_id: engine_id.as_str().to_owned(),
                selected_version,
            },
        }
    }
}

pub(super) async fn execute(
    operation: AndroidCoreVersionManagementOperation,
) -> AndroidCoreVersionManagementReadback {
    let application = match core_version_management_application() {
        Ok(application) => application,
        Err(failure) => return AndroidCoreVersionManagementReadback::Failed { failure },
    };
    match operation {
        AndroidCoreVersionManagementOperation::ObserveInstall { install_id } => {
            match CoreVersionInstallId::new(install_id) {
                Ok(install_id) => AndroidCoreVersionManagementReadback::InstallObservation {
                    observation: application.read_install(install_id).into(),
                },
                Err(error) => failed(format!("invalid Core version install ID: {error:?}")),
            }
        }
        AndroidCoreVersionManagementOperation::CancelInstall { install_id } => {
            match CoreVersionInstallId::new(install_id) {
                Ok(install_id) => AndroidCoreVersionManagementReadback::InstallCancellation {
                    outcome: application.cancel_install(install_id).into(),
                },
                Err(error) => failed(format!("invalid Core version install ID: {error:?}")),
            }
        }
        request_operation => match request_operation.into_application_request() {
            Ok(request) => match application.execute(request).await {
                Ok(result) => result.into(),
                Err(failure) => AndroidCoreVersionManagementReadback::Failed {
                    failure: map_application_failure(failure),
                },
            },
            Err(failure) => AndroidCoreVersionManagementReadback::Failed { failure },
        },
    }
}

fn failed(message: String) -> AndroidCoreVersionManagementReadback {
    AndroidCoreVersionManagementReadback::Failed {
        failure: FfiStatus::err(FfiErrorCode::InvalidInput, message),
    }
}

#[cfg(test)]
#[path = "version_tests.rs"]
mod tests;
