//! Runtime capability used by the application layer.
//!
//! The application needs a way to serialize asynchronous work and wait for a
//! delay, but it must not choose Tokio, async-std, or any other executor. Host
//! composition roots provide this capability.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// A task that a host runtime can drive to completion.
pub type ApplicationFuture = Pin<Box<dyn Future<Output = ()> + 'static>>;

/// Executor and timer capabilities required by `infiltrator-application`.
///
/// No concrete runtime types cross this boundary. A desktop composition may
/// implement it with Tokio while another host can use a different executor or
/// a native event loop bridge.
pub trait ApplicationRuntime: Send + Sync {
    fn block_on(&self, future: ApplicationFuture);

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_>;
}

/// Runtime-neutral sleep future returned by [`ApplicationRuntime::sleep`].
pub type ApplicationSleep<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
