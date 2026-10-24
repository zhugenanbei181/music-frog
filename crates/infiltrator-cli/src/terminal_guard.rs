//! 终端输出状态守卫与配置覆盖（EFFECTS-045 #181~#185）。
//!
//! CLI 一切彩色/光标/折行改动都必须可恢复：守卫在 Drop 时复位颜色与光标、
//! 折行状态，并把同一复位接进 panic 钧子链——程序即使崩溃，终端也回到
//! 干净状态。显示宽度按共享字符宽度表（全角 2 列、半角 1 列、零宽 0 列）
//! 计算，CJK 与 Emoji 不再撑破表对齐；`infiltrator-terminal.toml` 提供三
//! 个键的使用者覆盖（`unicode`、`animation`、`color`）。

use infiltrator_shared::effect_assets::char_width_class;
use std::env;
use std::fs;
use std::io::{Write, stderr};
use std::panic::{PanicHookInfo, set_hook, take_hook};
use std::path::PathBuf;
use toml::map::Map;

/// 终端复位序列：属性复位、光标显示、自动折行开启。
pub(crate) const RESTORE_SEQUENCE: &str = "\x1b[0m\x1b[?25h\x1b[?7h";

/// 终端输出状态守卫（#181、#184）。
///
/// `install` 记录当前 panic 钩子并换上先复位终端再调原钩子的链式钩子；
/// 守卫 Drop 时再写一次复位序列。正常结束、提前返回与 panic 三条路径
/// 都被覆盖。守卫是进程生命期的单例：`install` 只在 CLI 入口调用一次，
/// 链式钩子随进程存续。
pub(crate) struct TerminalGuard;

impl TerminalGuard {
    /// 安装守卫：链式 panic 钩子 + 进程退出时的 Drop 复位。
    pub(crate) fn install() -> Self {
        let previous = take_hook();
        let chained: Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send> =
            Box::new(move |info| {
                restore_terminal();
                if let Some(hook) = previous.as_ref() {
                    hook(info);
                }
            });
        set_hook(chained);
        Self
    }

    /// 立即写一次复位序列（不等 Drop）：就地刷新动效结束后调用。
    pub(crate) fn restore_now(&self) {
        restore_terminal();
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// 把复位序列写到 stderr（panic 路径上 stdout 可能正被占用）。
fn restore_terminal() {
    let mut stderr = stderr();
    let _ = stderr.write_all(RESTORE_SEQUENCE.as_bytes());
    let _ = stderr.flush();
}

/// 文本的显示宽度（#183）：全角 2 列、半角 1 列、零宽（组合符、变体
/// 选择符）0 列，宽度事实来自共享字符宽度表（EFFECTS-005 #21）。
pub(crate) fn display_width(text: &str) -> usize {
    use infiltrator_shared::effect_assets::CharWidthClass::*;
    text.chars()
        .map(|ch| match char_width_class(ch) {
            Full => 2,
            Half => 1,
            Zero => 0,
        })
        .sum()
}

/// 输出分区的降级策略（#182）：宽度够就居中，不够就左对齐，永不截断标题。
pub(crate) fn center_or_left(text: &str, available: usize) -> (String, usize) {
    let width = display_width(text);
    if width + 2 >= available {
        return (text.to_string(), 0);
    }
    let left = (available - width) / 2;
    (format!("{}{}", " ".repeat(left), text), left)
}

/// `infiltrator-terminal.toml` 的使用者覆盖（#185）。
///
/// 三个键、全部可省略：`unicode = "auto"|"full"|"simple"`、
/// `animation = true|false`、`color = "auto"|"truecolor"|"ansi256"|"ansi16"`。
/// 配置缺失、不可读或字段非法时整份覆盖按空处理——配置是覆盖，不是依赖。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TerminalOverrides {
    /// 字符集覆盖。
    pub(crate) unicode: Option<String>,
    /// 动效开关（false 时进度区退化为静态文案）。
    pub(crate) animation: Option<bool>,
    /// 色彩档位覆盖。
    pub(crate) color: Option<String>,
}

impl TerminalOverrides {
    /// 从工作目录或其任一上级查找并解析 `infiltrator-terminal.toml`。
    pub(crate) fn discover() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse(&text)
    }

    /// 解析配置文本；非法键值逐项忽略。
    pub(crate) fn parse(text: &str) -> Self {
        let parsed: toml::Value = toml::from_str(text)
            .unwrap_or_else(|_| toml::Value::Table(Map::new()));
        let table = parsed.as_table().cloned().unwrap_or_default();
        Self {
            unicode: table
                .get("unicode")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            animation: table.get("animation").and_then(|value| value.as_bool()),
            color: table
                .get("color")
                .and_then(|value| value.as_str())
                .map(str::to_string),
        }
    }
}

/// 自当前目录向上查找 `infiltrator-terminal.toml`，最深四级。
fn config_path() -> Option<PathBuf> {
    let mut directory = env::current_dir().ok()?;
    for _ in 0..4 {
        let candidate = directory.join("infiltrator-terminal.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !directory.pop() {
            break;
        }
    }
    None
}

#[cfg(test)]
#[path = "terminal_guard_test.rs"]
mod terminal_guard_test;
