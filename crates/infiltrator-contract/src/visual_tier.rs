//! The three visual quality tiers and the texture budget (EFFECTS-006 #24, #25).
//!
//! A surface that cannot afford a blurred shadow, a 4K texture and a live
//! particle field at once must not decide that for itself per screen: the
//! product declares three quality tiers once, and a surface renders the tier
//! it was given. The tiers bound the expensive things — texture edges, texture
//! counts, texture memory, shadow layers and blur radii — so a "simplified"
//! device degrades in a defined way instead of at random.

/// The three quality tiers the product renders at (EFFECTS-006 #25).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VisualTier {
    /// The cheapest tier: no blur, no shadow layer, the smallest texture set.
    Simplified,
    /// The default tier: one shadow layer, the full texture set.
    #[default]
    Standard,
    /// The richest tier: every shadow layer, the largest texture set.
    Enhanced,
}

impl VisualTier {
    /// Every quality tier, from the cheapest to the richest.
    pub const ALL: [Self; 3] = [Self::Simplified, Self::Standard, Self::Enhanced];

    /// The largest edge length of one texture, in pixels.
    pub const fn max_texture_edge_px(self) -> u32 {
        match self {
            Self::Simplified => 1024,
            Self::Standard => 2048,
            Self::Enhanced => 4096,
        }
    }

    /// The maximum number of textures the renderer may hold at once.
    pub const fn max_texture_count(self) -> u32 {
        match self {
            Self::Simplified => 64,
            Self::Standard => 192,
            Self::Enhanced => 512,
        }
    }

    /// The texture memory budget of the tier, in bytes.
    pub const fn texture_budget_bytes(self) -> u64 {
        match self {
            Self::Simplified => 32 * 1024 * 1024,
            Self::Standard => 96 * 1024 * 1024,
            Self::Enhanced => 256 * 1024 * 1024,
        }
    }

    /// How many shadow layers the tier paints.
    pub const fn shadow_layers(self) -> u32 {
        match self {
            Self::Simplified => 0,
            Self::Standard => 1,
            Self::Enhanced => 2,
        }
    }

    /// The largest blur radius the tier paints, in logical pixels.
    pub const fn max_blur_radius_px(self) -> f32 {
        match self {
            Self::Simplified => 0.0,
            Self::Standard => 12.0,
            Self::Enhanced => 32.0,
        }
    }

    /// Whether the tier paints the ambient (second) shadow layer.
    pub const fn paints_ambient_shadow(self) -> bool {
        self.shadow_layers() >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tier_ladder_rises() {
        assert_eq!(VisualTier::ALL.len(), 3);
        for pair in VisualTier::ALL.windows(2) {
            assert!(pair[0] < pair[1], "the tiers must be ordered");
        }
        for tier in VisualTier::ALL {
            assert!(tier.max_texture_edge_px() > 0);
            assert!(tier.max_texture_count() > 0);
            assert!(tier.texture_budget_bytes() > 0);
            assert!(tier.max_blur_radius_px() >= 0.0);
        }
    }

    #[test]
    fn the_budgets_grow_with_the_tier() {
        for pair in VisualTier::ALL.windows(2) {
            assert!(pair[0].max_texture_edge_px() < pair[1].max_texture_edge_px());
            assert!(pair[0].max_texture_count() < pair[1].max_texture_count());
            assert!(pair[0].texture_budget_bytes() < pair[1].texture_budget_bytes());
            assert!(pair[0].max_blur_radius_px() <= pair[1].max_blur_radius_px());
        }
    }

    #[test]
    fn the_simplified_tier_paints_no_shadow_and_no_blur() {
        assert_eq!(VisualTier::Simplified.shadow_layers(), 0);
        assert_eq!(VisualTier::Simplified.max_blur_radius_px(), 0.0);
        assert!(!VisualTier::Simplified.paints_ambient_shadow());
        assert_eq!(VisualTier::Standard.shadow_layers(), 1);
        assert!(!VisualTier::Standard.paints_ambient_shadow());
        assert_eq!(VisualTier::Enhanced.shadow_layers(), 2);
        assert!(VisualTier::Enhanced.paints_ambient_shadow());
    }

    #[test]
    fn the_default_tier_is_the_standard_one() {
        assert_eq!(VisualTier::default(), VisualTier::Standard);
    }
}
