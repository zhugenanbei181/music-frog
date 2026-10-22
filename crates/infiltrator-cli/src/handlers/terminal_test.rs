use crate::terminal_capability::{ColorDepth, GraphicsProtocol, TerminalCapabilities};

use super::{CapabilityReport, QuantizeReport, capability_rows, parse_hex_color, quantize_rows};

#[test]
fn capability_rows_report_the_negotiated_tiers() {
    let report = CapabilityReport::new(TerminalCapabilities {
        color_depth: ColorDepth::Ansi256,
        graphics_protocol: GraphicsProtocol::Sixel,
    });
    let rows = capability_rows(&report);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0], "color depth");
    assert_eq!(rows[0][1], "ansi256 (256 colors)");
    assert_eq!(rows[1][0], "graphics protocol");
    assert_eq!(rows[1][1], "sixel");
}

#[test]
fn quantize_report_folds_the_color_into_every_tier() {
    let report = QuantizeReport::parse("4f8cff").unwrap();
    assert_eq!(report.source.red, 79);
    assert_eq!(report.source.green, 140);
    assert_eq!(report.source.blue, 255);
    assert_eq!(report.truecolor.red, 79);
    assert_eq!(report.ansi256.index, 69);
    assert_eq!(report.ansi16.slot, "dark-gray");
}

#[test]
fn quantize_report_keeps_pure_colors_on_their_base_slot() {
    let report = QuantizeReport::parse("#0000ff").unwrap();
    assert_eq!(report.ansi256.index, 12);
    assert_eq!(report.ansi16.slot, "light-blue");
}

#[test]
fn quantize_rows_render_every_tier() {
    let report = QuantizeReport::parse("4f8cff").unwrap();
    let rows = quantize_rows(&report);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][0], "truecolor");
    assert_eq!(rows[0][1], "rgb(79, 140, 255)");
    assert_eq!(rows[1][0], "ansi256");
    assert_eq!(rows[1][1], "index 69");
    assert_eq!(rows[2][0], "ansi16");
    assert_eq!(rows[2][1], "dark-gray");
}

#[test]
fn hex_color_parsing_accepts_six_digits_and_rejects_garbage() {
    assert_eq!(parse_hex_color("4f8cff").unwrap(), (79, 140, 255));
    assert_eq!(parse_hex_color("#4F8CFF").unwrap(), (79, 140, 255));
    assert!(parse_hex_color("4f8cf").is_err());
    assert!(parse_hex_color("zzzzzz").is_err());
    assert!(parse_hex_color("").is_err());
    assert!(parse_hex_color("0x4f8cff").is_err());
}
