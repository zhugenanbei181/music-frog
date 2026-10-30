//! Presentation projections for user-owned general overrides (FLPARITY-038).

use infiltrator_contract::general_override::{
    GeneralOverrideBackendCompatibility, GeneralOverrideItemSummary, GeneralOverridePhase,
    GeneralOverrideSnapshot,
};

/// Projected row for one general override item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralOverrideRowProjection {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub order: usize,
    pub phase_label: String,
    pub backend_badge: String,
    pub is_incompatible: bool,
}

/// Projected page model for the general override list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralOverridePageProjection {
    pub rows: Vec<GeneralOverrideRowProjection>,
    pub total_count: usize,
    pub enabled_count: usize,
    pub count_headline: String,
    pub is_empty: bool,
    pub empty_placeholder: String,
}

/// Project a `GeneralOverrideSnapshot` into a presentation-layer `GeneralOverridePageProjection`.
pub fn project_general_overrides(
    snapshot: &GeneralOverrideSnapshot,
) -> GeneralOverridePageProjection {
    let rows: Vec<_> = snapshot
        .items
        .iter()
        .map(project_general_override_row)
        .collect();
    let total_count = snapshot.total_count;
    let enabled_count = snapshot.enabled_count;
    let is_empty = total_count == 0;
    let count_headline = if is_empty {
        "暂无通用覆写".to_owned()
    } else {
        format!("通用覆写 ({total_count} 项，{enabled_count} 启用)")
    };
    let empty_placeholder =
        "添加通用覆写以修改 Mihomo/sing-box 配置；覆写按顺序在引擎启动前或配置载入后应用"
            .to_owned();

    GeneralOverridePageProjection {
        rows,
        total_count,
        enabled_count,
        count_headline,
        is_empty,
        empty_placeholder,
    }
}

fn project_general_override_row(item: &GeneralOverrideItemSummary) -> GeneralOverrideRowProjection {
    let phase_label = match item.phase {
        GeneralOverridePhase::PreEngineLaunch => "引擎启动前",
        GeneralOverridePhase::PostConfigLoad => "配置载入后",
    }
    .to_owned();
    let (backend_badge, is_incompatible) = match &item.backend_compatibility {
        GeneralOverrideBackendCompatibility::Universal => ("通用".to_owned(), false),
        GeneralOverrideBackendCompatibility::MihomoOnly { feature } => {
            (format!("仅Mihomo ({feature})"), false)
        }
        GeneralOverrideBackendCompatibility::SingBoxOnly { feature } => {
            (format!("仅sing-box ({feature})"), false)
        }
        GeneralOverrideBackendCompatibility::Incompatible { reason } => {
            (format!("不兼容: {reason}"), true)
        }
    };

    GeneralOverrideRowProjection {
        id: item.id.clone(),
        name: item.name.clone(),
        enabled: item.enabled,
        order: item.order,
        phase_label,
        backend_badge,
        is_incompatible,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use GeneralOverrideBackendCompatibility;

    #[test]
    fn project_empty_general_overrides() {
        let snapshot = GeneralOverrideSnapshot::default();
        let proj = project_general_overrides(&snapshot);
        assert!(proj.is_empty);
        assert_eq!(proj.total_count, 0);
        assert_eq!(proj.count_headline, "暂无通用覆写");
    }

    #[test]
    fn project_populated_general_overrides() {
        let snapshot = GeneralOverrideSnapshot {
            items: vec![GeneralOverrideItemSummary {
                id: "ov1".into(),
                name: "Sniffer Config".into(),
                enabled: true,
                order: 0,
                phase: GeneralOverridePhase::PreEngineLaunch,
                backend_compatibility: GeneralOverrideBackendCompatibility::MihomoOnly {
                    feature: "sniffer".into(),
                },
                updated_at_epoch_ms: 1000,
            }],
            total_count: 1,
            enabled_count: 1,
            revision: 1,
            last_applied_epoch_ms: 1000,
        };

        let proj = project_general_overrides(&snapshot);
        assert!(!proj.is_empty);
        assert_eq!(proj.total_count, 1);
        assert_eq!(proj.enabled_count, 1);
        assert_eq!(proj.count_headline, "通用覆写 (1 项，1 启用)");
        assert_eq!(proj.rows[0].id, "ov1");
        assert_eq!(proj.rows[0].phase_label, "引擎启动前");
        assert_eq!(proj.rows[0].backend_badge, "仅Mihomo (sniffer)");
        assert!(!proj.rows[0].is_incompatible);
    }
}
