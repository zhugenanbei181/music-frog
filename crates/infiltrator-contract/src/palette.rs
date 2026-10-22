//! Semantic colour facts: what a colour *means* in this product.
//!
//! The skin palettes in this crate answer "which pixels"; this module answers
//! "which role". A surface asks for a role ([`SemanticToken`]) and receives
//! the colour the contract resolved for the active skin, so two surfaces can
//! no longer disagree about what "the warning colour" is.
//!
//! The module carries five facts:
//!
//! * the eight-step neutral surface ladder of one skin (#1);
//! * the two-stop linear gradient specification (#2);
//! * the highlight stroke and the translucent overlay roles (#3);
//! * the latency and lifecycle status colour ramps (#4);
//! * the WCAG text contrast ladder and the ratio measurement (#5).
//!
//! Nothing here invents a colour number a surface could have picked itself:
//! every value is derived from the skin palettes of the parent module, so a
//! change to a skin propagates to every role at once.

use crate::design_tokens::{RgbaToken, metrics, skin_core, skin_interaction};
use crate::latency_display::LatencyBand;
use crate::theme::ThemeSkin;

/// The eight neutral surface steps of one skin (EFFECTS-001 #1).
///
/// The ladder is the product's neutral scale: a surface that needs "one step
/// above the card" looks the role up here instead of inventing a grey. The
/// first three steps are the skin's canvas, rail and card, so the ladder can
/// never drift away from the core palette it is derived from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SurfaceStep {
    /// The window backdrop.
    Canvas,
    /// The navigation rail beside the canvas.
    Rail,
    /// The resting card or panel fill.
    Card,
    /// A control resting on a card (segmented track, input, chip).
    Control,
    /// A recessed well inside a card.
    Sunken,
    /// The deepest well (the track of a segmented control).
    Inset,
    /// A floating layer above the page (menu, popover, tooltip).
    Overlay,
    /// The lit edge of a raised surface.
    Highlight,
}

impl SurfaceStep {
    /// Every step of the ladder, from the backdrop to the lit edge.
    pub const ALL: [Self; 8] = [
        Self::Canvas,
        Self::Rail,
        Self::Card,
        Self::Control,
        Self::Sunken,
        Self::Inset,
        Self::Overlay,
        Self::Highlight,
    ];

    /// The position of this step in the ladder.
    pub const fn index(self) -> usize {
        match self {
            Self::Canvas => 0,
            Self::Rail => 1,
            Self::Card => 2,
            Self::Control => 3,
            Self::Sunken => 4,
            Self::Inset => 5,
            Self::Overlay => 6,
            Self::Highlight => 7,
        }
    }
}

/// The eight neutral surface steps of one skin, ordered backdrop → lit edge.
///
/// The first three steps are the skin's canvas, rail and card, taken straight
/// from the core palette; the five raised steps extend that scale downwards.
pub const fn surface_ladder(skin: ThemeSkin) -> [RgbaToken; 8] {
    let core = skin_core(skin);
    match skin {
        ThemeSkin::Dark => [
            core.canvas,
            core.sidebar,
            core.card,
            RgbaToken::rgb(0.176, 0.192, 0.204),
            RgbaToken::rgb(0.118, 0.129, 0.137),
            RgbaToken::rgb(0.098, 0.106, 0.114),
            RgbaToken::rgb(0.196, 0.212, 0.224),
            RgbaToken::rgb(0.267, 0.286, 0.298),
        ],
        ThemeSkin::Light => [
            core.canvas,
            core.sidebar,
            core.card,
            RgbaToken::rgb(0.910, 0.914, 0.910),
            RgbaToken::rgb(0.925, 0.929, 0.925),
            RgbaToken::rgb(0.902, 0.906, 0.902),
            RgbaToken::rgb(0.976, 0.980, 0.976),
            RgbaToken::rgb(1.0, 1.0, 1.0),
        ],
        ThemeSkin::Forest => [
            core.canvas,
            core.sidebar,
            core.card,
            RgbaToken::rgb(0.906, 0.941, 0.898),
            RgbaToken::rgb(0.886, 0.925, 0.878),
            RgbaToken::rgb(0.867, 0.910, 0.859),
            RgbaToken::rgb(0.984, 0.992, 0.976),
            RgbaToken::rgb(1.0, 1.0, 1.0),
        ],
        ThemeSkin::Amoled => [
            core.canvas,
            core.sidebar,
            core.card,
            RgbaToken::rgb(0.110, 0.125, 0.137),
            RgbaToken::rgb(0.063, 0.071, 0.078),
            RgbaToken::rgb(0.047, 0.055, 0.063),
            RgbaToken::rgb(0.125, 0.141, 0.153),
            RgbaToken::rgb(0.180, 0.200, 0.212),
        ],
    }
}

