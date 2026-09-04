//! Application services for MusicFrog Infiltrator.
//!
//! The application layer is runtime-neutral. Its public surface is expressed
//! in contract values and ports, so a UI or FFI consumer never needs to know
//! about an executor, task handles, or concrete Mihomo clients.

pub mod core_application;
pub mod configuration_application;
pub mod doctor_application;
pub mod overview;
pub mod network_application;
pub mod profile_application;
pub mod proxy_application;
pub mod routing_application;
pub mod settings_application;

use infiltrator_ports::application_runtime::ApplicationRuntime;
use std::future::Future;

/// Drive a typed future through the host-provided runtime without putting a
/// result type into the runtime port. This is used by application workers
/// that run on a dedicated thread and need to synchronously observe an
/// async-port result before publishing a snapshot.
pub(crate) fn run_on_runtime<T, F>(runtime: &dyn ApplicationRuntime, future: F) -> T
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    runtime.block_on(Box::pin(async move {
        let _ = sender.send(future.await);
    }));
    receiver
        .recv()
        .expect("application runtime dropped a completed future")
}

/// Validate a logical routing rule without exposing the domain error type to
/// a surface. The application owns the public error boundary; the pure AST
/// parser remains in `infiltrator-domain`.
pub fn validate_logical_rule_syntax(rule: &str) -> Result<(), String> {
    infiltrator_domain::sub_rules::validate_logical_rule_syntax(rule)
        .map_err(|error| error.to_string())
}
