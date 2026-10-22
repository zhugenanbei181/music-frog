//! 启动期终端能力协商：色彩深度与图形协议。
//!
//! 协商只读取终端自报的环境信号（`TERM`、`COLORTERM`、`TERM_PROGRAM`、
//! `KITTY_WINDOW_ID`、`XTERM_VERSION`、`WT_SESSION`、`NO_COLOR`、
//! `FORCE_COLOR`），全程不读终端输入：能力在进入交互循环之前就已确定，既不会
//! 抢占按键，也不会因为等待终端应答而卡死。
//!
//! 协商结果是纯函数式的快照，同一组信号永远得到同一份能力，渲染层因此可以在
//! 绘制时直接按档位取色，不必反复探测终端。

use std::env;

/// 终端色彩深度档位。
///
/// 档位是渲染层取色的依据：终端自报的能力决定上限，使用者的显式约束
/// （`NO_COLOR`、`FORCE_COLOR`）把档位往下压。量化层按档位把 24 位真彩折叠到
/// 该档位覆盖的范围里。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColorDepth {
    /// 24 位真彩。
    TrueColor,
    /// 256 色调色板。
    Ansi256,
    /// 16 色基础 ANSI。
    Ansi16,
}

impl ColorDepth {
    /// 该档位可用的颜色总数。
    pub(crate) fn palette_size(self) -> u32 {
        match self {
            ColorDepth::TrueColor => 16_777_216,
            ColorDepth::Ansi256 => 256,
            ColorDepth::Ansi16 => 16,
        }
    }

    /// 稳定标识，文本输出与 JSON 视图共用。
    pub(crate) fn label(self) -> &'static str {
        match self {
            ColorDepth::TrueColor => "truecolor",
            ColorDepth::Ansi256 => "ansi256",
            ColorDepth::Ansi16 => "ansi16",
        }
    }
}

/// 终端图形协议。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GraphicsProtocol {
    /// Kitty 图形协议：逐像素传输并用 Unicode 占位符定位。
    Kitty,
    /// Sixel 图形协议：DEC 六位图形。
    Sixel,
    /// 终端没有可用的图形协议。
    None,
}

impl GraphicsProtocol {
    /// 稳定标识，文本输出与 JSON 视图共用。
    pub(crate) fn label(self) -> &'static str {
        match self {
            GraphicsProtocol::Kitty => "kitty",
            GraphicsProtocol::Sixel => "sixel",
            GraphicsProtocol::None => "none",
        }
    }
}

/// 协商出来的终端能力快照。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalCapabilities {
    /// 色彩深度档位。
    pub(crate) color_depth: ColorDepth,
    /// 可用的图形协议。
    pub(crate) graphics_protocol: GraphicsProtocol,
}

/// 终端自报的环境信号快照。
///
/// 协商的输入只有这些信号，因此协商过程可以完全离线复现。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TerminalEnvironment {
    /// `TERM`：终端类型标识。
    pub(crate) term: Option<String>,
    /// `COLORTERM`：色彩能力标识。
    pub(crate) colorterm: Option<String>,
    /// `TERM_PROGRAM`：终端程序名。
    pub(crate) term_program: Option<String>,
    /// `KITTY_WINDOW_ID`：kitty 窗口标识，存在即说明当前跑在 kitty 里。
    pub(crate) kitty_window_id: Option<String>,
    /// `XTERM_VERSION`：xterm 版本标识。
    pub(crate) xterm_version: Option<String>,
    /// `WT_SESSION`：Windows Terminal 会话标识。
    pub(crate) wt_session: Option<String>,
    /// `NO_COLOR`：存在且非空即表示禁用彩色输出。
    pub(crate) no_color: Option<String>,
    /// `FORCE_COLOR`：强制彩色输出的档位。
    pub(crate) force_color: Option<String>,
}

impl TerminalEnvironment {
    /// 从当前进程环境读取终端信号；缺失或非法的信号记为 `None`。
    pub(crate) fn from_process() -> Self {
        Self {
            term: env::var("TERM").ok(),
            colorterm: env::var("COLORTERM").ok(),
            term_program: env::var("TERM_PROGRAM").ok(),
            kitty_window_id: env::var("KITTY_WINDOW_ID").ok(),
            xterm_version: env::var("XTERM_VERSION").ok(),
            wt_session: env::var("WT_SESSION").ok(),
            no_color: env::var("NO_COLOR").ok(),
            force_color: env::var("FORCE_COLOR").ok(),
        }
    }
}

/// 依据终端自报的信号协商能力。
pub(crate) fn negotiate(environment: &TerminalEnvironment) -> TerminalCapabilities {
    TerminalCapabilities {
        color_depth: negotiate_color_depth(environment),
        graphics_protocol: negotiate_graphics_protocol(environment),
    }
}

