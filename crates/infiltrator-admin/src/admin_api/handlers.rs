//! Admin API handlers, grouped by business domain under `handlers/`:
//!
//! - [`profiles`]: profile / subscription management
//! - [`runtime`]: runtime lifecycle, connections, logs/traffic/memory/ip
//! - [`proxies`]: proxy listing, mode/select, delay testing
//! - [`config`]: static config read-write (dns, fake-ip, providers,
//!   sniffer, rules, tun)
//! - [`settings`]: app settings, editor integration, WebDAV sync
//! - [`kernel`]: mihomo core version management
//! - [`system`]: capabilities, rebuild status, admin event stream
//!
//! This file is the module root: submodules plus the shared rebuild /
//! logging glue. The router in `super` consumes every handler through the
//! re-exports below, so route paths, methods, and handler names are
//! unchanged.

use std::sync::Arc;

use axum::{body::Body, http::Request, middleware::Next, response::Response};
use log::{info, warn};

use crate::admin_api::state::{AdminApiContext, RebuildStatus};

mod config;
mod kernel;
mod profiles;
mod proxies;
mod runtime;
mod settings;
mod system;

pub use config::*;
pub use kernel::*;
pub use profiles::*;
pub use proxies::*;
pub use runtime::*;
pub use settings::*;
pub use system::*;

pub async fn log_admin_request(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let status = response.status();
    let elapsed = start.elapsed();
    if status.is_client_error() || status.is_server_error() {
        warn!(
            "admin api {} {}{} -> {} ({}ms)",
            method,
            path,
            query,
            status.as_u16(),
            elapsed.as_millis()
        );
    } else {
        info!(
            "admin api {} {}{} -> {} ({}ms)",
            method,
            path,
            query,
            status.as_u16(),
            elapsed.as_millis()
        );
    }
    response
}

fn schedule_rebuild<C: AdminApiContext>(
    ctx: &C,
    rebuild_status: &Arc<RebuildStatus>,
    reason: &str,
) {
    let ctx = ctx.clone();
    let reason = reason.to_string();
    let rebuild_status = Arc::clone(rebuild_status);
    info!("schedule runtime rebuild: {reason}");
    rebuild_status.mark_start(&reason);
    tokio::spawn(async move {
        if let Err(err) = ctx.rebuild_runtime().await {
            warn!("runtime rebuild failed ({reason}): {err}");
            rebuild_status.mark_error(err.to_string());
        } else {
            info!("runtime rebuild completed ({reason})");
            rebuild_status.mark_success();
        }
    });
}

/// Config-level rebuild: same status bookkeeping as [`schedule_rebuild`],
/// but asks the host for a session-level core restart instead of a full
/// runtime re-bootstrap.
fn schedule_core_restart<C: AdminApiContext>(
    ctx: &C,
    rebuild_status: &Arc<RebuildStatus>,
    reason: &str,
) {
    let ctx = ctx.clone();
    let reason = reason.to_string();
    let rebuild_status = Arc::clone(rebuild_status);
    info!("schedule core restart: {reason}");
    rebuild_status.mark_start(&reason);
    tokio::spawn(async move {
        if let Err(err) = ctx.restart_core().await {
            warn!("core restart failed ({reason}): {err}");
            rebuild_status.mark_error(err.to_string());
        } else {
            info!("core restart completed ({reason})");
            rebuild_status.mark_success();
        }
    });
}

#[cfg(test)]
#[path = "handlers_test.rs"]
mod handlers_test;
