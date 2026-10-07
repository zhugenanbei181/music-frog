//! Button: our product skin over the official unstyled `bevy_ui_widgets`
//! [`Button`]. The official widget owns behavior (focus, press semantics,
//! `Activate` observers); this module owns the token-backed visual language,
//! polymorphic variants (Primary, Default, Secondary, Ghost, Danger, Outline),
//! size ladders, and loading/disabled states.
//! Callers wire `On<Activate>` themselves so one visual control can carry
//! different typed events.

use crate::interaction_block::InteractionBlocked;
use crate::palette::UiPalette;
use crate::responsive::TouchHitbox;
use crate::text::{Role, TextRole};
use crate::theme::{metrics, space};
use bevy::a11y::AccessibilityNode;
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::Insert;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, QueryData, Without};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::picking::hover::PickingInteraction;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, JustifyContent, Node, UiRect, Val, px,
};
use bevy::ui::widget::Text;
use bevy::ui::{InteractionDisabled, Pressed};
use bevy::ui_widgets::Button;

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

pub(crate) fn insert_button_disabled(
    insert: On<Insert<ButtonDisabled>>,
    buttons: Query<&ButtonDisabled>,
    mut commands: Commands,
) {
    let Ok(disabled) = buttons.get(insert.entity) else {
        return;
    };
    if disabled.0 {
        commands
            .entity(insert.entity)
            .insert(InteractionDisabled)
            .remove::<Pressed>();
    } else {
        commands
            .entity(insert.entity)
            .remove::<InteractionDisabled>();
    }
}

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
        return palette.disabled_ink;
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
    let role = match size {
        ButtonSize::Sm => Role::Caption,
        ButtonSize::Md => Role::Body,
        ButtonSize::Lg => Role::Heading,
    };
    button_with_label_scene(
        bsn! { Text(label) TextRole(role) ButtonLabel },
        variant,
        size,
        palette,
    )
}

/// A native label scene keeps translation metadata attached to the original widget.
pub fn button_with_label_scene(
    label: impl Scene + 'static,
    variant: ButtonVariant,
    size: ButtonSize,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let fill = button_fill(variant, false, false, false, false, palette);
    let edge = button_border(variant, false, false, false, false, palette);
    let (height_px, padding_h) = match size {
        ButtonSize::Sm => (palette.control_height_px * 0.8, space::S8),
        ButtonSize::Md => (palette.control_height_px, space::S12),
        ButtonSize::Lg => (palette.control_height_px * 1.25, space::S16),
    };

    Box::new(bsn! {
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
            TouchHitbox::default()
            Children [
                @{ label }
            ]
    })
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
            TouchHitbox::default()
            Children [
                Text(display_text) TextRole(Role::Body) ButtonLabel
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
            TouchHitbox::default()
            Children [
                Text(label) TextRole(Role::Body) PillLabel
            ]
    }
}

/// The compact pill variant for segmented controls.
pub fn pill_caption_scene(
    label: String,
    selected: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    pill_caption_with_label_scene(
        bsn! { Text(label) TextRole(Role::Caption) PillLabel },
        selected,
        palette,
    )
}

/// A compact native pill keeps localized label metadata on its real text entity.
pub fn pill_caption_with_label_scene(
    label: impl Scene + 'static,
    selected: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let edge = control_border(palette);
    Box::new(bsn! {
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
            TouchHitbox::default()
            Children [
                @label
            ]
    })
}

/// Paint reads the same temporary and domain suppression as native input.
#[derive(QueryData)]
#[query_data(mutable)]
pub struct ControlPaint {
    visual: &'static ControlVisual,
    interaction: Option<&'static PickingInteraction>,
    variant: Option<&'static ButtonVariantStyle>,
    disabled: Option<&'static ButtonDisabled>,
    blocked: Has<InteractionBlocked>,
    fill: &'static mut BackgroundColor,
    border: &'static mut BorderColor,
}
pub fn sync_control_visuals(palette: Res<UiPalette>, mut controls: Query<ControlPaint>) {
    for mut control in &mut controls {
        let variant = control.variant.map_or(ButtonVariant::Default, |v| v.0);
        let disabled = control.blocked || control.disabled.is_some_and(|d| d.0);
        let (hovered, pressed) = match control.interaction {
            Some(PickingInteraction::Hovered) if !disabled => (true, false),
            Some(PickingInteraction::Pressed) if !disabled => (true, true),
            _ => (false, false),
        };
        let fill = button_fill(
            variant,
            control.visual.0,
            hovered,
            pressed,
            disabled,
            &palette,
        );
        let border = button_border(
            variant,
            control.visual.0,
            hovered,
            pressed,
            disabled,
            &palette,
        );
        if control.fill.0 != fill {
            control.fill.0 = fill;
        }
        if control.border.top != border {
            control.border.set_all(border);
        }
    }
}

#[derive(QueryData)]
pub struct ControlLabelState {
    visual: &'static ControlVisual,
    variant: Option<&'static ButtonVariantStyle>,
    disabled: Option<&'static ButtonDisabled>,
    blocked: Has<InteractionBlocked>,
    children: &'static Children,
}
/// Restamp labels from the live palette and effective interaction state.
pub fn sync_control_labels(
    palette: Res<UiPalette>,
    controls: Query<ControlLabelState>,
    mut pill_labels: Query<(&PillLabel, &mut TextColor)>,
    mut button_labels: Query<(&ButtonLabel, &mut TextColor), Without<PillLabel>>,
) {
    for control in &controls {
        let variant = control.variant.map_or(ButtonVariant::Default, |v| v.0);
        let disabled = control.blocked || control.disabled.is_some_and(|d| d.0);
        let pill_ink = if disabled {
            palette.disabled_ink
        } else if control.visual.0 {
            palette.on_accent
        } else {
            palette.ink
        };
        let button_ink = button_text_color(variant, control.visual.0, disabled, &palette);
        for child in control.children.iter() {
            if let Ok((_, mut ink)) = pill_labels.get_mut(*child)
                && ink.0 != pill_ink
            {
                ink.0 = pill_ink;
            }
            if let Ok((_, mut ink)) = button_labels.get_mut(*child)
                && ink.0 != button_ink
            {
                ink.0 = button_ink;
            }
        }
    }
}

/// The SDK and accessibility tree must observe the same disabled fact as the skin.
#[derive(QueryData)]
#[query_data(mutable)]
pub struct ButtonInteractionState {
    entity: Entity,
    disabled: &'static ButtonDisabled,
    blocked: Has<InteractionBlocked>,
    sdk_disabled: Has<InteractionDisabled>,
    accessibility: Option<&'static mut AccessibilityNode>,
}
pub fn sync_button_disabled(mut commands: Commands, mut buttons: Query<ButtonInteractionState>) {
    for button in &mut buttons {
        let disabled = button.disabled.0 || button.blocked;
        if disabled != button.sdk_disabled {
            if disabled {
                commands
                    .entity(button.entity)
                    .insert(InteractionDisabled)
                    .remove::<Pressed>();
            } else {
                commands
                    .entity(button.entity)
                    .remove::<InteractionDisabled>();
            }
        }
        if let Some(mut accessibility) = button.accessibility
            && accessibility.is_disabled() != disabled
        {
            if disabled {
                accessibility.set_disabled();
            } else {
                accessibility.clear_disabled();
            }
        }
    }
}