/// 色彩深度协商。
///
/// 优先级自上而下：
///
/// 1. `FORCE_COLOR`：使用者的显式覆盖；
/// 2. `NO_COLOR`：使用者的显式禁用，落到最低档；
/// 3. `COLORTERM=truecolor|24bit`：终端自报真彩；
/// 4. `TERM` 片段：`direct` 为真彩、`256color` 为 256 色、`color` 为 16 色；
/// 5. 已知真彩终端的 `TERM` / `TERM_PROGRAM` 标识；
/// 6. 其余一律按 16 色兜底，宁可少色也不错色。
fn negotiate_color_depth(environment: &TerminalEnvironment) -> ColorDepth {
    if let Some(depth) = forced_color_depth(environment) {
        return depth;
    }
    if is_set(environment.no_color.as_deref()) {
        return ColorDepth::Ansi16;
    }
    let colorterm = lower(environment.colorterm.as_deref());
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return ColorDepth::TrueColor;
    }
    let term = lower(environment.term.as_deref());
    if term.contains("direct") {
        return ColorDepth::TrueColor;
    }
    if term.contains("256color") {
        return ColorDepth::Ansi256;
    }
    if TRUE_COLOR_TERM_MARKERS
        .iter()
        .any(|marker| term.contains(*marker))
    {
        return ColorDepth::TrueColor;
    }
    if term.contains("color") {
        return ColorDepth::Ansi16;
    }
    let term_program = lower(environment.term_program.as_deref());
    if TRUE_COLOR_TERM_PROGRAMS.contains(&term_program.as_str()) {
        return ColorDepth::TrueColor;
    }
    if is_set(environment.wt_session.as_deref()) {
        return ColorDepth::TrueColor;
    }
    ColorDepth::Ansi16
}

/// `FORCE_COLOR` 覆盖：`0`/`false` 关闭彩色，`1` 十六色，`2` 二百五十六色，
/// `3` 真彩；无法识别的取值按最低档处理。
///
/// 关闭彩色同样落到最低档：档位只描述能力，彩色要不要真的输出由上层决定。
fn forced_color_depth(environment: &TerminalEnvironment) -> Option<ColorDepth> {
    let raw = environment.force_color.as_deref()?.trim();
    if raw.is_empty() {
        return None;
    }
    match raw.to_ascii_lowercase().as_str() {
        "0" | "false" | "none" => Some(ColorDepth::Ansi16),
        "2" => Some(ColorDepth::Ansi256),
        "3" => Some(ColorDepth::TrueColor),
        _ => Some(ColorDepth::Ansi16),
    }
}

/// 图形协议协商。
///
/// kitty 由 `TERM` 片段或 `KITTY_WINDOW_ID` 判定；sixel 由 `TERM_PROGRAM`、
/// `XTERM_VERSION` 或 `TERM` 片段判定。两者同时命中时 kitty 优先：kitty 不实现
/// sixel，选 kitty 才是终端真正会渲染的那一条通路。
fn negotiate_graphics_protocol(environment: &TerminalEnvironment) -> GraphicsProtocol {
    let term = lower(environment.term.as_deref());
    if term.contains("kitty") || is_set(environment.kitty_window_id.as_deref()) {
        return GraphicsProtocol::Kitty;
    }
    let term_program = lower(environment.term_program.as_deref());
    if SIXEL_TERM_PROGRAMS.contains(&term_program.as_str()) {
        return GraphicsProtocol::Sixel;
    }
    let xterm_version = lower(environment.xterm_version.as_deref());
    if xterm_version.contains("sixel") || term.contains("sixel") {
        return GraphicsProtocol::Sixel;
    }
    GraphicsProtocol::None
}

/// 自报真彩的 `TERM` 片段。
const TRUE_COLOR_TERM_MARKERS: [&str; 7] = [
    "xterm-kitty",
    "kitty",
    "alacritty",
    "wezterm",
    "foot",
    "ghostty",
    "contour",
];

/// 自报真彩的 `TERM_PROGRAM` 取值（小写比较）。
const TRUE_COLOR_TERM_PROGRAMS: [&str; 7] = [
    "iterm.app",
    "wezterm",
    "ghostty",
    "vscode",
    "hyper",
    "mintty",
    "rio",
];

/// 自报 sixel 的 `TERM_PROGRAM` 取值（小写比较）。
const SIXEL_TERM_PROGRAMS: [&str; 5] = ["wezterm", "mlterm", "contour", "macterm", "foot"];

/// 小写化的环境信号；缺失时为空串。
fn lower(value: Option<&str>) -> String {
    value.unwrap_or_default().to_ascii_lowercase()
}

/// 环境信号是否存在且非空。
fn is_set(value: Option<&str>) -> bool {
    value.is_some_and(|text| !text.is_empty())
}

#[cfg(test)]
#[path = "terminal_capability_test.rs"]
mod terminal_capability_test;