/// The fill of one surface step of one skin.
pub fn surface_step(skin: ThemeSkin, step: SurfaceStep) -> RgbaToken {
    surface_ladder(skin)[step.index()]
}

/// The semantic colour roles the product paints (EFFECTS-001).
///
/// The enum is the vocabulary a surface programs against; the resolution
/// lives in [`semantic_token`], which is the only place a role becomes a
/// colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SemanticToken {
    /// The window backdrop.
    SurfaceCanvas,
    /// The navigation rail.
    SurfaceRail,
    /// The resting card or panel.
    SurfaceCard,
    /// A control resting on a card.
    SurfaceControl,
    /// A recessed well inside a card.
    SurfaceSunken,
    /// The deepest well.
    SurfaceInset,
    /// A floating layer above the page.
    SurfaceOverlay,
    /// The lit edge of a raised surface.
    SurfaceHighlight,
    /// Primary reading ink (headings and emphasised values).
    TextEmphasis,
    /// Body reading ink.
    TextBody,
    /// Secondary reading ink (hints, captions, metadata).
    TextSecondary,
    /// The ink of an inactive control.
    TextInactive,
    /// The accent fill and accent ink.
    Accent,
    /// The ink painted on top of the accent.
    OnAccent,
    /// A healthy status.
    StatusHealthy,
    /// A degraded status.
    StatusDegraded,
    /// A failed status.
    StatusFailed,
    /// An idle status.
    StatusIdle,
    /// A running status.
    StatusActive,
}

impl SemanticToken {
    /// Every semantic role the product paints.
    pub const ALL: [Self; 19] = [
        Self::SurfaceCanvas,
        Self::SurfaceRail,
        Self::SurfaceCard,
        Self::SurfaceControl,
        Self::SurfaceSunken,
        Self::SurfaceInset,
        Self::SurfaceOverlay,
        Self::SurfaceHighlight,
        Self::TextEmphasis,
        Self::TextBody,
        Self::TextSecondary,
        Self::TextInactive,
        Self::Accent,
        Self::OnAccent,
        Self::StatusHealthy,
        Self::StatusDegraded,
        Self::StatusFailed,
        Self::StatusIdle,
        Self::StatusActive,
    ];
}

/// The colour one semantic role resolves to in one skin.
pub fn semantic_token(skin: ThemeSkin, token: SemanticToken) -> RgbaToken {
    let core = skin_core(skin);
    let interaction = skin_interaction(skin);
    match token {
        SemanticToken::SurfaceCanvas => surface_step(skin, SurfaceStep::Canvas),
        SemanticToken::SurfaceRail => surface_step(skin, SurfaceStep::Rail),
        SemanticToken::SurfaceCard => surface_step(skin, SurfaceStep::Card),
        SemanticToken::SurfaceControl => surface_step(skin, SurfaceStep::Control),
        SemanticToken::SurfaceSunken => surface_step(skin, SurfaceStep::Sunken),
        SemanticToken::SurfaceInset => surface_step(skin, SurfaceStep::Inset),
        SemanticToken::SurfaceOverlay => surface_step(skin, SurfaceStep::Overlay),
        SemanticToken::SurfaceHighlight => surface_step(skin, SurfaceStep::Highlight),
        SemanticToken::TextEmphasis => core.ink,
        SemanticToken::TextBody => core.ink,
        SemanticToken::TextSecondary => core.ink_dim,
        SemanticToken::TextInactive => interaction.disabled_ink,
        SemanticToken::Accent => core.accent,
        SemanticToken::OnAccent => core.on_accent,
        SemanticToken::StatusHealthy => core.success,
        SemanticToken::StatusDegraded => core.warning,
        SemanticToken::StatusFailed => core.danger,
        SemanticToken::StatusIdle => core.ink_dim,
        SemanticToken::StatusActive => core.accent,
    }
}

