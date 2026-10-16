use super::*;
use crate::view::component_forms::{
    form_input_style, form_pick_style, style_focus_ring, style_ghost,
};

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum TestMsg {
    Search(String),
    Clear,
    Action,
    Add,
    Remove(usize),
    Input(String),
}
#[test]
fn test_skeleton_box_widget() {
    let _fixed: Element<'_, TestMsg> = skeleton_box(100.0, 24.0);
    let _fill: Element<'_, TestMsg> = skeleton_box(Length::Fill, 16.0);
    let _shimmer: Element<'_, TestMsg> = shimmer_box(200.0, 32.0, 0.75);

    let skeleton = ShimmerSkeleton::new(8.0, 0.25);
    assert_eq!(skeleton.radius, 8.0);
    assert_eq!(skeleton.phase, 0.25);
}
/// DUAL-15-14: the shared interaction seams really read the resolution
/// palette, so a re-hardcoded hover wash or focus ring fails here (and the
/// guard scans the source for the raw literals).
#[test]
fn test_interaction_seams_consume_the_shared_tokens() {
    use iced::widget::{button, pick_list, text_input};

    let dark = Theme::Dark;
    let tk = theme::tokens(&dark);

    let focused = form_input_style(&dark, text_input::Status::Focused { is_hovered: false });
    assert_eq!(focused.border.color, tk.focus_ring);
    assert_eq!(focused.border.width, 2.0);
    let idle = form_input_style(&dark, text_input::Status::Active);
    assert_eq!(idle.border.color, tk.card_border);

    let pick_opened = form_pick_style(&dark, pick_list::Status::Opened { is_hovered: false });
    assert_eq!(pick_opened.border.color, tk.focus_ring);
    assert_eq!(pick_opened.border.width, 2.0);

    let focus_border = style_focus_ring(&dark, true);
    assert_eq!(focus_border.color, tk.focus_ring);
    assert_eq!(focus_border.width, 2.0);

    let hovered = style_ghost(&dark, button::Status::Hovered);
    assert_eq!(hovered.background, Some(tk.hover.into()));
    let pressed = style_ghost(&dark, button::Status::Pressed);
    assert_eq!(pressed.background, Some(tk.pressed.into()));
    assert_ne!(hovered.background, pressed.background);
    let disabled = style_ghost(&dark, button::Status::Disabled);
    assert_eq!(disabled.text_color, tk.text_tertiary);

    // Card hover lift state: background lifts, shadow upgrades to floating
    let card_idle = hover_lift_card_surface(&dark, false);
    assert_eq!(card_idle.shadow, tk.card_shadow);
    let card_hovered = hover_lift_card_surface(&dark, true);
    assert_eq!(card_hovered.shadow, tk.floating_shadow);
}

#[test]
fn test_card_primitives_and_dual_shadow_surfaces() {
    let dark = Theme::Dark;
    let tk = theme::tokens(&dark);

    // Verify surface definitions
    let amb = card_ambient_surface(&dark);
    assert_eq!(amb.shadow, tk.card_dual_shadow.ambient);
    assert_eq!(amb.border.width, 0.0);

    let key = card_key_surface(&dark);
    assert_eq!(key.shadow, tk.card_dual_shadow.key);
    assert_eq!(key.border.color, tk.card_border);

    let float_amb = floating_ambient_surface(&dark);
    assert_eq!(float_amb.shadow, tk.floating_dual_shadow.ambient);

    let float_key = floating_key_surface(&dark);
    assert_eq!(float_key.shadow, tk.floating_dual_shadow.key);

    // Verify constructor widgets compile and build elements
    let _card_elem: Element<'_, TestMsg> = card(Some("Title".to_string()), text("Content"));
    let _clipped_elem: Element<'_, TestMsg> = clipped_card(None, text("No title clipped"));
    let _dual_card: Element<'_, TestMsg> =
        dual_shadow_card(Some("Dual".to_string()), text("Dual shadow body"));
    let _floating_card: Element<'_, TestMsg> = floating_dual_shadow_card(text("Floating body"));
}
