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
