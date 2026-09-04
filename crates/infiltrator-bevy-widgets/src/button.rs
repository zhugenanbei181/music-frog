//! Button: our product skin over the official unstyled `bevy_ui_widgets`
//! [`Button`]. The official widget owns behavior (focus, press semantics,
//! `Activate` observers); this module owns the token-backed visual language,
//! polymorphic variants (Primary, Default, Secondary, Ghost, Danger, Outline),
//! size ladders, and loading/disabled states.
//! Callers wire `On<Activate>` themselves so one visual control can carry
//! different typed events.

use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::Without;
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

/// Button visual variants specifying tone, hierarchy and semantic role.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Standard card-elevated fill with hairline edge.
    #[default]
    Default,
    /// Solid accent fill with high contrast text.
    Primary,
    /// Recessed accent container fill for secondary actions.
    Secondary,
    /// Transparent fill with hover backdrop overlay.
    Ghost,
    /// Danger / destructive semantic fill.
    Danger,
    /// Hairline framed button with transparent background.
    Outline,
}

/// Component wrapper for button variant.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonVariantStyle(pub ButtonVariant);

/// Button size ladder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonSize {
    /// Compact size for toolbars and dense rails.
    Sm,
    /// Standard body size for typical dialog and page actions.
    #[default]
    Md,
    /// Prominent large size for primary workflows and hero cards.
    Lg,
}

/// Component wrapper for button size.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonSizeStyle(pub ButtonSize);

/// Marker component for button loading state (showing spinner/dots indicator and suppressing input).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonLoading(pub bool);

/// Marker component for button disabled state.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonDisabled(pub bool);

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

/// Marker component on polymorphic button labels for theme and state sync.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonLabel;

/// Resolve button background fill from variant, selected, hovered, pressed, disabled and palette.
/// Pure function — 100% headless testable.
pub fn button_fill(
    variant: ButtonVariant,
    selected: bool,
    hovered: bool,
    pressed: bool,
    disabled: bool,
    palette: &UiPalette,
) -> Color {
    if disabled {
        return match variant {
            ButtonVariant::Ghost | ButtonVariant::Outline => Color::NONE,
            _ => palette.surface_elevated.with_alpha(0.4),
        };
    }

    if pressed {
        return match variant {
            ButtonVariant::Danger => palette.danger.with_alpha(0.7),
            ButtonVariant::Primary => palette.accent.with_alpha(0.8),
            ButtonVariant::Ghost => palette.pressed_bg,
            _ => palette.pressed_bg,
        };
    }

    if hovered {
        return match variant {
            ButtonVariant::Danger => palette.danger.with_alpha(0.9),
            ButtonVariant::Primary => palette.accent.with_alpha(0.9),
            ButtonVariant::Ghost => palette.hover_bg,
            _ => palette.hover_bg,
        };
    }

    if selected {
        return palette.accent;
    }

    match variant {
        ButtonVariant::Default => palette.surface_elevated,
        ButtonVariant::Primary => palette.accent,
        ButtonVariant::Secondary => palette.accent_container,
        ButtonVariant::Ghost | ButtonVariant::Outline => Color::NONE,
        ButtonVariant::Danger => palette.danger,
    }
}

/// Resolve button border color from variant, selected, hovered, pressed, disabled and palette.
/// Pure function.
pub fn button_border(
    variant: ButtonVariant,
    selected: bool,
    _hovered: bool,
    _pressed: bool,
    disabled: bool,
    palette: &UiPalette,
) -> Color {
    if disabled {
        return match variant {
            ButtonVariant::Ghost => Color::NONE,
            _ => palette.border.with_alpha(0.3),
        };
    }

    if selected {
        return palette.accent;
    }

    match variant {
        ButtonVariant::Ghost => Color::NONE,
        ButtonVariant::Danger => palette.danger,
        ButtonVariant::Primary => palette.accent,
        ButtonVariant::Default | ButtonVariant::Secondary | ButtonVariant::Outline => {
            palette.border
        }
    }
}

/// Resolve button label text color from variant, selected, disabled and palette.
/// Pure function.
pub fn button_text_color(
    variant: ButtonVariant,
    selected: bool,
    disabled: bool,
    palette: &UiPalette,
) -> Color {
    if disabled {
        return palette.ink_dim;
    }

    if selected {
        return palette.on_accent;
    }

    match variant {
        ButtonVariant::Primary | ButtonVariant::Danger => palette.on_accent,
        ButtonVariant::Secondary => palette.accent,
        ButtonVariant::Default | ButtonVariant::Ghost | ButtonVariant::Outline => palette.ink,
    }
}

/// Resolve control fill for standard pills. Pure function.
pub fn control_fill(selected: bool, hovered: bool, pressed: bool, palette: &UiPalette) -> Color {
    button_fill(
        ButtonVariant::Default,
        selected,
        hovered,
        pressed,
        false,
        palette,
    )
}

