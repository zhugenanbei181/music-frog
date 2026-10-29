//! `infiltrator` — command line front end for the MusicFrog Infiltrator
//! engine-neutral Core manager (Mihomo / sing-box). The binary only parses
//! arguments and maps handler outcomes to process exit codes; every capability
//! lives behind the workspace crates (doctor/bootstrap in `infiltrator-core`,
//! kernel in `mihomo-version`, controller API in `mihomo-api`, and so on).

#![allow(dead_code)]

use std::process::exit;
mod char_fx;
mod charset;
mod color_quantize;
mod commands;
mod context;
mod handlers;
mod output;
mod terminal_capability;
mod terminal_guard;

#[cfg(test)]
mod test_support;

use crate::terminal_capability::TerminalEnvironment;
use clap::Parser;

#[tokio::main]
async fn main() {
    // 终端输出状态守卫（EFFECTS-045 #181/#184）：正常结束、提前返回与 panic
    // 三条路径都复位颜色、光标与折行，终端永远回到干净状态。
    let _guard = terminal_guard::TerminalGuard::install();
    let cli = commands::Cli::parse();
    // 终端能力在进入任何交互循环之前协商完成：协商只读终端自报的环境信号，
    // 不触碰终端输入，因此既不会抢占按键，也不会等待应答而卡死。
    let environment = TerminalEnvironment::from_process();
    let capabilities = terminal_capability::negotiate(&environment);
    // 表格线框按协商出的字符集档案取字形（EFFECTS-043）。
    output::configure_table_charset(capabilities.charset);
    exit(handlers::run(cli.command, capabilities).await);
}
