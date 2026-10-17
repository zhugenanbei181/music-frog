//! Replaying state stream plus the bounded wait helper used by `wait_ready`.
//!
//! `futures-channel` keeps the fan-out executor-neutral: a subscriber may be
//! polled by any runtime, or drained synchronously through `try_next`.

use super::{LurkerRuntimeState, LurkerRuntimeStateStream};
use futures_core::Stream;
use futures_timer::Delay;
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

impl LurkerRuntimeStateStream {
    /// Await the next state. The current state is always delivered first for a
    /// freshly created stream.
    pub async fn next(&mut self) -> Option<LurkerRuntimeState> {
        poll_fn(|context| Pin::new(&mut self.receiver).poll_next(context)).await
    }
}

impl Stream for LurkerRuntimeStateStream {
    type Item = LurkerRuntimeState;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(context)
    }
}

/// The outcome of waiting one state or one deadline, whichever comes first.
pub(super) enum StreamWait {
    State(Box<LurkerRuntimeState>),
    Timeout,
    Closed,
}

/// Await the next state or the deadline. The state channel wins when both are
/// ready in the same poll, so a terminal update is never lost to a race.
pub(super) async fn next_before(
    stream: &mut LurkerRuntimeStateStream,
    timeout: Duration,
) -> StreamWait {
    let mut delay = Delay::new(timeout);
    poll_fn(|context| {
        if let Poll::Ready(item) = Pin::new(&mut stream.receiver).poll_next(context) {
            return Poll::Ready(match item {
                Some(state) => StreamWait::State(Box::new(state)),
                None => StreamWait::Closed,
            });
        }
        if Pin::new(&mut delay).poll(context).is_ready() {
            return Poll::Ready(StreamWait::Timeout);
        }
        Poll::Pending
    })
    .await
}
