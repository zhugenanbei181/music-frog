//! `infiltrator` — command line front end for the MusicFrog Infiltrator
//! engine-neutral Core manager (Mihomo / sing-box). The binary only parses
//! arguments and maps handler outcomes to process exit codes; every capability
//! lives behind the workspace crates (doctor/bootstrap in `infiltrator-core`,
//! kernel in `mihomo-version`, controller API in `mihomo-api`, and so on).

use std::process::exit;
mod color_quantize;
mod commands;
mod context;
mod handlers;
mod output;
mod terminal_capability;

#[cfg(test)]
mod test_support;

use crate::terminal_capability::TerminalEnvironment;
use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = commands::Cli::parse();
    // 终端能力在进入任何交互循环之前协商完成：协商只读终端自报的环境信号，
    // 不触碰终端输入，因此既不会抢占按键，也不会等待应答而卡死。
    let capabilities = terminal_capability::negotiate(&TerminalEnvironment::from_process());
    exit(handlers::run(cli.command, capabilities).await);
}
