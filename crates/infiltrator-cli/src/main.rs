//! `infiltrator` — command line front end for the MusicFrog Infiltrator
//! mihomo kernel manager. The binary only parses arguments and maps handler
//! outcomes to process exit codes; every capability lives behind the
//! workspace crates (doctor/bootstrap in `infiltrator-core`, kernel in
//! `mihomo-version`, controller API in `mihomo-api`, and so on).

mod commands;
mod context;
mod handlers;
mod output;

#[cfg(test)]
mod test_support;

use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = commands::Cli::parse();
    std::process::exit(handlers::run(cli.command).await);
}
