//! Desktop process-exit cleanup registration.
//!
//! This is a host composition concern. UI crates can request a normal cleanup
//! through `run_now`, but they do not own signal handlers or OS commands.

use mihomo_platform::crash_reporter::{CleanExitHook, TerminationHandler};

pub fn install() -> anyhow::Result<TerminationHandler> {
    CleanExitHook::register_proxy_restore(|| {
        let _ = crate::proxy::apply_system_proxy(None);
    });
    CleanExitHook::register_tun_route_restore(|| {
        let _ = crate::tun_service::TunServiceManager::stop_service();
    });
    CleanExitHook::install_termination_handlers()
}

pub fn run_now() {
    CleanExitHook::run_emergency_cleanup();
}
