//! Cross-surface core-version values.

use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::EngineId;
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::num::NonZeroU64;

/// Release channels whose meaning does not vary by core engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineNeutralReleaseChannel {
    Stable,
    Alpha,
}

impl EngineNeutralReleaseChannel {
    pub const ALL: [Self; 2] = [Self::Stable, Self::Alpha];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleaseChannelIdError {
    Empty,
    Invalid,
}

impl Display for ReleaseChannelIdError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("release channel ID must not be empty"),
            Self::Invalid => formatter.write_str(
                "release channel ID must use lowercase ASCII letters, digits, and hyphens",
            ),
        }
    }
}

impl Error for ReleaseChannelIdError {}

/// Stable identifier for a release channel, interpreted only by its engine.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ReleaseChannelId(String);

impl ReleaseChannelId {
    pub fn new(value: impl Into<String>) -> Result<Self, ReleaseChannelIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ReleaseChannelIdError::Empty);
        }
        if value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(ReleaseChannelIdError::Invalid);
        }
        Ok(Self(value))
    }

    pub fn stable() -> Self {
        Self("stable".to_owned())
    }

    pub fn alpha() -> Self {
        Self("alpha".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ReleaseChannelId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// A release channel name bound to exactly one engine namespace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineReleaseChannel {
    pub engine_id: EngineId,
    pub channel_id: ReleaseChannelId,
}

impl EngineReleaseChannel {
    pub fn new(engine_id: EngineId, channel_id: ReleaseChannelId) -> Self {
        Self {
            engine_id,
            channel_id,
        }
    }

    pub fn common(engine_id: EngineId, channel: EngineNeutralReleaseChannel) -> Self {
        let channel_id = match channel {
            EngineNeutralReleaseChannel::Stable => ReleaseChannelId::stable(),
            EngineNeutralReleaseChannel::Alpha => ReleaseChannelId::alpha(),
        };
        Self::new(engine_id, channel_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledCoreVersion {
    pub engine_id: EngineId,
    pub version: String,
    pub path: String,
    pub is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreRelease {
    pub engine_id: EngineId,
    pub version: String,
    pub release_date: String,
}

/// Result of probing one release channel supported by the selected engine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreChannelStatus {
    Ready { release: CoreRelease },
    Failed { failure: Failure },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreChannelSnapshot {
    pub engine_id: EngineId,
    pub channel: EngineReleaseChannel,
    pub status: CoreChannelStatus,
}

/// Local version-selection journal for one engine.
///
/// `target` is the first still-installed, runnable version that the local
/// rollback action can select. The history is bounded by the host adapter;
/// it contains metadata only, never binary contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreRollbackSnapshot {
    pub engine_id: EngineId,
    pub current: Option<String>,
    pub target: Option<String>,
    pub history: Vec<String>,
}

impl CoreRollbackSnapshot {
    pub fn for_engine(engine_id: EngineId) -> Self {
        Self {
            engine_id,
            current: None,
            target: None,
            history: Vec::new(),
        }
    }
}

/// Integrity result of the most recent core artifact installation attempt.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreArtifactVerification {
    #[default]
    Unknown,
    Verified {
        engine_id: EngineId,
        version: String,
    },
    Rejected {
        engine_id: EngineId,
        version: String,
        failure: Failure,
    },
}

/// Bounded result of one online probe across all supported core channels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreVersionSnapshot {
    pub engine_id: EngineId,
    pub revision: u64,
    pub channels: Vec<CoreChannelSnapshot>,
    #[serde(default)]
    pub channel_failure: Option<Failure>,
    #[serde(default)]
    pub verification: CoreArtifactVerification,
    pub rollback: CoreRollbackSnapshot,
}

impl CoreVersionSnapshot {
    pub fn unavailable() -> Self {
        let engine_id = EngineId::new("unavailable").expect("unavailable engine ID is valid");
        Self {
            engine_id: engine_id.clone(),
            revision: 0,
            channels: Vec::new(),
            channel_failure: Some(Failure::unsupported(
                "core version engine has not been selected",
            )),
            verification: CoreArtifactVerification::Unknown,
            rollback: CoreRollbackSnapshot::for_engine(engine_id),
        }
    }

    pub fn for_engine(engine_id: EngineId) -> Self {
        Self {
            engine_id: engine_id.clone(),
            revision: 0,
            channels: Vec::new(),
            channel_failure: None,
            verification: CoreArtifactVerification::Unknown,
            rollback: CoreRollbackSnapshot::for_engine(engine_id),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreReleaseSummary {
    pub engine_id: EngineId,
    pub version: String,
    pub name: String,
    pub published_at: String,
    pub prerelease: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionDownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// Stable identifier for one asynchronous, engine-scoped version install.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CoreVersionInstallId(NonZeroU64);

impl CoreVersionInstallId {
    pub fn new(value: u64) -> Result<Self, CoreVersionInstallIdError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(CoreVersionInstallIdError::Zero)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreVersionInstallIdError {
    Zero,
}

/// Identity and observation revision owned by one version-install task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreVersionInstallContext {
    pub install_id: CoreVersionInstallId,
    pub engine_id: EngineId,
    pub version: String,
    pub revision: u64,
}

/// Mutually exclusive lifecycle of a managed version installation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreVersionInstallState {
    Running {
        context: CoreVersionInstallContext,
        progress: Option<VersionDownloadProgress>,
    },
    CancelRequested {
        context: CoreVersionInstallContext,
        progress: Option<VersionDownloadProgress>,
    },
    Completed {
        context: CoreVersionInstallContext,
    },
    Cancelled {
        context: CoreVersionInstallContext,
    },
    Failed {
        context: CoreVersionInstallContext,
        failure: Failure,
    },
}

impl CoreVersionInstallState {
    pub fn context(&self) -> &CoreVersionInstallContext {
        match self {
            Self::Running { context, .. }
            | Self::CancelRequested { context, .. }
            | Self::Completed { context }
            | Self::Cancelled { context }
            | Self::Failed { context, .. } => context,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running { .. } | Self::CancelRequested { .. })
    }
}

/// Receipt that the owner accepted an install task for execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreVersionInstallReceipt {
    pub install_id: CoreVersionInstallId,
}

/// Terminal outcome of asking the owner to cancel one install task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreVersionInstallCancelOutcome {
    Requested { install_id: CoreVersionInstallId },
    AlreadyRequested { install_id: CoreVersionInstallId },
    AlreadyFinished { install_id: CoreVersionInstallId },
    NotFound { install_id: CoreVersionInstallId },
    Unavailable { failure: Failure },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreVersionInstallObservation {
    Found { state: CoreVersionInstallState },
    NotFound { install_id: CoreVersionInstallId },
    Unavailable { failure: Failure },
}

/// Engine-scoped version-management actions accepted by the shared owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreVersionManagementAction {
    ListInstalled,
    ListReleases {
        limit: u32,
    },
    Latest {
        channel: EngineNeutralReleaseChannel,
    },
    Install {
        version: String,
    },
    Activate {
        version: String,
    },
    Uninstall {
        version: String,
    },
    Rollback,
}

/// A command binds the action to exactly one engine namespace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreVersionManagementRequest {
    pub engine_id: EngineId,
    pub action: CoreVersionManagementAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreVersionManagementResult {
    InstalledVersions {
        engine_id: EngineId,
        versions: Vec<InstalledCoreVersion>,
    },
    Releases {
        engine_id: EngineId,
        releases: Vec<CoreReleaseSummary>,
    },
    Latest {
        engine_id: EngineId,
        release: CoreRelease,
    },
    InstallAccepted {
        receipt: CoreVersionInstallReceipt,
        state: CoreVersionInstallState,
    },
    Activated {
        engine_id: EngineId,
        version: String,
    },
    Uninstalled {
        engine_id: EngineId,
        version: String,
    },
    RolledBack {
        engine_id: EngineId,
        selected_version: String,
    },
}
