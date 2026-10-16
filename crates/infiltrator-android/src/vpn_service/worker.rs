//! Typed lifecycle and readiness signaling for the Android tun2proxy worker.

use futures_util::future::poll_fn;
use std::fmt::{self, Display, Formatter};
use std::mem;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::{
    future::Future,
    task::Poll,
    thread,
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum WorkerStartReceipt {
    Started,
    Failed(WorkerStartFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum WorkerStartFailure {
    TunnelDeviceCreation(String),
    RunLoopInitialization(String),
    ExitedBeforeReady,
}

pub(super) fn report_start_failure(
    receipt_sender: oneshot::Sender<WorkerStartReceipt>,
    failure: WorkerStartFailure,
) {
    let _ = receipt_sender.send(WorkerStartReceipt::Failed(failure));
}

impl Display for WorkerStartFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TunnelDeviceCreation(message) => {
                write!(formatter, "failed to create Android TUN device: {message}")
            }
            Self::RunLoopInitialization(message) => {
                write!(
                    formatter,
                    "tun2proxy run loop failed before readiness: {message}"
                )
            }
            Self::ExitedBeforeReady => {
                formatter.write_str("tun2proxy run loop exited before its first pending poll")
            }
        }
    }
}

/// Report readiness only after the supplied future has completed its first
/// poll and yielded Pending. An early error or return is a failed start.
pub(super) fn report_first_pending<F>(
    future: F,
    receipt_sender: oneshot::Sender<WorkerStartReceipt>,
) -> impl Future<Output = Result<(), String>>
where
    F: Future<Output = Result<(), String>>,
{
    let mut future = Box::pin(future);
    let mut receipt_sender = Some(receipt_sender);

    poll_fn(move |context| match future.as_mut().poll(context) {
        Poll::Pending => {
            if let Some(sender) = receipt_sender.take() {
                let _ = sender.send(WorkerStartReceipt::Started);
            }
            Poll::Pending
        }
        Poll::Ready(result) => {
            if let Some(sender) = receipt_sender.take() {
                let failure = match result.as_ref() {
                    Ok(()) => WorkerStartFailure::ExitedBeforeReady,
                    Err(message) => WorkerStartFailure::RunLoopInitialization(message.clone()),
                };
                let _ = sender.send(WorkerStartReceipt::Failed(failure));
            }
            Poll::Ready(result)
        }
    })
}

