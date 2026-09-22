use super::*;
use crate::view::waveform::mini_waveform;

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
fn test_search_input_widget() {
    let _elem_empty: Element<'_, TestMsg> =
        search_input("Search...", "", TestMsg::Search, TestMsg::Clear);
    let _elem_filled: Element<'_, TestMsg> =
        search_input("Search...", "query", TestMsg::Search, TestMsg::Clear);
}

#[test]
fn test_banner_alert_widget() {
    let _alert_accent: Element<'_, TestMsg> =
        banner_alert(BadgeKind::Accent, "Notice", "Details here", None);
    let _alert_with_action: Element<'_, TestMsg> = banner_alert(
        BadgeKind::Danger,
        "Error",
        "Something failed",
        Some(text_btn("Retry", style_ghost, Some(TestMsg::Action))),
    );
}

#[test]
fn test_kbd_badge_widget() {
    let _ctrl: Element<'_, TestMsg> = kbd_badge("Ctrl");
    let _k: Element<'_, TestMsg> = kbd_badge("K");
}

#[test]
fn test_skeleton_box_widget() {
    let _fixed: Element<'_, TestMsg> = skeleton_box(100.0, 24.0);
    let _fill: Element<'_, TestMsg> = skeleton_box(Length::Fill, 16.0);
}

#[test]
fn test_dynamic_list_editor_widget() {
    let items = vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()];
    let _elem: Element<'_, TestMsg> = dynamic_list_editor(
        &items,
        "1.0.0.1",
        "Enter IP...",
        TestMsg::Input,
        TestMsg::Add,
        TestMsg::Remove,
    );
    let empty_items: Vec<String> = vec![];
    let _elem_empty: Element<'_, TestMsg> = dynamic_list_editor(
        &empty_items,
        "",
        "Enter IP...",
        TestMsg::Input,
        TestMsg::Add,
        TestMsg::Remove,
    );
}

#[test]
fn test_mini_waveform_widget() {
    let empty: &[u64] = &[];
    let _elem_empty: Element<'_, TestMsg> = mini_waveform(empty);
    let single = [1000u64];
    let _elem_single: Element<'_, TestMsg> = mini_waveform(&single);
    let samples = [100u64, 450, 800, 300, 950, 1200];
    let _elem_multi: Element<'_, TestMsg> = mini_waveform(&samples);
    let zeros = [0u64, 0, 0];
    let _elem_zeros: Element<'_, TestMsg> = mini_waveform(&zeros);
}

/// DUAL-15-03: the HUD strip canvas consumes the shared contract bars (per
/// mille heights) in either channel ink.
#[test]
fn test_hud_waveform_strip_widget() {
    use crate::view::waveform::{StripInk, hud_waveform};

    let empty: &[u16] = &[];
    let _elem_empty: Element<'_, TestMsg> = hud_waveform(empty, StripInk::Accent);
    let bars = [0u16, 125, 500, 1_000, 250];
    let _elem_down: Element<'_, TestMsg> = hud_waveform(&bars, StripInk::Accent);
    let _elem_up: Element<'_, TestMsg> = hud_waveform(&bars, StripInk::Success);
}

/// DUAL-15-14: the shared interaction seams really read the resolution
/// palette, so a re-hardcoded hover wash or focus ring fails here (and the
/// guard scans the source for the raw literals).
#[test]
fn test_interaction_seams_consume_the_shared_tokens() {
    use iced::widget::{button, text_input};

    let dark = Theme::Dark;
    let tk = theme::tokens(&dark);

    let focused = form_input_style(&dark, text_input::Status::Focused { is_hovered: false });
    assert_eq!(focused.border.color, tk.focus_ring);
    assert_eq!(focused.border.width, 1.5);
    let idle = form_input_style(&dark, text_input::Status::Active);
    assert_eq!(idle.border.color, tk.card_border);

    let hovered = style_ghost(&dark, button::Status::Hovered);
    assert_eq!(hovered.background, Some(tk.hover.into()));
    let pressed = style_ghost(&dark, button::Status::Pressed);
    assert_eq!(pressed.background, Some(tk.pressed.into()));
    assert_ne!(hovered.background, pressed.background);
    let disabled = style_ghost(&dark, button::Status::Disabled);
    assert_eq!(disabled.text_color, tk.text_tertiary);
}