/// The lifecycle states the product colours (EFFECTS-001 #4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LifecycleState {
    /// Nothing is running; the surface is at rest.
    Idle,
    /// A task is running.
    Active,
    /// A task finished and the system is healthy.
    Healthy,
    /// A task finished but the system is degraded.
    Degraded,
    /// A task failed.
    Failed,
}

impl LifecycleState {
    /// Every lifecycle state the product colours.
    pub const ALL: [Self; 5] = [
        Self::Idle,
        Self::Active,
        Self::Healthy,
        Self::Degraded,
        Self::Failed,
    ];

    /// The semantic role this state paints.
    pub const fn token(self) -> SemanticToken {
        match self {
            Self::Idle => SemanticToken::StatusIdle,
            Self::Active => SemanticToken::StatusActive,
            Self::Healthy => SemanticToken::StatusHealthy,
            Self::Degraded => SemanticToken::StatusDegraded,
            Self::Failed => SemanticToken::StatusFailed,
        }
    }
}

/// The colour of one lifecycle state in one skin.
pub fn lifecycle_token(skin: ThemeSkin, state: LifecycleState) -> RgbaToken {
    semantic_token(skin, state.token())
}

/// The colour of one latency band in one skin (EFFECTS-001 #4).
///
/// The mapping is the one the surfaces used to repeat in their own theme
/// modules: a fast node is healthy, a medium one is degraded, a slow one
/// failed, and an unmeasured one reads as secondary ink.
pub fn latency_token(skin: ThemeSkin, band: LatencyBand) -> RgbaToken {
    match band {
        LatencyBand::Fast => semantic_token(skin, SemanticToken::StatusHealthy),
        LatencyBand::Medium => semantic_token(skin, SemanticToken::StatusDegraded),
        LatencyBand::Slow => semantic_token(skin, SemanticToken::StatusFailed),
        LatencyBand::NotObserved | LatencyBand::UnconfirmedZero => {
            semantic_token(skin, SemanticToken::TextSecondary)
        }
    }
}

/// One stop of a two-stop linear gradient (EFFECTS-001 #2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    /// The colour painted at this stop.
    pub color: RgbaToken,
    /// The position along the gradient axis: 0.0 at the start, 1.0 at the end.
    pub offset: f32,
}

/// A two-stop linear gradient (EFFECTS-001 #2).
///
/// The product's gradients are deliberately two-stop: a third stop is how a
/// gradient starts to look hand-painted instead of designed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearGradient {
    /// The first stop of the axis.
    pub start: GradientStop,
    /// The last stop of the axis.
    pub end: GradientStop,
    /// The direction of the axis in degrees: 0 points right, 90 points down.
    pub angle_degrees: f32,
}

impl LinearGradient {
    /// The number of stops a product gradient has.
    pub const STOP_COUNT: usize = 2;

    /// A two-stop gradient that runs from top to bottom.
    pub const fn new(start: RgbaToken, end: RgbaToken) -> Self {
        Self {
            start: GradientStop {
                color: start,
                offset: 0.0,
            },
            end: GradientStop {
                color: end,
                offset: 1.0,
            },
            angle_degrees: 90.0,
        }
    }

    /// The same gradient along another axis.
    pub const fn with_angle(self, angle_degrees: f32) -> Self {
        Self {
            angle_degrees,
            ..self
        }
    }

    /// The two stops of the gradient, in axis order.
    pub const fn stops(&self) -> [GradientStop; 2] {
        [self.start, self.end]
    }

    /// The colour at `position` along the axis, clamped to the two stops.
    pub fn sample(&self, position: f32) -> RgbaToken {
        let t = if position.is_finite() {
            position.clamp(0.0, 1.0)
        } else {
            0.0
        };
        RgbaToken::rgba(
            lerp(self.start.color.r, self.end.color.r, t),
            lerp(self.start.color.g, self.end.color.g, t),
            lerp(self.start.color.b, self.end.color.b, t),
            lerp(self.start.color.a, self.end.color.a, t),
        )
    }
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// The alpha of the lit edge of a raised surface (EFFECTS-001 #3).
pub const HIGHLIGHT_STROKE_ALPHA: f32 = 0.08;

/// The alpha of the hairline that separates two surfaces (EFFECTS-001 #3).
pub const DIVIDER_ALPHA: f32 = 0.10;

/// The stroke that separates a raised surface from its backdrop (EFFECTS-001 #3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HighlightStroke {
    /// The colour of the lit edge.
    pub color: RgbaToken,
    /// The width of the stroke in logical pixels.
    pub width_px: f32,
}

