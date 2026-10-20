//! The deterministic fault model of the headless sandbox.
//!
//! Every fault is a *typed* value and every fault fires a fixed number of
//! times. The sandbox never uses a random number generator and never waits
//! for a timeout to decide an outcome, so the same run always produces the
//! same typed failure and the same recovery.

/// One deterministic fault the sandbox injects into the neutral runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The controller endpoint stops answering: the readiness probe reports a
    /// network error.
    NetworkFlap,
    /// The Core process dies while the runtime prepares the start.
    KernelProcessDeath,
    /// Another owner already holds the store write fence.
    LockContention,
    /// The profile document on disk is corrupted.
    CorruptedConfiguration,
}

/// The fault budget of one scenario: which fault fires and how often.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaultPlan {
    fault: Fault,
    remaining: u32,
}

impl FaultPlan {
    /// Creates a plan that fires `fault` `remaining` times.
    pub fn new(fault: Fault, remaining: u32) -> Self {
        Self { fault, remaining }
    }

    /// The fault this plan injects.
    pub fn fault(&self) -> Fault {
        self.fault
    }

    /// Consumes one occurrence of `fault`.
    ///
    /// Returns `false` when the plan injects another fault or when its budget
    /// is exhausted, so a fault never fires on the wrong adapter.
    pub fn fire(&mut self, fault: Fault) -> bool {
        if self.fault != fault || self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }

    /// Whether the plan still has an occurrence left.
    pub fn is_pending(&self) -> bool {
        self.remaining > 0
    }
}
