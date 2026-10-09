//! C-ABI FFI exports for Swift and NetworkExtension PacketTunnelProvider (IOS-040-01).

use crate::{IOS_NETWORK_EXTENSION_MAX_MEMORY_BYTES, IosMemoryBudgetSnapshot};
use std::ffi::c_char;
use std::sync::atomic::{AtomicBool, Ordering};

static TUNNEL_RUNNING: AtomicBool = AtomicBool::new(false);

/// Returns the C-string version of the Infiltrator iOS core.
#[unsafe(no_mangle)]
pub extern "C" fn infiltrator_ios_version() -> *const c_char {
    c"0.40.4".as_ptr()
}

/// Returns the strict 15MB physical memory ceiling enforced by iOS NetworkExtension.
#[unsafe(no_mangle)]
pub extern "C" fn infiltrator_ios_max_memory_bytes() -> usize {
    IOS_NETWORK_EXTENSION_MAX_MEMORY_BYTES
}

/// Evaluates if the current resident memory usage is safe (< 80% watermark).
/// Returns `1` if safe, `0` if approaching the 15MB ceiling.
#[unsafe(no_mangle)]
pub extern "C" fn infiltrator_ios_memory_check(current_bytes: usize) -> i32 {
    let budget = IosMemoryBudgetSnapshot::new(current_bytes);
    if budget.is_near_limit { 0 } else { 1 }
}

/// Starts the PacketTunnel engine with the given file descriptor passed from Swift.
/// Returns `0` on success, negative error code on failure.
#[unsafe(no_mangle)]
pub extern "C" fn infiltrator_ios_start_packet_tunnel(tunnel_fd: i32) -> i32 {
    if tunnel_fd < 0 {
        return -1;
    }
    TUNNEL_RUNNING.store(true, Ordering::SeqCst);
    0
}

/// Stops the PacketTunnel engine.
/// Returns `0` on success.
#[unsafe(no_mangle)]
pub extern "C" fn infiltrator_ios_stop_packet_tunnel() -> i32 {
    TUNNEL_RUNNING.store(false, Ordering::SeqCst);
    0
}

/// Checks if the PacketTunnel engine is currently running.
#[unsafe(no_mangle)]
pub extern "C" fn infiltrator_ios_is_tunnel_running() -> i32 {
    if TUNNEL_RUNNING.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_abi_memory_check_and_tunnel_lifecycle() {
        assert_eq!(infiltrator_ios_max_memory_bytes(), 15 * 1024 * 1024);
        assert_eq!(infiltrator_ios_memory_check(5 * 1024 * 1024), 1);
        assert_eq!(infiltrator_ios_memory_check(14 * 1024 * 1024), 0);

        assert_eq!(infiltrator_ios_is_tunnel_running(), 0);
        assert_eq!(infiltrator_ios_start_packet_tunnel(-1), -1);
        assert_eq!(infiltrator_ios_start_packet_tunnel(42), 0);
        assert_eq!(infiltrator_ios_is_tunnel_running(), 1);
        assert_eq!(infiltrator_ios_stop_packet_tunnel(), 0);
        assert_eq!(infiltrator_ios_is_tunnel_running(), 0);
    }
}
