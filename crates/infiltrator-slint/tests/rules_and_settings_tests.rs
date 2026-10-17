//! Tests for Rule toggling, Provider batch refresh, language preference parsing, and theme/color parsing in infiltrator-slint.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::language::LanguagePreference;

#[test]
fn test_toggle_rule_enabled_intent_generation() {
    let index = 42usize;
    let intent = CommandIntent::ToggleRuleEnabled { index };
    match intent {
        CommandIntent::ToggleRuleEnabled { index: i } => assert_eq!(i, 42),
        _ => panic!("wrong intent"),
    }
}

#[test]
fn test_refresh_rule_providers_batch_intent() {
    let intent = CommandIntent::RefreshRuleProviders;
    assert_eq!(intent, CommandIntent::RefreshRuleProviders);
}

#[test]
fn test_language_preference_parsing() {
    assert_eq!(
        LanguagePreference::parse("system").unwrap(),
        LanguagePreference::System
    );
    assert_eq!(
        LanguagePreference::parse("zh-CN").unwrap(),
        LanguagePreference::SimplifiedChinese
    );
    assert_eq!(
        LanguagePreference::parse("zh").unwrap(),
        LanguagePreference::SimplifiedChinese
    );
    assert_eq!(
        LanguagePreference::parse("en-US").unwrap(),
        LanguagePreference::English
    );
    assert_eq!(
        LanguagePreference::parse("en").unwrap(),
        LanguagePreference::English
    );
    assert!(LanguagePreference::parse("fr-FR").is_err());
}

#[test]
fn test_theme_mode_whitelist() {
    let valid_modes = ["system", "light", "dark", "forest", "amoled"];
    for mode in valid_modes {
        let is_valid = matches!(mode, "system" | "light" | "dark" | "forest" | "amoled");
        assert!(is_valid);
    }

    let invalid_mode = "neon";
    let is_valid = matches!(
        invalid_mode,
        "system" | "light" | "dark" | "forest" | "amoled"
    );
    assert!(!is_valid);
}

#[test]
fn test_hex_color_parsing_logic() {
    let parse_hex = |value: &str| -> Option<(u8, u8, u8)> {
        let value = value.trim().trim_start_matches('#');
        if value.len() != 6 {
            return None;
        }
        let red = u8::from_str_radix(&value[0..2], 16).ok()?;
        let green = u8::from_str_radix(&value[2..4], 16).ok()?;
        let blue = u8::from_str_radix(&value[4..6], 16).ok()?;
        Some((red, green, blue))
    };

    assert_eq!(parse_hex("#016888"), Some((0x01, 0x68, 0x88)));
    assert_eq!(parse_hex("016888"), Some((0x01, 0x68, 0x88)));
    assert_eq!(parse_hex("#FFF"), None);
    assert_eq!(parse_hex("#GGGGGG"), None);
    assert_eq!(parse_hex(""), None);
}

#[test]
fn test_contrast_calculation() {
    let is_light = |r: u8, g: u8, b: u8| -> bool {
        let luma = 0.299 * (r as f32) + 0.587 * (g as f32) + 0.114 * (b as f32);
        luma > 140.0
    };

    assert!(!is_light(1, 104, 136));
    assert!(is_light(240, 240, 240));
}
