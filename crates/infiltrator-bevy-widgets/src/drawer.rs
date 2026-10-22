//! Drawer: edge-docked sliding sheet / navigation drawer over a dark scrim.
//!
//! **Pure Geometry Core**: [`DrawerPlacement`] (Left, Right, Top, Bottom) and
//! [`drawer_rect`] compute the panel's exact bounding box and slide trajectory
//! relative to the viewport. Headless-testable with zero Bevy dependency.
//!
//! **Typed Reveal**: the panel's reveal is a phase, not a pair of flags.
//! [`DrawerPhase`] is the single source of truth (closed, opening, open,
//! closing) and it carries the slide progress itself, so the panel always
//! shows exactly as much of itself as the phase says it does. The typed
//! [`DrawerRequest`] and the typed [`DrawerPhase::transition`] are the only
//! way the phase moves; [`DrawerPhase::advance`] walks it along the slide.
//!
//! **Scene Adapter**: [`drawer_scene`] builds a declarative full-viewport scrim
//! hosting the edge-docked panel at token borders and surface fills, and
//! [`animate_drawers`] walks the phase and places the panel by it.

use crate::palette::{UiPalette, rem};
use crate::popover::Rect;
use crate::theme::space;
use bevy::color::Alpha;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::time::Time;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, Node, PositionType, UiRect, Val,
    percent,
};

/// Docking edge for the drawer panel.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawerPlacement {
    /// Left edge (e.g. mobile navigation drawer).
    #[default]
    Left,
    /// Right edge (e.g. inspector/filter sidebar).
    Right,
    /// Top edge (e.g. banner notification sheet).
    Top,
    /// Bottom edge (e.g. bottom sheet / action tray).
    Bottom,
}

/// The drawer's reveal phase — the panel's own typed stage.
///
/// The phase carries the slide progress, so the phase and the geometry are
/// one fact: a panel that is `Opening { progress: 0.4 }` shows exactly 40% of
/// itself, and no second copy of the progress can disagree with it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DrawerPhase {
    /// The panel is docked away: it shows nothing.
    #[default]
    Closed,
    /// The panel is sliding in; `progress` is the revealed fraction.
    Opening { progress: f32 },
    /// The panel is fully revealed.
    Open,
    /// The panel is sliding back out; `progress` is the revealed fraction.
    Closing { progress: f32 },
}

/// The typed request that moves the drawer's phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawerRequest {
    /// The drawer should be visible: the panel slides in.
    Reveal,
    /// The drawer should be hidden: the panel slides back out.
    Hide,
}

/// The drawer's slide duration (seconds).
pub const DRAWER_SLIDE_SECS: f32 = 0.2;

/// The docked edge's offset: the panel's own edge sits on the viewport's
/// edge, so the offset is zero in every placement.
const DOCKED_EDGE_OFFSET: f32 = 0.0;

impl DrawerPhase {
    /// The typed transition: the only way the phase moves.
    pub fn transition(self, request: DrawerRequest) -> Self {
        match (self, request) {
            (Self::Closed | Self::Closing { .. }, DrawerRequest::Reveal) => Self::Opening {
                progress: self.progress(),
            },
            (Self::Open | Self::Opening { .. }, DrawerRequest::Hide) => Self::Closing {
                progress: self.progress(),
            },
            (Self::Open, DrawerRequest::Reveal) => Self::Open,
            (Self::Closed, DrawerRequest::Hide) => Self::Closed,
            (Self::Opening { .. }, DrawerRequest::Reveal) => self,
            (Self::Closing { .. }, DrawerRequest::Hide) => self,
        }
    }

    /// The revealed fraction: 0 is docked away, 1 is fully revealed.
    pub fn progress(self) -> f32 {
        match self {
            Self::Closed => 0.0,
            Self::Opening { progress } | Self::Closing { progress } => progress.clamp(0.0, 1.0),
            Self::Open => 1.0,
        }
    }

    /// Whether the panel is (or is becoming) visible.
    pub fn is_visible(self) -> bool {
        matches!(self, Self::Opening { .. } | Self::Open)
    }

    /// Walk the phase along the slide by `delta_secs` and settle it at the
    /// end of the transition.
    pub fn advance(self, delta_secs: f32) -> Self {
        let step = (delta_secs / DRAWER_SLIDE_SECS).clamp(0.0, 1.0);
        match self {
            Self::Opening { progress } => {
                let next = (progress + step).min(1.0);
                if next >= 1.0 {
                    Self::Open
                } else {
                    Self::Opening { progress: next }
                }
            }
            Self::Closing { progress } => {
                let next = (progress - step).max(0.0);
                if next <= 0.0 {
                    Self::Closed
                } else {
                    Self::Closing { progress: next }
                }
            }
            settled => settled,
        }
    }
}

