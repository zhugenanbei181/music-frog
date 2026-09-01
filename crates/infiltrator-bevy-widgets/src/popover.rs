//! Popover: anchored floating panel over a light scrim, pure placement core.
//!
//! **Pure geometry core**: [`placement`] (and its [`Rect`] / [`Side`]
//! vocabulary) is the whole anchor contract — prefer the requested side of
//! the anchor, flip to the other side when the viewport runs out of room,
//! keep the panel inside the viewport on both axes. Total and unit-tested
//! across the four quadrants; a panel bigger than the viewport clamps instead
//! of panicking. Hosts own the anchor rect, the viewport rect and the mount
//! decision; this layer never measures live layout.
//!
//! **Scene adapter**: [`popover_scene`] stamps the placement once at spawn
//! (absolute pixel geometry inside a full-viewport scrim the host mounts at
//! its root) and hosts the caller's content scene. Open / close is host
//! mount / unmount — a popover with nothing to say must not stay mounted,
//! so this module has no open state to get stuck. [`sync_popover_visuals`]
//! re-projects scrim and panel chrome compare-and-set, so a theme switch
//! repaints in place.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, Node, PositionType, UiRect, Val,
    percent, px,
};

use crate::palette::UiPalette;
use crate::theme::space;

/// Gap (px) between the anchor edge and the panel — one spacing rung.
pub const ANCHOR_GAP_PX: f32 = space::S8;

/// A pixel rectangle in viewport space, y growing downward (bevy UI
/// convention): `(x, y)` is the top-left corner. Zero bevy — the placement
/// core's test surface.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// Right edge (exclusive).
    pub fn right(self) -> f32 {
        self.x + self.w
    }

    /// Bottom edge (exclusive).
    pub fn bottom(self) -> f32 {
        self.y + self.h
    }
}

/// Which side of the anchor the panel prefers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    /// Below the anchor's bottom edge (the default: menus and cards open
    /// downward).
    #[default]
    Below,
    /// Above the anchor's top edge.
    Above,
}

/// Everything [`placement`] needs, in one copyable bundle: the anchor's
/// rect, the viewport it must stay inside, the preferred side and the
/// panel's pixel size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchorHint {
    /// The anchor's rect (e.g. the triggering button's bounds).
    pub anchor: Rect,
    /// The viewport the panel must stay inside (the host window's UI area).
    pub viewport: Rect,
    /// Preferred side; flipped when the viewport runs out of room.
    pub side: Side,
    /// The panel's `(width, height)` in px.
    pub panel: (f32, f32),
}

/// Place the panel for one [`AnchorHint`]. Pure, total, flip-honest:
///
/// - the preferred side wins whenever the whole panel fits there;
/// - otherwise the other side wins when it fits (the flip);
/// - when neither side fits, the preferred side is kept but clamped into
///   the viewport (never a panic, never off-viewport geometry);
/// - horizontally the panel's left edge seeks the anchor's left edge and
///   clamps into the viewport;
/// - a panel dimension larger than the viewport pins to the viewport edge.
pub fn placement(hint: AnchorHint, gap: f32) -> Rect {
    let AnchorHint {
        anchor,
        viewport,
        side,
        panel: (panel_w, panel_h),
    } = hint;
    let below_top = anchor.bottom() + gap;
    let above_top = anchor.y - gap - panel_h;
    let below_fits = below_top + panel_h <= viewport.bottom();
    let above_fits = above_top >= viewport.y;
    let (preferred, flipped) = match side {
        Side::Below => (below_top, above_top),
        Side::Above => (above_top, below_top),
    };
    let top = match (side, below_fits, above_fits) {
        (Side::Below, true, _) | (Side::Above, _, true) => preferred,
        _ if flipped_fits(side, below_fits, above_fits) => flipped,
        // Neither side holds the panel: keep the preferred side, clamped.
        _ => preferred,
    };
    let x = anchor
        .x
        .clamp(viewport.x, (viewport.right() - panel_w).max(viewport.x));
    Rect {
        x,
        y: top.clamp(viewport.y, (viewport.bottom() - panel_h).max(viewport.y)),
        w: panel_w,
        h: panel_h,
    }
}

/// Whether the flipped side holds the whole panel.
fn flipped_fits(side: Side, below_fits: bool, above_fits: bool) -> bool {
    match side {
        Side::Below => above_fits,
        Side::Above => below_fits,
    }
}

/// Marker on the full-viewport scrim behind the panel.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PopoverScrim;

/// Marker on the floating panel.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PopoverPanel;

/// The popover overlay: a scrim over the viewport the host mounts it under,
/// plus the panel at the stamped placement hosting the caller's content.
/// The geometry is frozen at spawn — a popover that must follow a moving
/// anchor is remounted by the host (mount / unmount is host law here).
pub fn popover_scene(
    hint: AnchorHint,
    content: Box<dyn Scene>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let rect = placement(hint, ANCHOR_GAP_PX);
    let edge = palette.border;
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::FlexStart,
        }
        BackgroundColor({ palette.scrim() })
        PopoverScrim
        Children [
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(rect.x),
                    top: px(rect.y),
                    width: px(rect.w),
                    height: px(rect.h),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(space::S12)),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
                }
                BackgroundColor({ palette.surface })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                PopoverPanel
                Children [
                    ( { content } ),
                ]
            ),
        ]
    }
}

/// Repaint every popover from the live palette: scrim, panel fill and
/// hairline edge — compare-and-set, unchanged frames cost nothing. The two
/// fill queries are disjoint by marker.
#[allow(clippy::type_complexity)]
pub fn sync_popover_visuals(
    palette: Res<UiPalette>,
    mut scrims: Query<&mut BackgroundColor, (With<PopoverScrim>, Without<PopoverPanel>)>,
    mut panels: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (With<PopoverPanel>, Without<PopoverScrim>),
    >,
) {
    let scrim = palette.scrim();
    for mut fill in &mut scrims {
        if fill.0 != scrim {
            fill.0 = scrim;
        }
    }
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
