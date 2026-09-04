//! Pure policy for how a validated profile change becomes live.

/// How the running core should pick up a new configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyStrategy {
    /// Try the controller hot-reload first and keep the process (and its
    /// connections) alive; the application adapter may fall back to a
    /// restart if the reload is rejected or health checks fail.
    PreferReload,
    /// Always restart the process. Required for changes hot-reload does not
    /// apply reliably, such as TUN devices, DNS stacks, and listeners.
    AlwaysRestart,
}
