//! Process cleanup and result combination for the switch kit.

use super::super::failure::{ConformanceCheck, ConformanceFailure};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::CoreProcess;
use std::sync::Arc;

pub(super) async fn cleanup_processes(
    processes: &[Arc<dyn CoreProcess>],
) -> Result<(), ConformanceFailure> {
    let mut failures = Vec::new();
    for process in processes {
        if let Err(failure) = cleanup_process(process.as_ref()).await {
            failures.push(failure);
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(ConformanceFailure::state(
            ConformanceCheck::SwitchCleanup,
            "every conformance process stops",
            "cleanup left a live process",
            failures.join("; "),
        ))
    }
}

async fn cleanup_process(process: &dyn CoreProcess) -> Result<(), String> {
    let label = process
        .core_identity()
        .map(|identity| identity.engine.as_str().to_owned())
        .unwrap_or_else(|| "unknown".to_owned());
    match process.status().await {
        Ok(CoreLifecycle::Stopped) => {}
        Ok(_) => {
            process
                .stop()
                .await
                .map_err(|error| format!("{label} cleanup stop failed: {error}"))?;
            match process.status().await {
                Ok(CoreLifecycle::Stopped) => {}
                Ok(other) => return Err(format!("{label} cleanup left lifecycle {other:?}")),
                Err(error) => {
                    return Err(format!("{label} cleanup state read failed: {error}"));
                }
            }
        }
        Err(error) => return Err(format!("{label} cleanup state read failed: {error}")),
    }
    match process.pid().await {
        None => Ok(()),
        Some(pid) => Err(format!("{label} cleanup left pid {pid}")),
    }
}

pub(super) fn combine_switch_result(
    result: Result<(), ConformanceFailure>,
    cleanup: Result<(), ConformanceFailure>,
) -> Result<(), ConformanceFailure> {
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(failure), Ok(())) => Err(failure),
        (Ok(()), Err(cleanup)) => Err(cleanup),
        (Err(failure), Err(cleanup)) => Err(failure.with_message_suffix(&cleanup.to_string())),
    }
}
