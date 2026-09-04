//! Standardized 4-tier responsive breakpoint and multi-end adaptive layout engine.
//!
//! Charter (docs/BEVY_UI_FRONTEND.md): Multi-end adaptive presentation layer for
//! Bevy UI. Standardizes four breakpoint tiers (Compact/Medium/Expanded/Ultra),
//! density modes (Compact/Comfortable), orientations, adaptive sidebar/nav modes,
//! master-detail coordination models, and modal-to-actionsheet transformations.

use bevy::ecs::event::Event;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::window::{PrimaryWindow, Window};

use crate::theme::Breakpoint;

/// Layout density setting for scaling spacing, padding, row heights, and control sizes.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Density {
    /// Compact density: tighter padding, lower control heights, and higher information density.
    Compact,
    /// Comfortable density: standard spacing, touch-friendly touch targets, standard heights.
    #[default]
    Comfortable,
}

impl Density {
    /// Whether current density is Compact.
    pub fn is_compact(&self) -> bool {
        matches!(self, Density::Compact)
    }

    /// Whether current density is Comfortable.
    pub fn is_comfortable(&self) -> bool {
        matches!(self, Density::Comfortable)
    }

    /// Scale factor for dimensions (0.85 for Compact, 1.0 for Comfortable).
    pub fn scale_factor(&self) -> f32 {
        match self {
            Density::Compact => 0.85,
            Density::Comfortable => 1.0,
        }
    }

    /// Compute density-adjusted padding.
    pub fn padding(&self, base_px: f32) -> f32 {
        match self {
            Density::Compact => (base_px * 0.75).round(),
            Density::Comfortable => base_px,
        }
    }

    /// Compute density-adjusted gap.
    pub fn gap(&self, base_px: f32) -> f32 {
        match self {
            Density::Compact => (base_px * 0.75).round(),
            Density::Comfortable => base_px,
        }
    }

    /// Compute density-adjusted row height.
    pub fn row_height(&self, base_px: f32) -> f32 {
        match self {
            Density::Compact => (base_px * 0.82).round(),
            Density::Comfortable => base_px,
        }
    }

    /// Control height for current density (28.0 px Compact, 36.0 px Comfortable).
    pub fn control_height(&self) -> f32 {
        match self {
            Density::Compact => crate::theme::metrics::CONTROL_HEIGHT_COMPACT,
            Density::Comfortable => crate::theme::metrics::CONTROL_HEIGHT_COMFORTABLE,
        }
    }
}

/// Screen or viewport orientation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Orientation {
    /// Height > Width (Portrait).
    Portrait,
    /// Width >= Height (Landscape).
    #[default]
    Landscape,
}

impl Orientation {
    /// Derive orientation from width and height dimensions.
    pub fn from_dimensions(width_px: f32, height_px: f32) -> Self {
        if height_px > width_px {
            Orientation::Portrait
        } else {
            Orientation::Landscape
        }
    }

    pub fn is_portrait(&self) -> bool {
        matches!(self, Orientation::Portrait)
    }

    pub fn is_landscape(&self) -> bool {
        matches!(self, Orientation::Landscape)
    }
}

/// Adaptive navigation / sidebar presentation mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SidebarMode {
    /// Collapsed sidebar into horizontal bottom navigation bar (Compact / Mobile).
    BottomNav,
    /// Slim icon rail (Medium / Tablet).
    Rail,
    /// Standard 240px sidebar with labels and mode segment (Expanded / Desktop).
    #[default]
    Standard,
    /// Roomy 280px sidebar with rich widgets (Ultra / Widescreen).
    Wide,
}

impl SidebarMode {
    /// Recommended width in pixels for the sidebar in this mode (None for BottomNav).
    pub fn width_px(&self) -> Option<f32> {
        match self {
            SidebarMode::BottomNav => None,
            SidebarMode::Rail => Some(72.0),
            SidebarMode::Standard => Some(240.0),
            SidebarMode::Wide => Some(280.0),
        }
    }

    pub fn is_bottom_nav(&self) -> bool {
        matches!(self, SidebarMode::BottomNav)
    }

    pub fn is_rail(&self) -> bool {
        matches!(self, SidebarMode::Rail)
    }

    pub fn is_standard(&self) -> bool {
        matches!(self, SidebarMode::Standard)
    }

    pub fn is_wide(&self) -> bool {
        matches!(self, SidebarMode::Wide)
    }
}

/// Adaptive Master-Detail layout strategy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MasterDetailMode {
    /// Single-pane navigation (Stacked): either Master list or Detail view visible, with back button.
    #[default]
    Stacked,
    /// Dual-pane side-by-side split screen: Master pane on left, Detail pane on right.
    Split,
}