/// The lit edge of the active skin.
pub fn highlight_stroke(skin: ThemeSkin) -> HighlightStroke {
    let core = skin_core(skin);
    HighlightStroke {
        color: RgbaToken::rgba(core.ink.r, core.ink.g, core.ink.b, HIGHLIGHT_STROKE_ALPHA),
        width_px: metrics::HAIRLINE,
    }
}

/// The translucency roles the product paints (EFFECTS-001 #3).
///
/// A role names *why* a surface is translucent; the alpha belongs to the
/// skin, so the hover wash of one skin and the hover wash of another skin are
/// resolved by the same rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OverlayRole {
    /// The neutral hover wash over a control.
    Hover,
    /// The neutral pressed wash over a control.
    Pressed,
    /// The modal backdrop over the page.
    Scrim,
    /// The dimmed wash of an inactive control.
    Inactive,
    /// The lit edge of a raised surface.
    Highlight,
    /// The hairline between two surfaces.
    Divider,
}

impl OverlayRole {
    /// Every translucency role the product paints.
    pub const ALL: [Self; 6] = [
        Self::Hover,
        Self::Pressed,
        Self::Scrim,
        Self::Inactive,
        Self::Highlight,
        Self::Divider,
    ];
}

/// The alpha of one translucency role in one skin.
pub fn overlay_alpha(skin: ThemeSkin, role: OverlayRole) -> f32 {
    let interaction = skin_interaction(skin);
    match role {
        OverlayRole::Hover => interaction.hover.a,
        OverlayRole::Pressed => interaction.pressed.a,
        OverlayRole::Scrim => interaction.scrim.a,
        OverlayRole::Inactive => interaction.disabled_ink.a,
        OverlayRole::Highlight => HIGHLIGHT_STROKE_ALPHA,
        OverlayRole::Divider => DIVIDER_ALPHA,
    }
}

/// The text contrast ladder of one skin (EFFECTS-001 #5).
///
/// The floor of a tier is the WCAG 2.1 obligation the tier carries. The
/// inactive tier claims no floor because WCAG 1.4.3 exempts inactive
/// components; its measured ratio is still published so a surface can see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextContrastTier {
    /// Headings and primary reading text: the AAA floor (7:1).
    Emphasis,
    /// Body text: the AA floor (4.5:1).
    Body,
    /// Secondary text and large labels: the AA-large / non-text floor (3:1).
    Secondary,
    /// The ink of an inactive control; no floor is claimed.
    Inactive,
}

impl TextContrastTier {
    /// Every tier of the contrast ladder.
    pub const ALL: [Self; 4] = [Self::Emphasis, Self::Body, Self::Secondary, Self::Inactive];

    /// The contrast ratio the tier must reach on the reading surface.
    pub const fn minimum_ratio(self) -> f32 {
        match self {
            Self::Emphasis => 7.0,
            Self::Body => 4.5,
            Self::Secondary => 3.0,
            Self::Inactive => 0.0,
        }
    }

    /// The semantic role this tier paints.
    pub const fn token(self) -> SemanticToken {
        match self {
            Self::Emphasis => SemanticToken::TextEmphasis,
            Self::Body => SemanticToken::TextBody,
            Self::Secondary => SemanticToken::TextSecondary,
            Self::Inactive => SemanticToken::TextInactive,
        }
    }
}

/// The ink of one text tier in one skin.
pub fn text_token(skin: ThemeSkin, tier: TextContrastTier) -> RgbaToken {
    semantic_token(skin, tier.token())
}

/// Composite a translucent colour over an opaque backdrop.
fn composite_over(foreground: RgbaToken, background: RgbaToken) -> RgbaToken {
    let alpha = foreground.a.clamp(0.0, 1.0);
    RgbaToken::rgb(
        foreground.r * alpha + background.r * (1.0 - alpha),
        foreground.g * alpha + background.g * (1.0 - alpha),
        foreground.b * alpha + background.b * (1.0 - alpha),
    )
}

