use anyhow::anyhow;
use serde::Serialize;

use crate::color_quantize::{QuantizedColor, quantize};
use crate::commands::TerminalAction;
use crate::output;
use crate::output::print_table;
use crate::terminal_capability::{ColorDepth, TerminalCapabilities};

/// 打印协商到的终端能力，或把一段真彩折算到各档位。
pub(crate) fn handle(
    action: TerminalAction,
    capabilities: TerminalCapabilities,
) -> anyhow::Result<()> {
    match action {
        TerminalAction::Capabilities { json } => {
            let report = CapabilityReport::new(capabilities);
            if json {
                output::print_json(&report)?;
            } else {
                print_table(&["Capability", "Value"], &capability_rows(&report));
            }
        }
        TerminalAction::Quantize { color, json } => {
            let report = QuantizeReport::parse(&color)?;
            if json {
                output::print_json(&report)?;
            } else {
                print_table(&["Tier", "Result"], &quantize_rows(&report));
            }
        }
    }
    Ok(())
}

/// 终端能力的 JSON 视图。
#[derive(Serialize)]
struct CapabilityReport {
    /// 色彩深度档位标识。
    color_depth: &'static str,
    /// 色彩深度档位可用的颜色总数。
    color_palette_size: u32,
    /// 图形协议标识。
    graphics_protocol: &'static str,
    /// 输出字符集档案标识（EFFECTS-043）。
    charset: &'static str,
}

impl CapabilityReport {
    fn new(capabilities: TerminalCapabilities) -> Self {
        Self {
            color_depth: capabilities.color_depth.label(),
            color_palette_size: capabilities.color_depth.palette_size(),
            graphics_protocol: capabilities.graphics_protocol.label(),
            charset: capabilities.charset.label(),
        }
    }
}

/// 能力报告的表格视图。
fn capability_rows(report: &CapabilityReport) -> Vec<Vec<String>> {
    vec![
        vec![
            "color depth".to_string(),
            format!(
                "{} ({} colors)",
                report.color_depth, report.color_palette_size
            ),
        ],
        vec![
            "graphics protocol".to_string(),
            report.graphics_protocol.to_string(),
        ],
        vec!["charset".to_string(), report.charset.to_string()],
    ]
}

/// 量化结果的 JSON 视图。
#[derive(Serialize)]
struct QuantizeReport {
    /// 输入真彩。
    source: RgbChannels,
    /// 真彩档的结果。
    truecolor: RgbChannels,
    /// 256 色档的结果。
    ansi256: PaletteEntry,
    /// 16 色档的结果。
    ansi16: AnsiEntry,
}

/// 真彩三通道。
#[derive(Serialize)]
struct RgbChannels {
    /// 红通道。
    red: u8,
    /// 绿通道。
    green: u8,
    /// 蓝通道。
    blue: u8,
}

/// 256 色调色板条目。
#[derive(Serialize)]
struct PaletteEntry {
    /// 调色板索引。
    index: u8,
}

/// 16 色基础 ANSI 条目。
#[derive(Serialize)]
struct AnsiEntry {
    /// ANSI 槽位标识。
    slot: &'static str,
}

impl QuantizeReport {
    /// 解析 `rrggbb` 真彩并算出三档结果。
    fn parse(color: &str) -> anyhow::Result<Self> {
        let (red, green, blue) = parse_hex_color(color)?;
        let ansi256 = palette_index(quantize(red, green, blue, ColorDepth::Ansi256))?;
        let ansi16 = ansi_slot(quantize(red, green, blue, ColorDepth::Ansi16))?;
        Ok(Self {
            source: RgbChannels { red, green, blue },
            truecolor: RgbChannels { red, green, blue },
            ansi256: PaletteEntry { index: ansi256 },
            ansi16: AnsiEntry { slot: ansi16 },
        })
    }
}

/// 量化结果的表格视图。
fn quantize_rows(report: &QuantizeReport) -> Vec<Vec<String>> {
    vec![
        vec![
            "truecolor".to_string(),
            format!(
                "rgb({}, {}, {})",
                report.truecolor.red, report.truecolor.green, report.truecolor.blue
            ),
        ],
        vec![
            "ansi256".to_string(),
            format!("index {}", report.ansi256.index),
        ],
        vec!["ansi16".to_string(), report.ansi16.slot.to_string()],
    ]
}

/// 量化结果里的 256 色调色板索引。
fn palette_index(color: QuantizedColor) -> anyhow::Result<u8> {
    match color {
        QuantizedColor::Ansi256 { index } => Ok(index),
        QuantizedColor::TrueColor { .. } | QuantizedColor::Ansi16 { .. } => Err(anyhow!(
            "the 256 color tier produced a color from another tier"
        )),
    }
}

/// 量化结果里的 16 色基础 ANSI 槽位。
fn ansi_slot(color: QuantizedColor) -> anyhow::Result<&'static str> {
    match color {
        QuantizedColor::Ansi16 { slot } => Ok(slot.label()),
        QuantizedColor::TrueColor { .. } | QuantizedColor::Ansi256 { .. } => Err(anyhow!(
            "the 16 color tier produced a color from another tier"
        )),
    }
}

/// 解析 `rrggbb` 形式的真彩。
fn parse_hex_color(color: &str) -> anyhow::Result<(u8, u8, u8)> {
    let trimmed = color.trim().trim_start_matches('#');
    if trimmed.len() != 6 || !trimmed.chars().all(|digit| digit.is_ascii_hexdigit()) {
        return Err(anyhow!(
            "color must be six hexadecimal digits (rrggbb), got '{color}'"
        ));
    }
    let red = u8::from_str_radix(&trimmed[0..2], 16)?;
    let green = u8::from_str_radix(&trimmed[2..4], 16)?;
    let blue = u8::from_str_radix(&trimmed[4..6], 16)?;
    Ok((red, green, blue))
}

#[cfg(test)]
#[path = "terminal_test.rs"]
mod terminal_test;