/// Adaptive modal dialog morphology.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ModalForm {
    /// Bottom-docked ActionSheet for mobile/compact touch screens.
    ActionSheet,
    /// Centered floating Dialog card for medium/expanded/ultra desktop screens.
    #[default]
    CenteredDialog,
}

/// Global responsive context resource providing layout metrics and multi-end adaptive state.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ResponsiveContext {
    /// Current viewport / window width in pixels.
    pub width_px: f32,
    /// Current viewport / window height in pixels.
    pub height_px: f32,
    /// 4-tier breakpoint category.
    pub breakpoint: Breakpoint,
    /// Active layout density.
    pub density: Density,
    /// Active viewport orientation.
    pub orientation: Orientation,
}

impl Default for ResponsiveContext {
    fn default() -> Self {
        Self::new(1180.0, 760.0)
    }
}

impl ResponsiveContext {
    /// Construct a new ResponsiveContext with default comfortable density.
    pub fn new(width_px: f32, height_px: f32) -> Self {
        let breakpoint = Breakpoint::from_width(width_px);
        let orientation = Orientation::from_dimensions(width_px, height_px);
        Self {
            width_px,
            height_px,
            breakpoint,
            density: Density::Comfortable,
            orientation,
        }
    }

    /// Set explicit density on construction.
    pub fn with_density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    /// Update dimensions, recomputing breakpoint and orientation. Returns true if changed.
    pub fn set_dimensions(&mut self, width_px: f32, height_px: f32) -> bool {
        let breakpoint = Breakpoint::from_width(width_px);
        let orientation = Orientation::from_dimensions(width_px, height_px);
        if (self.width_px - width_px).abs() > 0.5
            || (self.height_px - height_px).abs() > 0.5
            || self.breakpoint != breakpoint
            || self.orientation != orientation
        {
            self.width_px = width_px;
            self.height_px = height_px;
            self.breakpoint = breakpoint;
            self.orientation = orientation;
            true
        } else {
            false
        }
    }

    /// Update density. Returns true if changed.
    pub fn set_density(&mut self, density: Density) -> bool {
        if self.density != density {
            self.density = density;
            true
        } else {
            false
        }
    }

    /// Whether current breakpoint is Compact (<600px).
    pub fn is_compact(&self) -> bool {
        self.breakpoint.is_compact()
    }

    /// Whether current breakpoint is Medium (600px..1024px).
    pub fn is_medium(&self) -> bool {
        self.breakpoint.is_medium()
    }

    /// Whether current breakpoint is Expanded (1024px..1440px).
    pub fn is_expanded(&self) -> bool {
        self.breakpoint.is_expanded()
    }

    /// Whether current breakpoint is Ultra (>=1440px).
    pub fn is_ultra(&self) -> bool {
        self.breakpoint.is_ultra()
    }

    /// Whether orientation is Portrait.
    pub fn is_portrait(&self) -> bool {
        self.orientation.is_portrait()
    }

    /// Whether orientation is Landscape.
    pub fn is_landscape(&self) -> bool {
        self.orientation.is_landscape()
    }

    /// Derive appropriate sidebar mode based on breakpoint.
    pub fn sidebar_mode(&self) -> SidebarMode {
        match self.breakpoint {
            Breakpoint::Compact => SidebarMode::BottomNav,
            Breakpoint::Medium => SidebarMode::Rail,
            Breakpoint::Expanded => SidebarMode::Standard,
            Breakpoint::Ultra => SidebarMode::Wide,
        }
    }

    /// Derive Master-Detail layout mode: Stacked on Compact, Split on Medium/Expanded/Ultra.
    pub fn master_detail_mode(&self) -> MasterDetailMode {
        match self.breakpoint {
            Breakpoint::Compact => MasterDetailMode::Stacked,
            Breakpoint::Medium | Breakpoint::Expanded | Breakpoint::Ultra => {
                MasterDetailMode::Split
            }
        }
    }

    /// Derive Modal form: ActionSheet on Compact, CenteredDialog on Medium/Expanded/Ultra.
    pub fn modal_form(&self) -> ModalForm {
        match self.breakpoint {
            Breakpoint::Compact => ModalForm::ActionSheet,
            Breakpoint::Medium | Breakpoint::Expanded | Breakpoint::Ultra => {
                ModalForm::CenteredDialog
            }
        }
    }

    /// Compute dynamic grid columns for card collections based on container width and card minimum width.
    pub fn grid_columns_for_card_width(&self, min_card_width: f32, gap: f32) -> usize {
        let available_width = self.width_px - (self.content_padding() * 2.0);
        let sidebar_w = self.sidebar_mode().width_px().unwrap_or(0.0);
        let content_w = (available_width - sidebar_w).max(200.0);
        let cols = ((content_w + gap) / (min_card_width + gap)).floor() as usize;
        cols.clamp(1, 8)
    }

