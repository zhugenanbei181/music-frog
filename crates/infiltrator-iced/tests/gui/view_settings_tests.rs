use super::*;

#[test]
fn test_settings_choice_display() {
    assert_eq!(format!("{}", SettingsChoice { value: "zh-CN" }), "zh-CN");
    assert_eq!(format!("{}", SettingsChoice { value: "stable" }), "stable");
}

#[test]
fn test_secondary_text_widget() {
    let _elem: Element<'_, Message> = secondary_text("Helper description");
}

#[test]
fn test_theme_segmented_control_options() {
    let options_en = vec!["Light".to_string(), "Dark".to_string(), "Forest".to_string(), "AMOLED".to_string()];
    let _ctrl_light = segmented_control(&options_en, 0, |_| Message::ToggleTheme);
    let _ctrl_dark = segmented_control(&options_en, 1, |_| Message::ToggleTheme);
    let _ctrl_forest = segmented_control(&options_en, 2, |_| Message::ToggleTheme);
    let _ctrl_amoled = segmented_control(&options_en, 3, |_| Message::ToggleTheme);
}

#[test]
fn test_shell_export_row() {
    let _row: Element<'_, Message> = shell_export_row("Bash", "export http_proxy=...", &Lang("en-US"));
    let _card: Element<'_, Message> = shell_export_card(&Lang("zh-CN"));
}

#[test]
fn test_inbounds_card() {
    let (state, _) = AppState::new();
    let _card: Element<'_, Message> = inbounds_card(&state, &Lang("zh-CN"));
}

#[test]
fn test_system_proxy_card() {
    let (state, _) = AppState::new();
    let lang = Lang(&state.shell.lang);
    let _card_zh: Element<'_, Message> = system_proxy_card(&state, &lang, false);
    let _card_en: Element<'_, Message> = system_proxy_card(&state, &lang, true);
}

#[test]
fn test_tun_card() {
    let (state1, _) = AppState::new();
    let lang1 = Lang(&state1.shell.lang);
    let _card_default: Element<'_, Message> = tun_card(&state1, &lang1, false);

    let (mut state2, _) = AppState::new();
    state2.editor.tun_stack = "mixed".to_string();
    state2.editor.tun_form.dns_hijack = "any:53".to_string();
    let lang2 = Lang(&state2.shell.lang);
    let _card_mixed: Element<'_, Message> = tun_card(&state2, &lang2, true);

    let (mut state3, _) = AppState::new();
    state3.editor.tun_stack = "system".to_string();
    let lang3 = Lang(&state3.shell.lang);
    let _card_system: Element<'_, Message> = tun_card(&state3, &lang3, false);
}

#[test]
fn test_settings_view_render() {
    let (state1, _) = AppState::new();
    let _view_zh: Element<'_, Message> = view(&state1);

    let (mut state2, _) = AppState::new();
    state2.shell.lang = "en-US".to_string();
    let _view_en: Element<'_, Message> = view(&state2);
}
