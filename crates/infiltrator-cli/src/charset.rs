//! 输出字符集档案与表格外框（EFFECTS-043 #174~#175）。
//!
//! 现代终端用 Unicode 圆角线框绘制表格分区；受限或老旧终端（`TERM=dumb`、
//! `linux` 控制台、显式 `unicode = simple` 覆盖）自动回退纯 ASCII 结构。
//! 字符集与色彩深度一样，是启动期协商出的纯快照，渲染层按档案取字形，
//! 不在绘制时反复探测。

use crate::terminal_capability::TerminalEnvironment;

/// 输出字符集档案。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CharsetProfile {
    /// Unicode 圆角/制表线框。
    #[default]
    Unicode,
    /// 纯 ASCII 简易字符（受限终端的安全回退）。
    AsciiSimple,
}

impl CharsetProfile {
    /// 稳定标识，文本输出与 JSON 视图共用。
    pub(crate) fn label(self) -> &'static str {
        match self {
            CharsetProfile::Unicode => "unicode",
            CharsetProfile::AsciiSimple => "ascii-simple",
        }
    }
}

/// 一套表格线框字形。
///
/// 字形按结构位取名，两套档案共用同一结构，渲染层不感知具体字符。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameGlyphs {
    /// 左上角。
    pub(crate) top_left: &'static str,
    /// 右上角。
    pub(crate) top_right: &'static str,
    /// 左下角。
    pub(crate) bottom_left: &'static str,
    /// 右下角。
    pub(crate) bottom_right: &'static str,
    /// 水平线。
    pub(crate) horizontal: &'static str,
    /// 垂直线。
    pub(crate) vertical: &'static str,
    /// 上侧丁字（列分隔上边）。
    pub(crate) top_tee: &'static str,
    /// 下侧丁字（列分隔下边）。
    pub(crate) bottom_tee: &'static str,
    /// 中部十字（列分隔中边）。
    pub(crate) cross: &'static str,
}

/// Unicode 圆角线框字形（#174）。
pub(crate) const UNICODE_FRAME: FrameGlyphs = FrameGlyphs {
    top_left: "╭",
    top_right: "╮",
    bottom_left: "╰",
    bottom_right: "╯",
    horizontal: "─",
    vertical: "│",
    top_tee: "┬",
    bottom_tee: "┴",
    cross: "┼",
};

/// 纯 ASCII 简易线框字形（#175）。
pub(crate) const ASCII_FRAME: FrameGlyphs = FrameGlyphs {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    horizontal: "-",
    vertical: "|",
    top_tee: "+",
    bottom_tee: "+",
    cross: "+",
};

impl CharsetProfile {
    /// 该档案的线框字形。
    pub(crate) fn frame(self) -> FrameGlyphs {
        match self {
            CharsetProfile::Unicode => UNICODE_FRAME,
            CharsetProfile::AsciiSimple => ASCII_FRAME,
        }
    }
}

/// 自报受限的 `TERM` 取值（小写比较）：这些终端画不好制表线，直接落 ASCII。
const LIMITED_TERMS: [&str; 4] = ["dumb", "linux", "cons25", "vt100"];

/// 依据终端自报信号与使用者覆盖协商字符集档案。
///
/// 优先级：显式 `INFILTRATOR_UNICODE` 覆盖（`simple` 落 ASCII、`full` 落
/// Unicode、其余取值忽略）→ `TERM` 受限名单 → Unicode。
pub(crate) fn negotiate_charset(environment: &TerminalEnvironment) -> CharsetProfile {
    if let Some(override_value) = environment.unicode_override.as_deref() {
        match override_value.trim().to_ascii_lowercase().as_str() {
            "simple" | "ascii" => return CharsetProfile::AsciiSimple,
            "full" | "unicode" => return CharsetProfile::Unicode,
            _ => {}
        }
    }
    let term = environment
        .term
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if LIMITED_TERMS.iter().any(|limited| term.contains(limited)) {
        return CharsetProfile::AsciiSimple;
    }
    CharsetProfile::Unicode
}

#[cfg(test)]
#[path = "charset_test.rs"]
mod charset_test;