    /// Recommended page content padding.
    pub fn content_padding(&self) -> f32 {
        let base = match self.breakpoint {
            Breakpoint::Compact => 12.0,
            Breakpoint::Medium => 16.0,
            Breakpoint::Expanded => 20.0,
            Breakpoint::Ultra => 24.0,
        };
        self.density.padding(base)
    }

    /// Compute density-scaled padding for any base value.
    pub fn density_padding(&self, base_px: f32) -> f32 {
        self.density.padding(base_px)
    }

    /// Compute density-scaled gap for any base value.
    pub fn density_gap(&self, base_px: f32) -> f32 {
        self.density.gap(base_px)
    }

    /// Compute density-scaled control height.
    pub fn density_control_height(&self) -> f32 {
        self.density.control_height()
    }
}

/// Event triggering a density switch.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DensitySwitch(pub Density);

/// Observer to apply DensitySwitch events to ResponsiveContext.
pub fn on_density_switch(
    trigger: On<DensitySwitch>,
    mut ctx: Option<ResMut<ResponsiveContext>>,
    mut density: Option<ResMut<Density>>,
) {
    if let Some(ref mut ctx) = ctx {
        ctx.set_density(trigger.0);
    }
    if let Some(ref mut d) = density {
        **d = trigger.0;
    }
}

/// System to sync PrimaryWindow size into ResponsiveContext and Breakpoint.
pub fn sync_responsive_context_from_window(
    windows: Option<Query<&Window, With<PrimaryWindow>>>,
    mut ctx: Option<ResMut<ResponsiveContext>>,
    mut bp: Option<ResMut<Breakpoint>>,
) {
    let Some(windows) = windows else { return };
    let Ok(primary) = windows.single() else {
        return;
    };
    let w = primary.width();
    let h = primary.height();

    if let Some(ref mut ctx) = ctx {
        if ctx.set_dimensions(w, h)
            && let Some(ref mut bp) = bp
        {
            **bp = ctx.breakpoint;
        }
    } else if let Some(ref mut bp) = bp {
        let next_bp = Breakpoint::from_width(w);
        if **bp != next_bp {
            **bp = next_bp;
        }
    }
}

use bevy::ui::UiRect;
use bevy::ui::Val;

/// Platform edge-to-edge safe area insets (status bar, gesture pill, camera cutouts).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Default)]
pub struct SafeAreaInsets {
    pub top_px: f32,
    pub bottom_px: f32,
    pub left_px: f32,
    pub right_px: f32,
}

impl SafeAreaInsets {
    /// Create insets with explicit edge values.
    pub fn new(top: f32, bottom: f32, left: f32, right: f32) -> Self {
        Self {
            top_px: top.max(0.0),
            bottom_px: bottom.max(0.0),
            left_px: left.max(0.0),
            right_px: right.max(0.0),
        }
    }

    /// Common Android phone default insets (24px status bar, 16px gesture bar).
    pub fn android_default() -> Self {
        Self::new(24.0, 16.0, 0.0, 0.0)
    }

    /// Common iOS phone default insets with Dynamic Island (48px status bar, 34px home indicator).
    pub fn ios_default() -> Self {
        Self::new(48.0, 34.0, 0.0, 0.0)
    }

    /// Convert insets to a Bevy UiRect.
    pub fn to_ui_rect(&self) -> UiRect {
        UiRect {
            left: Val::Px(self.left_px),
            right: Val::Px(self.right_px),
            top: Val::Px(self.top_px),
            bottom: Val::Px(self.bottom_px),
        }
    }

    /// Add insets on top of base page padding.
    pub fn pad_base(&self, base_px: f32) -> UiRect {
        UiRect {
            left: Val::Px(base_px + self.left_px),
            right: Val::Px(base_px + self.right_px),
            top: Val::Px(base_px + self.top_px),
            bottom: Val::Px(base_px + self.bottom_px),
        }
    }
}

/// Minimum touch target policy enforcing WCAG / Android 48dp guidelines on mobile.
pub struct TouchTargetPolicy;

impl TouchTargetPolicy {
    /// Minimum touch target dimension in pixels.
    pub fn min_dimension(density: Density, is_compact: bool) -> f32 {
        if is_compact {
            // Mobile compact: enforce 48px touch target for thumb ergonomics
            48.0
        } else {
            match density {
                Density::Compact => 28.0,
                Density::Comfortable => 36.0,
            }
        }
    }
}
