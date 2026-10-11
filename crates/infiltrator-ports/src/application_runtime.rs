//! Runtime capability used by the application layer.
//!
//! The application needs a way to serialize asynchronous work and wait for a
//! delay, but it must not choose Tokio, async-std, or any other executor. Host
//! composition roots provide this capability.

use std::future::Future;
use std::num::NonZeroU64;
use std::pin::Pin;
use std::time::Duration;

/// A task that a host runtime can drive to completion.
pub type ApplicationFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Identity for the sole admitted background task in one application runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ApplicationTaskId(NonZeroU64);

impl ApplicationTaskId {
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Admission returned by the bounded executor-neutral background scheduler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationTaskAdmission {
    Scheduled {
        task_id: ApplicationTaskId,
    },
    AtCapacity {
        occupying_task_id: ApplicationTaskId,
    },
    Closing,
    Closed,
    WorkerFailed,
    IdExhausted,
}

/// Terminal response from closing the background scheduler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationTaskShutdown {
    Closed,
    AlreadyClosed,
    WorkerFailed,
}

/// Executor and timer capabilities required by `infiltrator-application`.
///
/// No concrete runtime types cross this boundary. A desktop composition may
/// implement it with Tokio while another host can use a different executor or
/// a native event loop bridge.
pub trait ApplicationRuntime: Send + Sync {
    fn block_on(&self, future: ApplicationFuture);

    /// Admit one task into a single-slot background scheduler. Implementations
    /// without background execution return a typed closed result.
    fn spawn_background(&self, _future: ApplicationFuture) -> ApplicationTaskAdmission {
        ApplicationTaskAdmission::Closed
    }

    /// Close the scheduler and wait for its admitted task to leave the slot.
    fn shutdown_background_tasks(&self) -> ApplicationTaskShutdown {
        ApplicationTaskShutdown::AlreadyClosed
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_>;
}

/// Runtime-neutral sleep future returned by [`ApplicationRuntime::sleep`].
pub type ApplicationSleep<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
