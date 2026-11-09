//! The typed presentation policy of the desktop navigation column.
//!
//! FlClash's real desktop scene and its open-source `prism_ui` sidebar both
//! publish one uninterrupted list of destination rows. A group heading is not
//! navigation ink: it spends vertical rows twice (its own line box and the air
//! above it) and turns one navigation stack into two visually detached stacks.
//! The contract keeps group membership as a registry fact, but owns the
//! separate question of whether a surface paints that membership.

use crate::responsive_viewport::{SidebarForm, ViewportTier};

/// How a navigation surface states the boundary between two registry groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavigationSeparator {
    /// One uninterrupted destination list, measured against FlClash desktop.
    None,
    /// A localized heading plus its own air above and below the heading.
    Heading,
}

impl NavigationSeparator {
    pub const ALL: [Self; 2] = [Self::None, Self::Heading];

    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Heading => "heading",
        }
    }

    pub const fn paints_heading(self) -> bool {
        match self {
            Self::None => false,
            Self::Heading => true,
        }
    }
}

/// The separator a navigation form is allowed to paint.
pub const fn separator_for_form(form: SidebarForm) -> NavigationSeparator {
    match form {
        SidebarForm::BottomNav | SidebarForm::Rail | SidebarForm::Standard | SidebarForm::Wide => {
            NavigationSeparator::None
        }
    }
}

/// The separator this viewport's desktop navigation column paints.
pub const fn separator_for_tier(tier: ViewportTier) -> NavigationSeparator {
    separator_for_form(tier.sidebar_form())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::responsive_viewport::TIERS;

    #[test]
    fn every_desktop_tier_uses_the_reference_navigation_rhythm() {
        for tier in TIERS {
            assert_eq!(separator_for_tier(tier), NavigationSeparator::None);
            assert!(!separator_for_tier(tier).paints_heading());
        }
    }

    #[test]
    fn the_separator_vocabulary_is_stable() {
        assert_eq!(
            NavigationSeparator::ALL.map(NavigationSeparator::id),
            ["none", "heading"]
        );
        assert_eq!(
            separator_for_form(SidebarForm::Wide).id(),
            NavigationSeparator::None.id()
        );
    }
}
