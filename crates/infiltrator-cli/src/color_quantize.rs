//! 三级色彩量化：把 24 位真彩折叠到终端真正支持的色彩档位。
//!
//! 量化是纯函数：同一组输入永远得到同一份结果，因此渲染层可以在启动期就把
//! 主题色折算好，绘制时直接取用，不必逐帧判断终端档位。

use crate::terminal_capability::ColorDepth;

/// 16 色基础 ANSI 槽位。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnsiSlot {
    /// 黑色。
    Black,
    /// 红色。
    Red,
    /// 绿色。
    Green,
    /// 黄色。
    Yellow,
    /// 蓝色。
    Blue,
    /// 品红。
    Magenta,
    /// 青色。
    Cyan,
    /// 灰色（亮黑）。
    Gray,
    /// 深灰（亮黑）。
    DarkGray,
    /// 亮红。
    LightRed,
    /// 亮绿。
    LightGreen,
    /// 亮黄。
    LightYellow,
    /// 亮蓝。
    LightBlue,
    /// 亮品红。
    LightMagenta,
    /// 亮青。
    LightCyan,
    /// 白色。
    White,
}

impl AnsiSlot {
    /// 稳定标识，文本输出与 JSON 视图共用。
    pub(crate) fn label(self) -> &'static str {
        match self {
            AnsiSlot::Black => "black",
            AnsiSlot::Red => "red",
            AnsiSlot::Green => "green",
            AnsiSlot::Yellow => "yellow",
            AnsiSlot::Blue => "blue",
            AnsiSlot::Magenta => "magenta",
            AnsiSlot::Cyan => "cyan",
            AnsiSlot::Gray => "gray",
            AnsiSlot::DarkGray => "dark-gray",
            AnsiSlot::LightRed => "light-red",
            AnsiSlot::LightGreen => "light-green",
            AnsiSlot::LightYellow => "light-yellow",
            AnsiSlot::LightBlue => "light-blue",
            AnsiSlot::LightMagenta => "light-magenta",
            AnsiSlot::LightCyan => "light-cyan",
            AnsiSlot::White => "white",
        }
    }
}

/// 量化后的颜色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuantizedColor {
    /// 24 位真彩。
    TrueColor {
        /// 红通道。
        red: u8,
        /// 绿通道。
        green: u8,
        /// 蓝通道。
        blue: u8,
    },
    /// 256 色调色板索引。
    Ansi256 {
        /// 调色板索引。
        index: u8,
    },
    /// 16 色基础 ANSI 槽位。
    Ansi16 {
        /// ANSI 槽位。
        slot: AnsiSlot,
    },
}

/// 16 色基础 ANSI 槽位与 xterm 默认近似 RGB。
const BASE_16: [(AnsiSlot, (u8, u8, u8)); 16] = [
    (AnsiSlot::Black, (0, 0, 0)),
    (AnsiSlot::Red, (128, 0, 0)),
    (AnsiSlot::Green, (0, 128, 0)),
    (AnsiSlot::Yellow, (128, 128, 0)),
    (AnsiSlot::Blue, (0, 0, 128)),
    (AnsiSlot::Magenta, (128, 0, 128)),
    (AnsiSlot::Cyan, (0, 128, 128)),
    (AnsiSlot::Gray, (192, 192, 192)),
    (AnsiSlot::DarkGray, (128, 128, 128)),
    (AnsiSlot::LightRed, (255, 0, 0)),
    (AnsiSlot::LightGreen, (0, 255, 0)),
    (AnsiSlot::LightYellow, (255, 255, 0)),
    (AnsiSlot::LightBlue, (0, 0, 255)),
    (AnsiSlot::LightMagenta, (255, 0, 255)),
    (AnsiSlot::LightCyan, (0, 255, 255)),
    (AnsiSlot::White, (255, 255, 255)),
];

/// 6×6×6 色立方体的灰阶层级。
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// 24 级灰阶梯度的起始亮度。
const GRAYSCALE_START: u8 = 8;

/// 24 级灰阶梯度的亮度步长。
const GRAYSCALE_STEP: u8 = 10;

/// 按终端色彩档位量化真彩。
pub(crate) fn quantize(red: u8, green: u8, blue: u8, depth: ColorDepth) -> QuantizedColor {
    match depth {
        ColorDepth::TrueColor => QuantizedColor::TrueColor { red, green, blue },
        ColorDepth::Ansi256 => QuantizedColor::Ansi256 {
            index: nearest_palette_index(red, green, blue),
        },
        ColorDepth::Ansi16 => QuantizedColor::Ansi16 {
            slot: nearest_base_slot(red, green, blue),
        },
    }
}

/// xterm 256 色调色板里与给定真彩最近的一项。
///
/// 距离相同时取较小索引：调色板前半段是基础 ANSI 色，优先命中基础色可以让
/// 16 色终端沿用同一套主题色。
fn nearest_palette_index(red: u8, green: u8, blue: u8) -> u8 {
    let mut best_index = 0u8;
    let mut best_distance = u32::MAX;
    for index in 0u16..256 {
        let (entry_red, entry_green, entry_blue) = palette_entry(index as u8);
        let distance = squared_distance(red, green, blue, entry_red, entry_green, entry_blue);
        if distance < best_distance {
            best_distance = distance;
            best_index = index as u8;
        }
    }
    best_index
}

/// 16 色基础 ANSI 里与给定真彩最近的一个槽位。
fn nearest_base_slot(red: u8, green: u8, blue: u8) -> AnsiSlot {
    let mut best_slot = AnsiSlot::Black;
    let mut best_distance = u32::MAX;
    for (slot, (entry_red, entry_green, entry_blue)) in BASE_16 {
        let distance = squared_distance(red, green, blue, entry_red, entry_green, entry_blue);
        if distance < best_distance {
            best_distance = distance;
            best_slot = slot;
        }
    }
    best_slot
}

/// xterm 256 色调色板某一项的近似 RGB。
///
/// 布局与 xterm 一致：`0..=15` 是基础 ANSI 色，`16..=231` 是 6×6×6 色立方，
/// `232..=255` 是 24 级灰阶梯。
fn palette_entry(index: u8) -> (u8, u8, u8) {
    match index {
        0..=15 => BASE_16[index as usize].1,
        16..=231 => {
            let offset = index - 16;
            let red = offset / 36;
            let green = (offset % 36) / 6;
            let blue = offset % 6;
            (
                CUBE_LEVELS[red as usize],
                CUBE_LEVELS[green as usize],
                CUBE_LEVELS[blue as usize],
            )
        }
        _ => {
            let level = GRAYSCALE_START + (index - 232) * GRAYSCALE_STEP;
            (level, level, level)
        }
    }
}

/// 两个 RGB 之间的平方距离。
fn squared_distance(
    red: u8,
    green: u8,
    blue: u8,
    other_red: u8,
    other_green: u8,
    other_blue: u8,
) -> u32 {
    let red = u32::from(red.abs_diff(other_red));
    let green = u32::from(green.abs_diff(other_green));
    let blue = u32::from(blue.abs_diff(other_blue));
    red * red + green * green + blue * blue
}

#[cfg(test)]
#[path = "color_quantize_test.rs"]
mod color_quantize_test;