pub(super) struct WorkerStartPermit {
    pub generation: u64,
    pub stop_receiver: oneshot::Receiver<()>,
    #[cfg(target_os = "android")]
    pub receipt_sender: oneshot::Sender<WorkerStartReceipt>,
    #[cfg(target_os = "android")]
    pub receipt_receiver: oneshot::Receiver<WorkerStartReceipt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WorkerActivity {
    Starting,
    Ready,
    Stopping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WorkerControlError {
    Busy(WorkerActivity),
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WorkerReadyOutcome {
    Published,
    StopInProgress,
    StaleGeneration,
}

pub(super) enum WorkerStopRequest {
    Signal(oneshot::Sender<()>),
    AlreadyStopping,
    AlreadyStopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkerStopOutcome {
    Stopped,
    TimedOut,
    Unavailable,
}

const WORKER_STOP_POLL_INTERVAL: Duration = Duration::from_millis(10);

enum WorkerState {
    Stopped {
        generation: u64,
    },
    Starting {
        generation: u64,
        stop_sender: oneshot::Sender<()>,
    },
    Ready {
        generation: u64,
        stop_sender: oneshot::Sender<()>,
    },
    Stopping {
        generation: u64,
    },
}

pub(super) struct WorkerLifecycle {
    next_generation: u64,
    state: WorkerState,
}

impl Default for WorkerLifecycle {
    fn default() -> Self {
        Self {
            next_generation: 0,
            state: WorkerState::Stopped { generation: 0 },
        }
    }
}

impl WorkerLifecycle {
    pub(super) fn begin(&mut self) -> Result<WorkerStartPermit, WorkerControlError> {
        match &self.state {
            WorkerState::Starting { .. } => {
                return Err(WorkerControlError::Busy(WorkerActivity::Starting));
            }
            WorkerState::Ready { .. } => {
                return Err(WorkerControlError::Busy(WorkerActivity::Ready));
            }
            WorkerState::Stopping { .. } => {
                return Err(WorkerControlError::Busy(WorkerActivity::Stopping));
            }
            WorkerState::Stopped { .. } => {}
        }

        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(WorkerControlError::Unavailable)?;
        let generation = self.next_generation;
        let (stop_sender, stop_receiver) = oneshot::channel();
        #[cfg(target_os = "android")]
        let (receipt_sender, receipt_receiver) = oneshot::channel();
        self.state = WorkerState::Starting {
            generation,
            stop_sender,
        };
        Ok(WorkerStartPermit {
            generation,
            stop_receiver,
            #[cfg(target_os = "android")]
            receipt_sender,
            #[cfg(target_os = "android")]
            receipt_receiver,
        })
    }

    pub(super) fn publish_ready(&mut self, generation: u64) -> WorkerReadyOutcome {
        let state = mem::replace(&mut self.state, WorkerState::Stopped { generation: 0 });
        match state {
            WorkerState::Starting {
                generation: current,
                stop_sender,
            } if current == generation => {
                self.state = WorkerState::Ready {
                    generation: current,
                    stop_sender,
                };
                WorkerReadyOutcome::Published
            }
            WorkerState::Stopping {
                generation: current,
            } if current == generation => {
                self.state = WorkerState::Stopping {
                    generation: current,
                };
                WorkerReadyOutcome::StopInProgress
            }
            other => {
                self.state = other;
                WorkerReadyOutcome::StaleGeneration
            }
        }
    }

    pub(super) fn request_stop(&mut self) -> WorkerStopRequest {
        let state = mem::replace(&mut self.state, WorkerState::Stopped { generation: 0 });
        match state {
            WorkerState::Starting {
                generation,
                stop_sender,
            }
            | WorkerState::Ready {
                generation,
                stop_sender,
            } => {
                self.state = WorkerState::Stopping { generation };
                WorkerStopRequest::Signal(stop_sender)
            }
            WorkerState::Stopping { generation } => {
                self.state = WorkerState::Stopping { generation };
                WorkerStopRequest::AlreadyStopping
            }
            WorkerState::Stopped { generation } => {
                self.state = WorkerState::Stopped { generation };
                WorkerStopRequest::AlreadyStopped
            }
        }
    }

    pub(super) fn finish(&mut self, generation: u64) {
        let state = mem::replace(&mut self.state, WorkerState::Stopped { generation: 0 });
        self.state = match state {
            WorkerState::Starting {
                generation: current,
                ..
            }
            | WorkerState::Ready {
                generation: current,
                ..
            }
            | WorkerState::Stopping {
                generation: current,
            } if current == generation => WorkerState::Stopped {
                generation: current,
            },
            other => other,
        };
    }

    #[cfg(any(target_os = "android", test))]
    fn is_stopped(&self) -> bool {
        matches!(&self.state, WorkerState::Stopped { .. })
    }

    #[cfg(test)]
    fn activity(&self) -> Option<WorkerActivity> {
        match &self.state {
            WorkerState::Stopped { .. } => None,
            WorkerState::Starting { .. } => Some(WorkerActivity::Starting),
            WorkerState::Ready { .. } => Some(WorkerActivity::Ready),
            WorkerState::Stopping { .. } => Some(WorkerActivity::Stopping),
        }
    }
}

#[cfg(any(target_os = "android", test))]
static WORKER_LIFECYCLE: OnceLock<Mutex<WorkerLifecycle>> =
    OnceLock::new();

#[cfg(any(target_os = "android", test))]
fn shared_lifecycle() -> &'static Mutex<WorkerLifecycle> {
    WORKER_LIFECYCLE.get_or_init(|| Mutex::new(WorkerLifecycle::default()))
}

#[cfg(target_os = "android")]
pub(super) fn begin_worker() -> Result<WorkerStartPermit, WorkerControlError> {
    shared_lifecycle()
        .lock()
        .map_err(|_| WorkerControlError::Unavailable)?
        .begin()
}

#[cfg(target_os = "android")]
pub(super) fn publish_worker_ready(
    generation: u64,
) -> Result<WorkerReadyOutcome, WorkerControlError> {
    shared_lifecycle()
        .lock()
        .map(|mut lifecycle| lifecycle.publish_ready(generation))
        .map_err(|_| WorkerControlError::Unavailable)
}

#[cfg(target_os = "android")]
pub(super) fn finish_worker(generation: u64) -> Result<(), WorkerControlError> {
    shared_lifecycle()
        .lock()
        .map(|mut lifecycle| lifecycle.finish(generation))
        .map_err(|_| WorkerControlError::Unavailable)
}

fn wait_for_worker_exit(
    timeout: Duration,
    mut read_stopped: impl FnMut() -> Result<bool, ()>,
) -> WorkerStopOutcome {
    let deadline = Instant::now() + timeout;
    loop {
        match read_stopped() {
            Ok(true) => return WorkerStopOutcome::Stopped,
            Ok(false) => {}
            Err(()) => return WorkerStopOutcome::Unavailable,
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return WorkerStopOutcome::TimedOut;
        }
        thread::sleep(WORKER_STOP_POLL_INTERVAL.min(remaining));
    }
}

#[cfg(any(target_os = "android", test))]
pub(super) fn stop_worker_and_wait(timeout: Duration) -> WorkerStopOutcome {
    let request = match shared_lifecycle().lock() {
        Ok(mut lifecycle) => lifecycle.request_stop(),
        Err(_) => return WorkerStopOutcome::Unavailable,
    };
    match request {
        WorkerStopRequest::Signal(sender) => {
            let _ = sender.send(());
        }
        WorkerStopRequest::AlreadyStopping | WorkerStopRequest::AlreadyStopped => {}
    }
    wait_for_worker_exit(timeout, || {
        shared_lifecycle()
            .lock()
            .map(|lifecycle| lifecycle.is_stopped())
            .map_err(|_| ())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::future::poll_fn;
    use std::task::Poll;

    #[test]
    fn worker_stop_readback_distinguishes_stopped_timeout_and_unavailable() {
        assert_eq!(
            WorkerStopOutcome::Stopped,
            wait_for_worker_exit(Duration::from_millis(5), || Ok(true)),
        );
        assert_eq!(
            WorkerStopOutcome::TimedOut,
            wait_for_worker_exit(Duration::ZERO, || Ok(false)),
        );
        assert_eq!(
            WorkerStopOutcome::Unavailable,
            wait_for_worker_exit(Duration::from_millis(5), || Err(())),
        );
    }

    #[tokio::test]
    async fn readiness_is_sent_after_first_pending_poll_and_before_worker_completion() {
        let (receipt_sender, receipt_receiver) = oneshot::channel();
        let (finish_sender, mut finish_receiver) = oneshot::channel();
        let worker = tokio::spawn(report_first_pending(
            poll_fn(
                move |context| match Pin::new(&mut finish_receiver).poll(context) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
                    Poll::Ready(Err(_)) => Poll::Ready(Err("finish signal dropped".to_owned())),
                },
            ),
            receipt_sender,
        ));

        assert_eq!(receipt_receiver.await, Ok(WorkerStartReceipt::Started));
        finish_sender.send(()).expect("release the ready worker");
        assert_eq!(worker.await.expect("worker task joins"), Ok(()));
    }

    #[tokio::test]
    async fn an_error_before_first_pending_is_a_failed_start_receipt() {
        let (receipt_sender, receipt_receiver) = oneshot::channel();
        let result = report_first_pending(
            async { Err("TUN initialization failed".to_owned()) },
            receipt_sender,
        )
        .await;

        assert_eq!(
            receipt_receiver.await,
            Ok(WorkerStartReceipt::Failed(
                WorkerStartFailure::RunLoopInitialization("TUN initialization failed".to_owned())
            )),
        );
        assert_eq!(result, Err("TUN initialization failed".to_owned()));
    }

    #[tokio::test]
    async fn a_successful_exit_before_pending_is_not_reported_as_started() {
        let (receipt_sender, receipt_receiver) = oneshot::channel();
        let result = report_first_pending(async { Ok(()) }, receipt_sender).await;

        assert_eq!(
            receipt_receiver.await,
            Ok(WorkerStartReceipt::Failed(
                WorkerStartFailure::ExitedBeforeReady
            )),
        );
        assert_eq!(result, Ok(()));
    }

    #[tokio::test]
    async fn tunnel_device_creation_failure_has_a_typed_failed_receipt() {
        let (receipt_sender, receipt_receiver) = oneshot::channel();
        report_start_failure(
            receipt_sender,
            WorkerStartFailure::TunnelDeviceCreation("bad TUN descriptor".to_owned()),
        );

        assert_eq!(
            receipt_receiver.await,
            Ok(WorkerStartReceipt::Failed(
                WorkerStartFailure::TunnelDeviceCreation("bad TUN descriptor".to_owned())
            )),
        );
    }

    #[tokio::test]
    async fn worker_lifecycle_fences_ready_stop_and_exit_by_generation() {
        let mut lifecycle = WorkerLifecycle::default();
        let first = lifecycle.begin().expect("first worker admission");
        assert_eq!(lifecycle.activity(), Some(WorkerActivity::Starting));
        assert_eq!(
            lifecycle.publish_ready(first.generation),
            WorkerReadyOutcome::Published
        );
        assert_eq!(lifecycle.activity(), Some(WorkerActivity::Ready));
        match lifecycle.request_stop() {
            WorkerStopRequest::Signal(sender) => sender.send(()).expect("worker stop signal"),
            WorkerStopRequest::AlreadyStopping | WorkerStopRequest::AlreadyStopped => {
                panic!("a live worker must accept its first stop request")
            }
        }
        assert_eq!(first.stop_receiver.await, Ok(()));
        assert_eq!(
            lifecycle.publish_ready(first.generation),
            WorkerReadyOutcome::StopInProgress
        );
        lifecycle.finish(first.generation);
        assert_eq!(lifecycle.activity(), None);

        let second = lifecycle.begin().expect("worker restarts after prior exit");
        assert!(second.generation > first.generation);
        lifecycle.finish(first.generation);
        assert_eq!(lifecycle.activity(), Some(WorkerActivity::Starting));
        assert_eq!(
            lifecycle.publish_ready(second.generation),
            WorkerReadyOutcome::Published
        );
    }
}