/// Pure state of a drawer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawerState {
    pub placement: DrawerPlacement,
    pub size_px: f32,
    pub phase: DrawerPhase,
}

impl DrawerState {
    pub fn new(placement: DrawerPlacement, size_px: f32) -> Self {
        Self {
            placement,
            size_px,
            phase: DrawerPhase::Open,
        }
    }

    /// Move the drawer through its typed transition.
    pub fn transition(&mut self, request: DrawerRequest) {
        self.phase = self.phase.transition(request);
    }
}

#[derive(QueryFilter)]
pub struct DrawerScrimFilter {
    with_drawer_scrim: With<DrawerScrim>,
    without_drawer_panel: Without<DrawerPanel>,
}

#[derive(QueryFilter)]
pub struct DrawerPanelFilter {
    with_drawer_panel: With<DrawerPanel>,
    without_drawer_scrim: Without<DrawerScrim>,
}

/// Compute absolute bounding rectangle for a drawer panel during slide animation.
/// `open_ratio` is clamped to `0.0..=1.0`. Pure function.
pub fn drawer_rect(
    placement: DrawerPlacement,
    size_px: f32,
    viewport: Rect,
    open_ratio: f32,
) -> Rect {
    let ratio = open_ratio.clamp(0.0, 1.0);
    match placement {
        DrawerPlacement::Left => {
            let w = size_px.min(viewport.w);
            let x = viewport.x - w * (1.0 - ratio);
            Rect {
                x,
                y: viewport.y,
                w,
                h: viewport.h,
            }
        }
        DrawerPlacement::Right => {
            let w = size_px.min(viewport.w);
            let x = viewport.right() - w * ratio;
            Rect {
                x,
                y: viewport.y,
                w,
                h: viewport.h,
            }
        }
        DrawerPlacement::Top => {
            let h = size_px.min(viewport.h);
            let y = viewport.y - h * (1.0 - ratio);
            Rect {
                x: viewport.x,
                y,
                w: viewport.w,
                h,
            }
        }
        DrawerPlacement::Bottom => {
            let h = size_px.min(viewport.h);
            let y = viewport.bottom() - h * ratio;
            Rect {
                x: viewport.x,
                y,
                w: viewport.w,
                h,
            }
        }
    }
}

/// Marker on drawer full-screen scrim.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawerScrim;

/// Marker on drawer docked panel, with the geometry its slide needs.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct DrawerPanel {
    /// The edge the panel is docked to.
    pub placement: DrawerPlacement,
    /// The panel's size at the baseline root font size, in px.
    pub size_px: f32,
}

/// The drawer's live phase, mounted on the drawer root.
///
/// The phase is the panel's only state; the scrim's wash and the panel's
/// offset are read from it, so the drawer can never show a panel that
/// disagrees with its own phase.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct DrawerPhaseState(pub DrawerPhase);

/// Marker on drawer close button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawerCloseButton;

/// Message to open a drawer.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawerOpenEvent(pub Entity);

/// Message to close a drawer.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawerCloseEvent(pub Entity);

/// Declarative drawer scene constructor.
///
/// The panel is mounted at the phase's own progress, so a drawer that starts
/// closed is already out of the way and a drawer that starts open is already
/// in place — the slide is the phase's, never a separate offset.
///
/// This entry mounts the drawer on screen; a host that wants the drawer to
/// fade in mounts it closed with [`drawer_scene_with_phase`] and sends the
/// drawer's own open request.
pub fn drawer_scene(
    placement: DrawerPlacement,
    size_px: f32,
    content: Box<dyn Scene>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    drawer_scene_with_phase(placement, size_px, DrawerPhase::Open, content, palette)
}

