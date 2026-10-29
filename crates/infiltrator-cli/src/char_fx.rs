//! 字符艺术与轻量动效（EFFECTS-044 #176~#180）。
//!
//! 不引入外部动效或图片库：波形用盲文点阵（2×4 点/字符）画，图片回退用
//! 半块字符（▀ 上下两像素）画，状态转场用固定的帧序列就地刷新且结束立即
//! 停止 tick。动效只允许出现在进度区（[`ProgressAnimation`] 只能从
//! [`ProgressAnimation::for_progress`] 构造），列表与静态输出不允许有动效。

use crate::color_quantize::{QuantizedColor, quantize};
use crate::terminal_capability::ColorDepth;

/// 盲文点阵一列的采样档数：每个盲文字符上下 4 行点。
const BRAILLE_ROWS: usize = 4;

/// 盲文波形（#180）：把采样序列折叠成一行盲文点阵。
///
/// 每个盲文字符承载相邻两列采样（盲文是 2×4 点阵）：把 0.0..=1.0 的采样
/// 值量化到 0..4 的点高度，左列点占低 8 位、右列点占高 8 位（Unicode 盲文
/// 的位序）。采样不足两列时静默输出空串。
pub(crate) fn braille_waveform(samples: &[f32]) -> String {
    const LEFT_DOTS: [u32; 4] = [0x01, 0x02, 0x04, 0x40];
    const RIGHT_DOTS: [u32; 4] = [0x08, 0x10, 0x20, 0x80];
    let mut out = String::with_capacity(samples.len() / 2 + 1);
    let pair = samples.chunks(2);
    for chunk in pair {
        let mut codepoint = 0x2800u32;
        for (column, sample) in chunk.iter().enumerate() {
            let clamped = sample.clamp(0.0, 1.0);
            let height = (clamped * BRAILLE_ROWS as f32).ceil() as usize;
            let dots = if column == 0 { &LEFT_DOTS } else { &RIGHT_DOTS };
            for &dot in dots.iter().take(height.min(BRAILLE_ROWS)) {
                codepoint |= dot;
            }
        }
        out.push(char::from_u32(codepoint).expect("盲文码位恒有效"));
    }
    out
}

/// 半块字符图片回退（#179）：每个输出行用 `▀` 承载上下两个像素行。
///
/// 颜色按协商出的色彩档位量化（#178 的字符艺术腿复用同一量化器）：
/// 上像素作前景、下像素作背景，返回的每一行都在行尾复位。图片行不足
/// 两行的尾巴按黑色补齐；输入为空返回空。
pub(crate) fn halfblock_rows(pixel_rows: &[Vec<[u8; 3]>], depth: ColorDepth) -> Vec<String> {
    let mut rows = Vec::new();
    for pair in pixel_rows.chunks(2) {
        let top = pair.first().map(Vec::as_slice).unwrap_or(&[]);
        let bottom = pair.get(1).map(|row| row.as_slice()).unwrap_or(&[]);
        let columns = top.len().max(bottom.len());
        let mut line = String::new();
        for column in 0..columns {
            let upper = top.get(column).copied().unwrap_or([0, 0, 0]);
            let lower = bottom.get(column).copied().unwrap_or([0, 0, 0]);
            line.push_str(&halfblock_cell(upper, lower, depth));
        }
        if !line.is_empty() {
            line.push_str("\x1b[0m");
        }
        rows.push(line);
    }
    rows
}

/// 一个半块单元格：上像素作前景、下像素作背景。
fn halfblock_cell(top: [u8; 3], bottom: [u8; 3], depth: ColorDepth) -> String {
    let foreground = quantize(top[0], top[1], top[2], depth);
    let background = quantize(bottom[0], bottom[1], bottom[2], depth);
    format!(
        "\x1b[{};{}m▀",
        color_body(&foreground, true),
        color_body(&background, false)
    )
}

/// 量化色的 SGR 参数体：前景 `38;2;r;g;b`/`38;5;n`/`30..=37|90..=97`，
/// 背景对应 `48;…`/`40..=47|100..=107`。
fn color_body(color: &QuantizedColor, foreground: bool) -> String {
    let (truecolor_prefix, palette_prefix, base) = if foreground {
        ("38;2", "38;5", 30)
    } else {
        ("48;2", "48;5", 40)
    };
    match color {
        QuantizedColor::TrueColor { red, green, blue } => {
            format!("{truecolor_prefix};{red};{green};{blue}")
        }
        QuantizedColor::Ansi256 { index } => format!("{palette_prefix};{index}"),
        // 16 色档的前 8 槽走基础段，后 8 槽走亮色段（前景 +60、背景 +100）。
        QuantizedColor::Ansi16 { slot } => {
            let code = ansi16_code(slot.label());
            if code >= 8 {
                format!("{}", base + 60 + code as u32 - 8)
            } else {
                format!("{}", base + code as u32)
            }
        }
    }
}

/// 16 色槽位标识到 ANSI 槽号的映射，键与 `AnsiSlot::label` 一致。
fn ansi16_code(slot: &str) -> u8 {
    match slot {
        "black" => 0,
        "red" => 1,
        "green" => 2,
        "yellow" => 3,
        "blue" => 4,
        "magenta" => 5,
        "cyan" => 6,
        "gray" => 7,
        "dark-gray" => 8,
        "light-red" => 9,
        "light-green" => 10,
        "light-yellow" => 11,
        "light-blue" => 12,
        "light-magenta" => 13,
        "light-cyan" => 14,
        "white" => 15,
        _ => 7,
    }
}

/// 状态转场动效（#176~#177）：固定帧序列 + 帧间隔，结束立即停止。
///
/// 帧序列是产品固定资产（不可由外部输入构造任意闪烁序列）；
/// [`ProgressAnimation::frame_at`] 在超过总时长后返回 `None`——tick 立即
/// 停止，动效不会无限占用刷新。
pub(crate) struct ProgressAnimation {
    frames: &'static [&'static str],
    frame_ms: u32,
}

impl ProgressAnimation {
    /// 进度区专用的标准转场（braille 旋转）。
    pub(crate) fn for_progress() -> Self {
        Self {
            frames: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            frame_ms: 80,
        }
    }

    /// 某一时间点应显示的帧；动效结束后返回 `None`（结束即停）。
    pub(crate) fn frame_at(&self, elapsed_ms: u32) -> Option<&'static str> {
        let total = self.frames.len() as u32 * self.frame_ms;
        if elapsed_ms >= total {
            return None;
        }
        let index = (elapsed_ms / self.frame_ms) as usize % self.frames.len();
        Some(self.frames[index])
    }

    /// 一次完整转场的总时长（毫秒）。
    pub(crate) fn total_ms(&self) -> u32 {
        self.frames.len() as u32 * self.frame_ms
    }
}

#[cfg(test)]
#[path = "char_fx_test.rs"]
mod char_fx_test;
