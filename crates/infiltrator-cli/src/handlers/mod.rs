mod bootstrap;
mod connection;
mod doctor;
mod kernel;
mod profile;
mod proxy;
mod service;
mod sync;
mod telemetry;

use crate::commands::Commands;
use crate::output;

/// Handler finished successfully with no failures to report.
pub(crate) const EXIT_OK: i32 = 0;
/// The command itself failed (bad environment, controller unreachable, ...).
/// Exit code 1 is reserved for doctor: `doctor::exit_code` maps a report with
/// failing checks to 1.
pub(crate) const EXIT_ERROR: i32 = 2;

/// Run a parsed command and return the process exit code. Handler errors are
/// printed here so `main` can stay a one-liner.
pub async fn run(command: Commands) -> i32 {
    match dispatch(command).await {
        Ok(code) => code,
        Err(err) => {
            output::print_error(&format!("error: {err:#}"));
            EXIT_ERROR
        }
    }
}

async fn dispatch(command: Commands) -> anyhow::Result<i32> {
    match command {
        Commands::Doctor { action } => doctor::handle(action).await,
        Commands::Bootstrap => bootstrap::handle().await.map(|()| EXIT_OK),
        Commands::Kernel { action } => kernel::handle(action).await.map(|()| EXIT_OK),
        Commands::Profile { action } => profile::handle(action).await.map(|()| EXIT_OK),
        Commands::Service { action } => service::handle(action).await.map(|()| EXIT_OK),
        Commands::Proxy { action } => proxy::handle(action).await.map(|()| EXIT_OK),
        Commands::Connection { action } => connection::handle(action).await.map(|()| EXIT_OK),
        Commands::Sync { action } => sync::handle(action).await.map(|()| EXIT_OK),
    }
}