/// The WCAG relative luminance of one opaque colour.
pub fn relative_luminance(color: RgbaToken) -> f32 {
    // The weights are measured in f64: in f32 the three channels of a pure
    // white do not sum to exactly one, and a contrast ratio that is off by
    // two millionths is not a measurement of anything.
    (0.2126_f64 * channel_luminance(color.r)
        + 0.7152_f64 * channel_luminance(color.g)
        + 0.0722_f64 * channel_luminance(color.b)) as f32
}

fn channel_luminance(channel: f32) -> f64 {
    let value = f64::from(channel).clamp(0.0, 1.0);
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// The measured contrast ratio of two colours (WCAG 2.1).
///
/// A translucent foreground is composited over the background first, because
/// a ratio computed on un-composited channels would describe a colour nobody
/// ever sees. The background is treated as the opaque backdrop a surface
/// paints it on.
pub fn contrast_ratio(foreground: RgbaToken, background: RgbaToken) -> f32 {
    let base = RgbaToken::rgb(background.r, background.g, background.b);
    let painted = composite_over(foreground, base);
    let foreground_luminance = relative_luminance(painted);
    let background_luminance = relative_luminance(base);
    // The ratio is measured in f64: in f32 the division of the two offsets
    // is off by two millionths, and a contrast ratio that reads 20.999998
    // for black on white is not a measurement of anything.
    let lighter = f64::from(foreground_luminance).max(f64::from(background_luminance));
    let darker = f64::from(foreground_luminance).min(f64::from(background_luminance));
    ((lighter + 0.05) / (darker + 0.05)) as f32
}

/// The measured contrast of one text tier on the reading surface.
///
/// The reading surface is the card: it is where body text lives in every
/// page of the product.
pub fn text_contrast(skin: ThemeSkin, tier: TextContrastTier) -> f32 {
    let card = semantic_token(skin, SemanticToken::SurfaceCard);
    contrast_ratio(text_token(skin, tier), card)
}

/// Whether one text tier meets its floor on the reading surface.
pub fn meets_contrast_floor(skin: ThemeSkin, tier: TextContrastTier) -> bool {
    text_contrast(skin, tier) >= tier.minimum_ratio()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_surface_ladder_keeps_the_core_palette_and_is_a_scale() {
        for skin in ThemeSkin::ALL {
            let core = skin_core(skin);
            let ladder = surface_ladder(skin);
            assert_eq!(ladder.len(), SurfaceStep::ALL.len());
            assert_eq!(ladder[SurfaceStep::Canvas.index()], core.canvas);
            assert_eq!(ladder[SurfaceStep::Rail.index()], core.sidebar);
            assert_eq!(ladder[SurfaceStep::Card.index()], core.card);
            for pair in ladder.windows(2) {
                assert_ne!(
                    pair[0], pair[1],
                    "{skin:?}: two surface steps must not share one fill"
                );
            }
            for step in SurfaceStep::ALL {
                assert_eq!(surface_step(skin, step), ladder[step.index()]);
            }
        }
    }

    #[test]
    fn the_semantic_roles_resolve_to_the_skin_palette() {
        for skin in ThemeSkin::ALL {
            let core = skin_core(skin);
            assert_eq!(
                semantic_token(skin, SemanticToken::Accent),
                core.accent,
                "{skin:?}: the accent role is the skin accent"
            );
            assert_eq!(
                semantic_token(skin, SemanticToken::OnAccent),
                core.on_accent
            );
            assert_eq!(
                semantic_token(skin, SemanticToken::StatusHealthy),
                core.success
            );
            assert_eq!(
                semantic_token(skin, SemanticToken::StatusDegraded),
                core.warning
            );
            assert_eq!(
                semantic_token(skin, SemanticToken::StatusFailed),
                core.danger
            );
            for token in SemanticToken::ALL {
                let color = semantic_token(skin, token);
                assert!(
                    color.r.is_finite()
                        && color.g.is_finite()
                        && color.b.is_finite()
                        && color.a.is_finite(),
                    "{skin:?} {token:?} resolved a non-finite channel"
                );
            }
        }
    }

    #[test]
    fn the_latency_ramp_follows_the_status_ramp() {
        for skin in ThemeSkin::ALL {
            assert_eq!(
                latency_token(skin, LatencyBand::Fast),
                lifecycle_token(skin, LifecycleState::Healthy)
            );
            assert_eq!(
                latency_token(skin, LatencyBand::Medium),
                lifecycle_token(skin, LifecycleState::Degraded)
            );
            assert_eq!(
                latency_token(skin, LatencyBand::Slow),
                lifecycle_token(skin, LifecycleState::Failed)
            );
            assert_eq!(
                latency_token(skin, LatencyBand::NotObserved),
                lifecycle_token(skin, LifecycleState::Idle)
            );
            assert_eq!(
                latency_token(skin, LatencyBand::UnconfirmedZero),
                lifecycle_token(skin, LifecycleState::Idle)
            );
        }
    }

    #[test]
    fn a_gradient_has_exactly_two_stops() {
        let start = RgbaToken::rgb(0.0, 0.0, 0.0);
        let end = RgbaToken::rgb(1.0, 1.0, 1.0);
        let gradient = LinearGradient::new(start, end);
        assert_eq!(LinearGradient::STOP_COUNT, 2);
        assert_eq!(gradient.stops().len(), 2);
        assert_eq!(gradient.stops()[0].offset, 0.0);
        assert_eq!(gradient.stops()[1].offset, 1.0);
        assert_eq!(gradient.sample(0.0), start);
        assert_eq!(gradient.sample(1.0), end);
        assert_eq!(gradient.sample(0.5).r, 0.5);
        // Out-of-range positions clamp to the two stops instead of
        // extrapolating a colour the gradient never had.
        assert_eq!(gradient.sample(-1.0), start);
        assert_eq!(gradient.sample(2.0), end);
        assert_eq!(gradient.with_angle(0.0).angle_degrees, 0.0);
    }

    #[test]
    fn the_translucent_roles_are_ordered_and_never_opaque() {
        for skin in ThemeSkin::ALL {
            for role in OverlayRole::ALL {
                let alpha = overlay_alpha(skin, role);
                assert!(
                    alpha > 0.0 && alpha < 1.0,
                    "{skin:?} {role:?} must be a translucent wash"
                );
            }
            assert!(
                overlay_alpha(skin, OverlayRole::Pressed) > overlay_alpha(skin, OverlayRole::Hover),
                "{skin:?}: the pressed wash must read stronger than the hover wash"
            );
            let stroke = highlight_stroke(skin);
            assert_eq!(stroke.width_px, metrics::HAIRLINE);
            assert!(stroke.color.a > 0.0 && stroke.color.a < 1.0);
        }
    }

    #[test]
    fn the_text_contrast_ladder_meets_its_floor_on_every_skin() {
        for skin in ThemeSkin::ALL {
            for tier in [
                TextContrastTier::Emphasis,
                TextContrastTier::Body,
                TextContrastTier::Secondary,
            ] {
                let ratio = text_contrast(skin, tier);
                assert!(
                    ratio >= tier.minimum_ratio(),
                    "{skin:?} {tier:?} measured {ratio:.3}:1, below the {:.1}:1 floor",
                    tier.minimum_ratio()
                );
                assert!(meets_contrast_floor(skin, tier));
            }
        }
    }

    #[test]
    fn the_inactive_tier_claims_no_floor_but_publishes_its_ratio() {
        assert_eq!(TextContrastTier::Inactive.minimum_ratio(), 0.0);
        for skin in ThemeSkin::ALL {
            let ratio = text_contrast(skin, TextContrastTier::Inactive);
            assert!(ratio > 0.0, "{skin:?}: the inactive ink must be measurable");
            assert!(meets_contrast_floor(skin, TextContrastTier::Inactive));
        }
    }

    #[test]
    fn the_contrast_measurement_follows_wcag() {
        let black = RgbaToken::rgb(0.0, 0.0, 0.0);
        let white = RgbaToken::rgb(1.0, 1.0, 1.0);
        assert!((contrast_ratio(black, white) - 21.0).abs() < 1e-6);
        assert!((contrast_ratio(white, white) - 1.0).abs() < 1e-6);
        // A translucent ink is measured after compositing, not before.
        let half = RgbaToken::rgba(0.0, 0.0, 0.0, 0.5);
        let composited = contrast_ratio(half, white);
        assert!(composited > 1.0 && composited < 21.0);
    }
}
