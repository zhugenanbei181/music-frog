//! Independently selectable conformance check groups.

/// One cohesive set of adapter checks.
///
/// Groups are explicit so a host can run a focused subset while the whole
/// product harness runs [`ConformanceGroup::ALL`]. Process-dependent groups
/// still require the adapter to support the managed start/stop lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConformanceGroup {
    /// Initial stopped state, start, readiness, running state, stop and the
    /// exact terminal readback.
    Lifecycle,
    /// Codec scope as well as valid, invalid and native-invalid documents.
    Document,
    /// Outbound inventory, selection, observation, details and probe.
    Outbound,
    /// Routing observation plus readback or the typed unsupported rejection.
    Routing,
    /// Stale/foreign scope and stale revision rejection with typed errors.
    Errors,
}

impl ConformanceGroup {
    /// Every group in a deterministic order.
    pub const ALL: [Self; 5] = [
        Self::Lifecycle,
        Self::Document,
        Self::Outbound,
        Self::Routing,
        Self::Errors,
    ];

    /// Returns the stable, machine-readable label of the group.
    pub fn label(self) -> &'static str {
        match self {
            Self::Lifecycle => "lifecycle",
            Self::Document => "document",
            Self::Outbound => "outbound",
            Self::Routing => "routing",
            Self::Errors => "errors",
        }
    }
}