/// The declarative drawer scene at an explicit phase.
///
/// The phase is the drawer's own state, so the scene mounts the panel exactly
/// where that phase says it is: a closed drawer is already out of the way.
pub fn drawer_scene_with_phase(
    placement: DrawerPlacement,
    size_px: f32,
    phase: DrawerPhase,
    content: Box<dyn Scene>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let progress = phase.progress();
    // The panel's travel is the panel's own size, on the rem channel, so the
    // slide scales with the root font size like every other size.
    let travel = -size_px * (1.0 - progress);

    let (width_val, height_val, left_val, top_val, right_val, bottom_val, radius_val) =
        match placement {
            DrawerPlacement::Left => (
                rem(size_px),
                percent(100),
                rem(travel),
                rem(DOCKED_EDGE_OFFSET),
                Val::Auto,
                Val::Auto,
                BorderRadius {
                    top_left: Val::Px(0.0).into(),
                    top_right: rem(palette.card_radius_px).into(),
                    bottom_left: Val::Px(0.0).into(),
                    bottom_right: rem(palette.card_radius_px).into(),
                },
            ),
            DrawerPlacement::Right => (
                rem(size_px),
                percent(100),
                Val::Auto,
                rem(DOCKED_EDGE_OFFSET),
                rem(travel),
                Val::Auto,
                BorderRadius {
                    top_left: rem(palette.card_radius_px).into(),
                    top_right: Val::Px(0.0).into(),
                    bottom_left: rem(palette.card_radius_px).into(),
                    bottom_right: Val::Px(0.0).into(),
                },
            ),
            DrawerPlacement::Top => (
                percent(100),
                rem(size_px),
                rem(DOCKED_EDGE_OFFSET),
                rem(travel),
                Val::Auto,
                Val::Auto,
                BorderRadius {
                    top_left: rem(palette.card_radius_px).into(),
                    top_right: rem(palette.card_radius_px).into(),
                    bottom_left: Val::Px(0.0).into(),
                    bottom_right: Val::Px(0.0).into(),
                },
            ),
            DrawerPlacement::Bottom => (
                percent(100),
                rem(size_px),
                rem(DOCKED_EDGE_OFFSET),
                Val::Auto,
                Val::Auto,
                rem(travel),
                BorderRadius {
                    top_left: Val::Px(0.0).into(),
                    top_right: Val::Px(0.0).into(),
                    bottom_left: rem(palette.card_radius_px).into(),
                    bottom_right: rem(palette.card_radius_px).into(),
                },
            ),
        };

    bsn! {
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::FlexStart,
            }
            BackgroundColor({ palette.scrim })
            DrawerScrim
            DrawerPhaseState({ phase })
            Children [
                Node {
                    position_type: PositionType::Absolute,
                    left: left_val,
                    top: top_val,
                    right: right_val,
                    bottom: bottom_val,
                    width: width_val,
                    height: height_val,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(rem(space::S16)),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: radius_val,
                }
                BackgroundColor({ palette.surface })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                DrawerPanel { placement: placement, size_px: size_px }
                Children [
                    @{ content }
                ]
            ]
    }
}

/// Walk every mounted drawer's phase and place its panel by the phase.
///
/// The phase is the drawer's only state: the panel's edge offset and the
/// scrim's wash are both read from it, so the drawer always shows exactly as
/// much of itself as the phase says it does. The drawer's own open and close
/// requests are the only thing that moves the phase.
pub fn animate_drawers(
    time: Res<Time>,
    palette: Res<UiPalette>,
    mut opens: MessageReader<DrawerOpenEvent>,
    mut closes: MessageReader<DrawerCloseEvent>,
    mut drawers: Query<(&mut DrawerPhaseState, &Children, &mut BackgroundColor), With<DrawerScrim>>,
    mut panels: Query<(&mut Node, &DrawerPanel), Without<DrawerScrim>>,
) {
    for event in opens.read() {
        if let Ok(mut drawer) = drawers.get_mut(event.0) {
            // The query item is the whole tuple: its first element is the
            // phase component's own `Mut`, and the phase is that component's
            // field.
            let phase: DrawerPhase = drawer.0.0;
            drawer.0.0 = phase.transition(DrawerRequest::Reveal);
        }
    }
    for event in closes.read() {
        if let Ok(mut drawer) = drawers.get_mut(event.0) {
            let phase: DrawerPhase = drawer.0.0;
            drawer.0.0 = phase.transition(DrawerRequest::Hide);
        }
    }
    let step = time.delta_secs();
    for (mut state, children, mut scrim) in &mut drawers {
        let phase = state.0.advance(step);
        state.0 = phase;
        let progress = phase.progress();
        // The wash is the palette's own scrim, scaled by the phase: the
        // backdrop dims exactly as much as the panel has slid in.
        scrim.0 = palette.scrim.with_alpha(palette.scrim.alpha() * progress);
        for child in children.iter() {
            let Ok((mut node, panel)) = panels.get_mut(*child) else {
                continue;
            };
            let travel = -panel.size_px * (1.0 - progress);
            match panel.placement {
                DrawerPlacement::Left => node.left = rem(travel),
                DrawerPlacement::Right => node.right = rem(travel),
                DrawerPlacement::Top => node.top = rem(travel),
                DrawerPlacement::Bottom => node.bottom = rem(travel),
            }
        }
    }
}

/// Repaint drawer panels and scrims from live palette.
pub fn sync_drawer_visuals(
    palette: Res<UiPalette>,
    mut panels: Query<(&mut BackgroundColor, &mut BorderColor), DrawerPanelFilter>,
) {
    let edge = palette.border;
    for (mut fill, mut border) in &mut panels {
        if fill.0 != palette.surface {
            fill.0 = palette.surface;
        }
        if border.top != edge {
            border.set_all(edge);
        }
    }
}
