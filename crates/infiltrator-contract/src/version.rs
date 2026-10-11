//! Cross-surface core-version values.

use crate::core_identity::EngineId;
use crate::error::Failure;
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

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
