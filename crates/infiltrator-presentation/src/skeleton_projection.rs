//! Presentation read-model projection for skeleton loading and emotional empty-state illustrations (FLPARITY-025).

use infiltrator_contract::skeleton_shimmer::{
    EmptyStateIllustrationKind, ShimmerAnimationSpec, SkeletonCardLayoutKind,
};

/// Presentation model for a skeleton card loading placeholder.
#[derive(Debug, Clone, PartialEq)]
pub struct SkeletonCardLayoutProjection {
    pub layout_kind: SkeletonCardLayoutKind,
    pub item_count: usize,
    pub card_height_px: f32,
    pub shimmer: ShimmerAnimationSpec,
    pub placeholder_title: &'static str,
}

impl SkeletonCardLayoutProjection {
    pub fn of(layout_kind: SkeletonCardLayoutKind, item_count: usize) -> Self {
        let (card_height_px, placeholder_title) = match layout_kind {
            SkeletonCardLayoutKind::ProxyNodeCard => (72.0, "加载节点列表..."),
            SkeletonCardLayoutKind::RuleRow => (48.0, "加载分流规则..."),
            SkeletonCardLayoutKind::ConnectionRow => (56.0, "加载网络连接..."),
            SkeletonCardLayoutKind::ProfileCard => (110.0, "加载订阅配置..."),
        };

        Self {
            layout_kind,
            item_count,
            card_height_px,
            shimmer: ShimmerAnimationSpec::default(),
            placeholder_title,
        }
    }
}

/// Presentation model for an emotional empty state with actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyStateProjection {
    pub kind: EmptyStateIllustrationKind,
    pub title: String,
    pub description: String,
    pub primary_action: Option<String>,
    pub icon_name: &'static str,
}

impl EmptyStateProjection {
    pub fn of(kind: EmptyStateIllustrationKind) -> Self {
        Self {
            kind,
            title: kind.title_zh().to_owned(),
            description: kind.description_zh().to_owned(),
            primary_action: Some(kind.primary_action_zh().to_owned()),
            icon_name: kind.icon_token(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_card_layout_projects_reasonable_dimensions() {
        let proj = SkeletonCardLayoutProjection::of(SkeletonCardLayoutKind::ProxyNodeCard, 8);
        assert_eq!(proj.item_count, 8);
        assert_eq!(proj.card_height_px, 72.0);
        assert_eq!(proj.placeholder_title, "加载节点列表...");
        assert!(proj.shimmer.cycle_duration_ms > 0);
    }

    #[test]
    fn empty_state_projects_all_kinds_consistently() {
        for kind in EmptyStateIllustrationKind::ALL {
            let proj = EmptyStateProjection::of(kind);
            assert!(!proj.title.is_empty());
            assert!(!proj.description.is_empty());
            assert!(proj.primary_action.is_some());
            assert!(!proj.icon_name.is_empty());
        }
    }
}
