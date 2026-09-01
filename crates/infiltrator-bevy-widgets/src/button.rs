//! Pill button: our product skin over the official unstyled `bevy_ui_widgets`
//! [`Button`]. The official widget owns behavior (focus, press semantics,
//! `Activate` observers); this module owns the token-backed visual language.
//! Callers wire `On<Activate>` themselves so one visual control can carry
//! different typed events.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Changed, Or};
use bevy::ecs::system::{Query, Res};
use bevy::picking::hover::PickingInteraction;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, JustifyContent, Node, UiRect, Val, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::{metrics, space};

/// Stable visual state carried by every product-owned button: the page-owned
/// selected bit. Interaction itself remains bevy's `PickingInteraction`;
/// this component only feeds the shared repaint system.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlVisual(pub bool);

/// Marker on a pill's own label: the label ink follows the pill's selected
/// bit — `on_accent` while selected, ordinary ink otherwise. Pure routing
/// for [`sync_control_labels`]; role stamping still owns size and face.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PillLabel;

/// Resolve one control fill from the palette and interaction state.
/// Hover and pressed are transient; selected is the page-owned state.
/// Pure function — headless-testable without any picking runtime.
pub fn control_fill(selected: bool, hovered: bool, pressed: bool, palette: &UiPalette) -> Color {
    if pressed {
        return palette.pressed_bg;
    }
    if hovered {
        return palette.hover_bg;
    }
    if selected {
        return palette.accent;
    }
    palette.surface_elevated
}

/// The pill's hairline edge: a token border so the pill reads as a
/// control on surfaces close to its idle fill (light canvas). Pure
/// function.
pub fn control_border(palette: &UiPalette) -> Color {
    palette.border
}

/// Unstyled bevy button plus the product pill skin. Interaction wiring
/// belongs to the caller.
pub fn pill_scene(label: String, selected: bool, palette: &UiPalette) -> impl Scene + use<> {
    let edge = control_border(palette);
    bsn! {
        Node {
            min_width: px(palette.control_height_px * 2.8),
            height: px(palette.control_height_px),
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border: UiRect::all(Val::Px(metrics::HAIRLINE)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ control_fill(selected, false, false, palette) })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        ControlVisual(selected)
        Button
        Children [
            ( Text(label) TextRole(Role::Body) PillLabel ),
        ]
    }
}

/// The compact pill variant for segmented controls: caption-size label in
/// a control-height shell. Exists because four CJK glyphs at body size
/// cannot fit three of them into a 240px rail (measured in bevy capture
/// round 1 — the body-size pills overflowed the sidebar into the content
/// column). Same visual state machinery as [`pill_scene`]: `ControlVisual`,
/// the official `Button`, and the [`PillLabel`] ink contract.
pub fn pill_caption_scene(
    label: String,
    selected: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = control_border(palette);
    bsn! {
        Node {
            height: px(palette.control_height_px * 0.8),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            border: UiRect::all(Val::Px(metrics::HAIRLINE)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ control_fill(selected, false, false, palette) })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        ControlVisual(selected)
        Button
        Children [
            ( Text(label) TextRole(Role::Caption) PillLabel ),
        ]
    }
}

/// Repaint product-owned controls whose interaction or selected state
/// changed. bevy's picking runtime drives the transient states; this system
/// supplies the shared token skin.
#[allow(clippy::type_complexity)]
pub fn sync_control_visuals(
    palette: Res<UiPalette>,
    mut controls: Query<
        (
            &ControlVisual,
            Option<&PickingInteraction>,
            &mut BackgroundColor,
        ),
        Or<(Changed<ControlVisual>, Changed<PickingInteraction>)>,
    >,
) {
    for (visual, interaction, mut fill) in &mut controls {
        let (hovered, pressed) = match interaction {
            Some(PickingInteraction::Hovered) => (true, false),
            Some(PickingInteraction::Pressed) => (true, true),
            _ => (false, false),
        };
        fill.0 = control_fill(visual.0, hovered, pressed, &palette);
    }
}

/// Restamp every pill's chrome — the label ink (`on_accent` while
/// selected, ordinary ink otherwise) and the hairline edge — from the live
/// palette. Compare-and-set, so unchanged frames cost nothing and a theme
/// switch repaints pills without any switch-specific hook.
#[allow(clippy::type_complexity)]
pub fn sync_control_labels(
    palette: Res<UiPalette>,
    mut pills: Query<(&ControlVisual, &mut BorderColor, &Children)>,
    mut labels: Query<(&PillLabel, &mut TextColor)>,
) {
    for (visual, mut border, children) in &mut pills {
        let ink = if visual.0 {
            palette.on_accent
        } else {
            palette.ink
        };
        let edge = control_border(&palette);
        if border.top != edge {
            border.set_all(edge);
        }
        for child in children.iter() {
            if let Ok((_, mut label_ink)) = labels.get_mut(*child)
                && label_ink.0 != ink
            {
                label_ink.0 = ink;
            }
        }
    }
}
