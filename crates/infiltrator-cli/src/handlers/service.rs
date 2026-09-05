use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::CoreProcess;
use mihomo_platform::desktop::ProcessCoreController;

use crate::commands::ServiceAction;
use crate::context::Runtime;
use crate::handlers::telemetry;

pub(crate) async fn handle(action: ServiceAction) -> anyhow::Result<()> {
    match action {
        ServiceAction::Start => lifecycle(Lifecycle::Start).await,
        ServiceAction::Stop => lifecycle(Lifecycle::Stop).await,
        ServiceAction::Restart => lifecycle(Lifecycle::Restart).await,
        ServiceAction::Status => lifecycle(Lifecycle::Status).await,
        ServiceAction::Logs { level } => telemetry::logs(level.as_deref()).await,
        ServiceAction::Traffic => telemetry::traffic().await,
        ServiceAction::Memory => telemetry::memory().await,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lifecycle {
    Start,
    Stop,
    Restart,
    Status,
}

async fn lifecycle(action: Lifecycle) -> anyhow::Result<()> {
    let runtime = Runtime::detect().await?;
    if matches!(action, Lifecycle::Start | Lifecycle::Restart) {
        // mihomo-rs parity: `service start` creates the default profile and
        // controller settings on first launch instead of failing.
        runtime
            .doctor_application()
            .bootstrap()
            .await
            .map_err(|failure| anyhow::anyhow!(failure.message))?;
    }
    let controller = build_controller(&runtime).await?;
    let message = run_lifecycle(action, &controller).await?;
    println!("{message}");
    Ok(())
}

/// The production controller: default kernel binary + current profile config,
/// with the pid file living in the resolved home directory.
pub(crate) async fn build_controller(runtime: &Runtime) -> anyhow::Result<ProcessCoreController> {
    let binary = runtime.version_manager()?.get_binary_path(None).await?;
    let config = runtime.config_manager()?.get_current_path().await?;
    Ok(ProcessCoreController::with_home(
        binary,
        config,
        runtime.home().to_path_buf(),
    ))
}

/// Lifecycle state machine over any [`CoreProcess`] so tests can inject a
/// mock instead of a real core process.
pub(crate) async fn run_lifecycle(
    action: Lifecycle,
    controller: &dyn CoreProcess,
) -> anyhow::Result<String> {
    match action {
        Lifecycle::Start => {
            controller
                .start()
                .await
                .map_err(|error| anyhow::anyhow!(error))?;
            Ok("Service started".to_string())
        }
        Lifecycle::Stop => {
            controller
                .stop()
                .await
                .map_err(|error| anyhow::anyhow!(error))?;
            Ok("Service stopped".to_string())
        }
        Lifecycle::Restart => {
            if matches!(
                controller
                    .status()
                    .await
                    .map_err(|error| anyhow::anyhow!(error))?,
                CoreLifecycle::Starting | CoreLifecycle::Ready | CoreLifecycle::Running
            ) {
                controller
                    .stop()
                    .await
                    .map_err(|error| anyhow::anyhow!(error))?;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            controller
                .start()
                .await
                .map_err(|error| anyhow::anyhow!(error))?;
            Ok("Service restarted".to_string())
        }
        Lifecycle::Status => Ok(status_message(
            matches!(
                controller
                    .status()
                    .await
                    .map_err(|error| anyhow::anyhow!(error))?,
                CoreLifecycle::Starting | CoreLifecycle::Ready | CoreLifecycle::Running
            ),
            controller.pid().await,
        )),
    }
}

pub(crate) fn status_message(running: bool, pid: Option<u32>) -> String {
    match (running, pid) {
        (true, Some(pid)) => format!("Service is running (pid {pid})"),
        (true, None) => "Service is running".to_string(),
        (false, _) => "Service is stopped".to_string(),
    }
}

#[cfg(test)]
#[path = "service_test.rs"]
mod service_test;
