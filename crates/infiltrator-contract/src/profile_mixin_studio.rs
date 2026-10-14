//! Engine-authored, format-neutral Mixin Studio observations.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMixinToggleId {
    Ipv6,
    AllowLan,
    DnsFakeIp,
    Tun,
    Sniffer,
    DebugLog,
}

impl ProfileMixinToggleId {
    pub const ALL: [Self; 6] = [
        Self::Ipv6,
        Self::AllowLan,
        Self::DnsFakeIp,
        Self::Tun,
        Self::Sniffer,
        Self::DebugLog,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ipv6 => "ipv6",
            Self::AllowLan => "allow-lan",
            Self::DnsFakeIp => "dns-fake-ip",
            Self::Tun => "tun",
            Self::Sniffer => "sniffer",
            Self::DebugLog => "log-level-debug",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMixinColumnRole {
    Base,
    Overlay,
    Composed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileMixinColumn {
    pub role: ProfileMixinColumnRole,
    pub content: String,
    pub editable: bool,
    pub line_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMixinCascadeStageId {
    Base,
    Subscription,
    Merge,
    PreMixin,
    PostMixin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMixinStageDeclaration {
    Applied,
    Undeclared,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileMixinCascadeStage {
    pub id: ProfileMixinCascadeStageId,
    pub declaration: ProfileMixinStageDeclaration,
    pub line_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMixinPreflight {
    Accepted,
    Blocked { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMixinCascade {
    Merged {
        stages: Vec<ProfileMixinCascadeStage>,
        line_count: usize,
    },
    Blocked {
        stages: Vec<ProfileMixinCascadeStage>,
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMixinComposition {
    Produced(ProfileMixinColumn),
    Blocked { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileMixinColumns {
    pub base: ProfileMixinColumn,
    pub overlay: ProfileMixinColumn,
    pub composed: ProfileMixinComposition,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileMixinToggle {
    pub id: ProfileMixinToggleId,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMixinToggleObservation {
    Available(Vec<ProfileMixinToggle>),
    Unavailable { reason: String },
}

/// Schema-adapter facts for one Mixin Studio read; presentation supplies the copy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileMixinStudioObservation {
    pub preflight: ProfileMixinPreflight,
    pub cascade: ProfileMixinCascade,
    pub columns: ProfileMixinColumns,
    pub toggles: ProfileMixinToggleObservation,
}

#[cfg(test)]
#[path = "profile_mixin_studio_tests.rs"]
mod tests;
