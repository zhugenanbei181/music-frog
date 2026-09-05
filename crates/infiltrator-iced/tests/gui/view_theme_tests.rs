use super::*;

#[test]
fn test_theme_names_and_detection() {
    let light = theme_from_name("light");
    let dark = theme_from_name("dark");
    let forest = theme_from_name("forest");
    let amoled = theme_from_name("amoled");
    let black = theme_from_name("black");

    assert_eq!(theme_to_name(&light), "light");
    assert_eq!(theme_to_name(&dark), "dark");
    assert_eq!(theme_to_name(&forest), "forest");
    assert_eq!(theme_to_name(&amoled), "amoled");
    assert_eq!(theme_to_name(&black), "amoled");

    assert!(!is_forest(&light));
    assert!(!is_forest(&dark));
    assert!(is_forest(&forest));
    assert!(!is_forest(&amoled));

    assert!(!is_amoled(&light));
    assert!(!is_amoled(&dark));
    assert!(!is_amoled(&forest));
    assert!(is_amoled(&amoled));
    assert!(is_amoled(&black));
}

#[test]
fn test_tokens_resolution() {
    let light = theme_from_name("light");
    let dark = theme_from_name("dark");
    let forest = theme_from_name("forest");
    let amoled = theme_from_name("amoled");

    assert_eq!(tokens(&light).canvas, LIGHT.canvas);
    assert_eq!(tokens(&dark).canvas, DARK.canvas);
    assert_eq!(tokens(&forest).canvas, FOREST.canvas);
    assert_eq!(tokens(&amoled).canvas, AMOLED.canvas);
    assert_eq!(tokens(&amoled).canvas, Color::from_rgb(0.0, 0.0, 0.0));
}

#[test]
fn test_floating_shadows() {
    const {
        assert!(LIGHT.floating_shadow.blur_radius > LIGHT.card_shadow.blur_radius);
        assert!(DARK.floating_shadow.blur_radius > DARK.card_shadow.blur_radius);
        assert!(FOREST.floating_shadow.blur_radius > FOREST.card_shadow.blur_radius);
        assert!(AMOLED.floating_shadow.blur_radius > AMOLED.card_shadow.blur_radius);
    }

    const {
        assert!(LIGHT.floating_shadow.color.a > LIGHT.card_shadow.color.a);
        assert!(DARK.floating_shadow.color.a > DARK.card_shadow.color.a);
        assert!(FOREST.floating_shadow.color.a > FOREST.card_shadow.color.a);
        assert!(AMOLED.floating_shadow.color.a > AMOLED.card_shadow.color.a);
    }
}

#[test]
fn test_accent_presets() {
    for preset in AccentPreset::ALL {
        let name = preset.as_str();
        assert_eq!(AccentPreset::from_name(name), Some(preset));

        let modified = DARK.with_accent_preset(preset);
        assert_eq!(modified.accent, preset.color(true));
    }

    let custom_color = Color::from_rgb(0.5, 0.2, 0.8);
    let custom_tokens = AMOLED.with_accent(custom_color);
    assert_eq!(custom_tokens.accent, custom_color);
    assert_eq!(custom_tokens.badge_accent, custom_color);
}
