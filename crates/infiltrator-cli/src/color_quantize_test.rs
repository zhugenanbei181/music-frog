use super::{AnsiSlot, QuantizedColor, palette_entry, quantize};
use crate::terminal_capability::ColorDepth;

#[test]
fn truecolor_keeps_the_original_channels() {
    let quantized = quantize(79, 140, 255, ColorDepth::TrueColor);
    assert_eq!(
        quantized,
        QuantizedColor::TrueColor {
            red: 79,
            green: 140,
            blue: 255
        }
    );
}

#[test]
fn ansi256_prefers_the_base_color_on_an_exact_match() {
    assert_eq!(
        quantize(255, 0, 0, ColorDepth::Ansi256),
        QuantizedColor::Ansi256 { index: 9 }
    );
    assert_eq!(
        quantize(0, 0, 0, ColorDepth::Ansi256),
        QuantizedColor::Ansi256 { index: 0 }
    );
    assert_eq!(
        quantize(255, 255, 255, ColorDepth::Ansi256),
        QuantizedColor::Ansi256 { index: 15 }
    );
}

#[test]
fn ansi256_falls_back_to_the_cube_when_no_base_color_is_close() {
    // (250, 40, 60) 离基础色都很远，最近的是色立方里的高饱和红。
    let quantized = quantize(250, 40, 60, ColorDepth::Ansi256);
    let QuantizedColor::Ansi256 { index } = quantized else {
        panic!("the 256 color tier must answer with a palette index");
    };
    assert!(
        index >= 16 && index <= 231,
        "unexpected palette index {index}"
    );
    let (red, green, blue) = palette_entry(index);
    assert!(red > 200, "the cube entry should keep the red channel");
    assert!(
        green < 100,
        "the cube entry should keep the green channel low"
    );
    assert!(
        blue < 120,
        "the cube entry should keep the blue channel low"
    );
}

#[test]
fn ansi16_maps_pure_colors_onto_the_base_slots() {
    assert_eq!(
        quantize(255, 0, 0, ColorDepth::Ansi16),
        QuantizedColor::Ansi16 {
            slot: AnsiSlot::LightRed
        }
    );
    assert_eq!(
        quantize(0, 0, 0, ColorDepth::Ansi16),
        QuantizedColor::Ansi16 {
            slot: AnsiSlot::Black
        }
    );
    assert_eq!(
        quantize(192, 192, 192, ColorDepth::Ansi16),
        QuantizedColor::Ansi16 {
            slot: AnsiSlot::Gray
        }
    );
}

#[test]
fn palette_layout_follows_the_xterm_convention() {
    assert_eq!(palette_entry(0), (0, 0, 0));
    assert_eq!(palette_entry(15), (255, 255, 255));
    assert_eq!(palette_entry(16), (0, 0, 0));
    assert_eq!(palette_entry(196), (255, 0, 0));
    assert_eq!(palette_entry(231), (255, 255, 255));
    assert_eq!(palette_entry(232), (8, 8, 8));
    assert_eq!(palette_entry(255), (238, 238, 238));
}

#[test]
fn quantized_palette_entry_stays_close_to_the_source_color() {
    let quantized = quantize(79, 140, 255, ColorDepth::Ansi256);
    let QuantizedColor::Ansi256 { index } = quantized else {
        panic!("the 256 color tier must answer with a palette index");
    };
    let (red, green, blue) = palette_entry(index);
    let distance = u32::from(red.abs_diff(79))
        + u32::from(green.abs_diff(140))
        + u32::from(blue.abs_diff(255));
    assert!(
        distance <= 120,
        "the palette entry drifted too far: {distance}"
    );
}

#[test]
fn quantization_is_deterministic() {
    for depth in [
        ColorDepth::TrueColor,
        ColorDepth::Ansi256,
        ColorDepth::Ansi16,
    ] {
        assert_eq!(quantize(12, 200, 90, depth), quantize(12, 200, 90, depth));
    }
}

#[test]
fn ansi_slot_labels_are_stable() {
    assert_eq!(AnsiSlot::Black.label(), "black");
    assert_eq!(AnsiSlot::DarkGray.label(), "dark-gray");
    assert_eq!(AnsiSlot::LightBlue.label(), "light-blue");
    assert_eq!(AnsiSlot::White.label(), "white");
}