/// The pill's hairline edge: a token border. Pure function.
pub fn control_border(palette: &UiPalette) -> Color {
    palette.border
}

/// Polymorphic button scene supporting all variants, sizes, and states.
pub fn button_sized_scene(
    label: String,
    variant: ButtonVariant,
    size: ButtonSize,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let fill = button_fill(variant, false, false, false, false, palette);
    let edge = button_border(variant, false, false, false, false, palette);
    let (height_px, padding_h, role) = match size {
        ButtonSize::Sm => (palette.control_height_px * 0.8, space::S8, Role::Caption),
        ButtonSize::Md => (palette.control_height_px, space::S12, Role::Body),
        ButtonSize::Lg => (palette.control_height_px * 1.25, space::S16, Role::Heading),
    };

    bsn! {
        Node {
            height: px(height_px),
            padding: UiRect::horizontal(Val::Px(padding_h)),
            border: UiRect::all(Val::Px(metrics::HAIRLINE)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ fill })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        ButtonVariantStyle(variant)
        ButtonSizeStyle(size)
        ControlVisual(false)
        ButtonLoading(false)
        ButtonDisabled(false)
        Button
        Children [
            ( Text(label) TextRole(role) ButtonLabel ),
        ]
    }
}

/// Standard medium button scene for a given variant.
pub fn button_scene(
    label: String,
    variant: ButtonVariant,
    palette: &UiPalette,
) -> impl Scene + use<> {
    button_sized_scene(label, variant, ButtonSize::Md, palette)
}

/// Button with loading indicator state support.
pub fn loading_button_scene(
    label: String,
    loading: bool,
    variant: ButtonVariant,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let fill = button_fill(variant, false, false, false, false, palette);
    let edge = button_border(variant, false, false, false, false, palette);
    let display_text = if loading {
        "•••".to_string()
    } else {
        label
    };

    bsn! {
        Node {
            height: px(palette.control_height_px),
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border: UiRect::all(Val::Px(metrics::HAIRLINE)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ fill })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        ButtonVariantStyle(variant)
        ButtonSizeStyle(ButtonSize::Md)
        ControlVisual(false)
        ButtonLoading(loading)
        ButtonDisabled(loading)
        Button
        Children [
            ( Text(display_text) TextRole(Role::Body) ButtonLabel ),
        ]
    }
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

/// The compact pill variant for segmented controls.
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
/// changed.
#[allow(clippy::type_complexity)]
pub fn sync_control_visuals(
    palette: Res<UiPalette>,
    mut controls: Query<(
        &ControlVisual,
        Option<&PickingInteraction>,
        Option<&ButtonVariantStyle>,
        Option<&ButtonDisabled>,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    for (visual, interaction, variant_comp, disabled_comp, mut fill, mut border) in &mut controls {
        let variant = variant_comp.map(|v| v.0).unwrap_or(ButtonVariant::Default);
        let disabled = disabled_comp.map(|d| d.0).unwrap_or(false);
        let (hovered, pressed) = match interaction {
            Some(PickingInteraction::Hovered) if !disabled => (true, false),
            Some(PickingInteraction::Pressed) if !disabled => (true, true),
            _ => (false, false),
        };
        let target_fill = button_fill(variant, visual.0, hovered, pressed, disabled, &palette);
        let target_border = button_border(variant, visual.0, hovered, pressed, disabled, &palette);
        if fill.0 != target_fill {
            fill.0 = target_fill;
        }
        if border.top != target_border {
            border.set_all(target_border);
        }
    }
}

/// Restamp every pill and button label from the live palette and component state.
#[allow(clippy::type_complexity)]
pub fn sync_control_labels(
    palette: Res<UiPalette>,
    pills: Query<(
        &ControlVisual,
        Option<&ButtonVariantStyle>,
        Option<&ButtonDisabled>,
        &Children,
    )>,
    mut pill_labels: Query<(&PillLabel, &mut TextColor)>,
    mut button_labels: Query<(&ButtonLabel, &mut TextColor), Without<PillLabel>>,
) {
    for (visual, variant_comp, disabled_comp, children) in &pills {
        let variant = variant_comp.map(|v| v.0).unwrap_or(ButtonVariant::Default);
        let disabled = disabled_comp.map(|d| d.0).unwrap_or(false);
        let pill_ink = if visual.0 {
            palette.on_accent
        } else {
            palette.ink
        };
        let button_ink = button_text_color(variant, visual.0, disabled, &palette);

        for child in children.iter() {
            if let Ok((_, mut label_ink)) = pill_labels.get_mut(*child)
                && label_ink.0 != pill_ink
            {
                label_ink.0 = pill_ink;
            }
            if let Ok((_, mut label_ink)) = button_labels.get_mut(*child)
                && label_ink.0 != button_ink
            {
                label_ink.0 = button_ink;
            }
        }
    }
}
