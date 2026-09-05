//! Cross-surface core-version values.

use crate::error::Failure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreReleaseChannel {
    Stable,
    Alpha,
    MetaCore,
}

impl CoreReleaseChannel {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "alpha" | "pre-release" | "prerelease" => Self::Alpha,
            "meta" | "meta-core" | "metacore" | "nightly" => Self::MetaCore,
            _ => Self::Stable,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Alpha => "alpha",
            Self::MetaCore => "meta-core",
        }
    }

    pub const ALL: [Self; 3] = [Self::Stable, Self::Alpha, Self::MetaCore];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledCoreVersion {
    pub version: String,
    pub path: String,
    pub is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreRelease {
    pub version: String,
    pub release_date: String,
}

/// Result of probing one official Mihomo release channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreChannelStatus {
    Ready { release: CoreRelease },
    Failed { failure: Failure },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreChannelSnapshot {
    pub channel: CoreReleaseChannel,
    pub status: CoreChannelStatus,
}

/// Integrity result of the most recent core artifact installation attempt.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreArtifactVerification {
    #[default]
    Unknown,
    Verified { version: String },
    Rejected { version: String, failure: Failure },
}

/// Bounded result of one online probe across all supported core channels.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreVersionSnapshot {
    pub revision: u64,
    pub channels: Vec<CoreChannelSnapshot>,
    #[serde(default)]
    pub verification: CoreArtifactVerification,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreReleaseSummary {
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
