use serde::{Deserialize, Serialize};

/// Application phase determining when the general override is merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GeneralOverridePhase {
    /// Applied before the core engine process launches (modifies listen ports, tun, external controller, dns).
    #[default]
    PreEngineLaunch,
    /// Applied during profile config assembly (overrides routing rules, proxy groups, outbound chains).
    PostConfigLoad,
}

impl GeneralOverridePhase {
    /// Localized Chinese display label.
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::PreEngineLaunch => "引擎启动前",
            Self::PostConfigLoad => "配置载入后",
        }
    }

    /// Localized English display label.
    pub const fn label_en(self) -> &'static str {
        match self {
            Self::PreEngineLaunch => "Pre-Engine Launch",
            Self::PostConfigLoad => "Post-Config Load",
        }
    }
}

/// Compatibility assessment of an override snippet with core engine backends.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GeneralOverrideBackendCompatibility {
    /// Fully compatible with both Mihomo and sing-box engines.
    #[default]
    Universal,
    /// Requires Mihomo features (e.g. sniffer, geodata-mode, specific tun stacks).
    MihomoOnly { feature: String },
    /// Requires sing-box features.
    SingBoxOnly { feature: String },
    /// Incompatible with all supported engines.
    Incompatible { reason: String },
}

impl GeneralOverrideBackendCompatibility {
    /// Returns true if compatible with the requested engine identifier.
    pub fn is_compatible_with(&self, engine: &str) -> bool {
        match self {
            Self::Universal => true,
            Self::MihomoOnly { .. } => engine.eq_ignore_ascii_case("mihomo"),
            Self::SingBoxOnly { .. } => {
                engine.eq_ignore_ascii_case("singbox") || engine.eq_ignore_ascii_case("sing-box")
            }
            Self::Incompatible { .. } => false,
        }
    }

    /// Localized Chinese badge label.
    pub fn badge_zh(&self) -> String {
        match self {
            Self::Universal => "通用".to_owned(),
            Self::MihomoOnly { feature } => format!("仅Mihomo ({feature})"),
            Self::SingBoxOnly { feature } => format!("仅sing-box ({feature})"),
            Self::Incompatible { .. } => "不兼容".to_owned(),
        }
    }
}

/// One general override item in the user-owned collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralOverrideItem {
    /// Stable item identifier.
    pub id: String,
    /// User-facing descriptive name.
    pub name: String,
    /// YAML or JSON override configuration snippet.
    pub content: String,
    /// Whether this override is active.
    pub enabled: bool,
    /// Application order index (lower values apply first).
    pub order: usize,
    /// Targeted application phase.
    pub phase: GeneralOverridePhase,
    /// Engine compatibility classification.
    pub backend_compatibility: GeneralOverrideBackendCompatibility,
    /// Epoch timestamp in milliseconds of the last modification.
    pub updated_at_epoch_ms: u64,
}

impl GeneralOverrideItem {
    /// Produce a lightweight summary for safe UI rendering.
    pub fn to_summary(&self) -> GeneralOverrideItemSummary {
        GeneralOverrideItemSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            enabled: self.enabled,
            order: self.order,
            phase: self.phase,
            backend_compatibility: self.backend_compatibility.clone(),
            updated_at_epoch_ms: self.updated_at_epoch_ms,
        }
    }
}

/// Summary representation of a general override item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralOverrideItemSummary {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub order: usize,
    pub phase: GeneralOverridePhase,
    pub backend_compatibility: GeneralOverrideBackendCompatibility,
    pub updated_at_epoch_ms: u64,
}

/// Complete collection of user-owned general overrides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GeneralOverrideCollection {
    pub items: Vec<GeneralOverrideItem>,
    pub revision: u64,
    pub last_applied_epoch_ms: u64,
}

/// Typed command to insert or update a general override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertGeneralOverrideCommand {
    pub item: GeneralOverrideItem,
    pub expected_revision: Option<u64>,
}

/// Typed command to toggle an override item enabled/disabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToggleGeneralOverrideCommand {
    pub id: String,
    pub enabled: bool,
    pub expected_revision: Option<u64>,
}

/// Typed command to reorder the general overrides by ID list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReorderGeneralOverridesCommand {
    pub ordered_ids: Vec<String>,
    pub expected_revision: Option<u64>,
}

/// Typed command to remove an override item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoveGeneralOverrideCommand {
    pub id: String,
    pub expected_revision: Option<u64>,
}

/// Typed receipt for general override mutation operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GeneralOverrideMutationReceipt {
    Success { id: String, revision: u64 },
    Reordered { revision: u64 },
    NotFound { id: String },
    DuplicateName { name: String },
    InvalidContent { reason: String },
    StaleRevision { expected: u64, actual: u64 },
}

/// Typed receipt for general override application execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GeneralOverrideApplyReceipt {
    Applied {
        applied_count: usize,
        revision: u64,
        phase: GeneralOverridePhase,
    },
    SkippedNoOverrides {
        phase: GeneralOverridePhase,
    },
    BackendUnsupported {
        engine: String,
        feature: String,
    },
    ParseFailure {
        id: String,
        reason: String,
    },
}

/// Readback snapshot of general overrides across all surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GeneralOverrideSnapshot {
    pub items: Vec<GeneralOverrideItemSummary>,
    pub total_count: usize,
    pub enabled_count: usize,
    pub revision: u64,
    pub last_applied_epoch_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_checks() {
        let univ = GeneralOverrideBackendCompatibility::Universal;
        assert!(univ.is_compatible_with("mihomo"));
        assert!(univ.is_compatible_with("singbox"));
        assert_eq!(univ.badge_zh(), "通用");

        let mihomo = GeneralOverrideBackendCompatibility::MihomoOnly {
            feature: "sniffer".into(),
        };
        assert!(mihomo.is_compatible_with("mihomo"));
        assert!(!mihomo.is_compatible_with("singbox"));
        assert!(mihomo.badge_zh().contains("仅Mihomo"));

        let singbox = GeneralOverrideBackendCompatibility::SingBoxOnly {
            feature: "v2ray-transport".into(),
        };
        assert!(!singbox.is_compatible_with("mihomo"));
        assert!(singbox.is_compatible_with("sing-box"));
    }

    #[test]
    fn phase_labels() {
        assert_eq!(
            GeneralOverridePhase::PreEngineLaunch.label_zh(),
            "引擎启动前"
        );
        assert_eq!(
            GeneralOverridePhase::PostConfigLoad.label_zh(),
            "配置载入后"
        );
    }
}
