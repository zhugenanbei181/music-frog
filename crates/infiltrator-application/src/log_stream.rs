//! Application-owned stream driver with an injected executor and weak lifecycle observation.
use crate::log_application::LogApplication;
use futures_util::future::{AbortHandle, Abortable};
use futures_util::{FutureExt, StreamExt, select};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::LogSession;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_ports::application_runtime::ApplicationRuntime;
use infiltrator_ports::runtime_gateway::{RuntimeGateway, RuntimeStreamEvent};
use std::sync::Arc;
use std::thread::{Builder, JoinHandle, current};
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
struct LogDriver {
    logs: LogApplication,
    gateway: Arc<dyn RuntimeGateway>,
    runtime: Arc<dyn ApplicationRuntime>,
    observe: Box<dyn Fn() -> Option<CoreLifecycleSnapshot> + Send + Sync>,
}
impl LogDriver {
    fn session(&self) -> Option<LogSession> {
        let core = (self.observe)()?;
        core.session_token
            .filter(|token| {
                token.is_valid()
                    && matches!(
                        core.lifecycle,
                        CoreLifecycle::Running | CoreLifecycle::Ready
                    )
            })
            .map(|token| LogSession {
                generation: core.generation,
                token,
            })
    }
    async fn wait(&self, session: LogSession, duration: Duration) {
        let delay = self.runtime.sleep(duration).fuse();
        futures_util::pin_mut!(delay);
        while self.session() == Some(session) {
            let tick = self.runtime.sleep(POLL_INTERVAL).fuse();
            futures_util::pin_mut!(tick);
            select! { _ = delay => return, _ = tick => {} }
        }
    }
    async fn run(self) {
        while (self.observe)().is_some() {
            let session = self.session();
            if !self.logs.bind(session) {
                continue;
            }
            let Some(session) = session else {
                self.runtime.sleep(POLL_INTERVAL).await;
                continue;
            };
            let opening = self.gateway.stream_logs(Some("debug".into())).fuse();
            futures_util::pin_mut!(opening);
            let opened = loop {
                let tick = self.runtime.sleep(POLL_INTERVAL).fuse();
                futures_util::pin_mut!(tick);
                select! { result = opening => break Some(result), _ = tick => {} }
                if self.session() != Some(session) {
                    break None;
                }
            };
            if self.session() != Some(session) {
                continue;
            }
            let Some(opened) = opened else { continue };
            let mut stream = match opened {
                Ok(stream) => stream,
                Err(error) => {
                    let failure = Failure::from(error);
                    let retryable = failure.retryable;
                    self.logs.fail(session, failure);
                    if retryable {
                        self.wait(session, Duration::from_secs(1)).await;
                    } else {
                        while self.session() == Some(session) {
                            self.runtime.sleep(POLL_INTERVAL).await;
                        }
                    }
                    continue;
                }
            };
            let mut permanent_failure = false;
            loop {
                let event = stream.next().fuse();
                let tick = self.runtime.sleep(POLL_INTERVAL).fuse();
                futures_util::pin_mut!(event, tick);
                let next = select! { item = event => Some(item), _ = tick => None };
                if self.session() != Some(session) {
                    break;
                }
                match next {
                    Some(Some(event)) => {
                        permanent_failure = matches!(&event, RuntimeStreamEvent::Failed(failure) if !failure.retryable);
                        self.logs.ingest(session, event);
                        if permanent_failure {
                            break;
                        }
                    }
                    Some(None) => {
                        self.logs.fail(
                            session,
                            Failure::new(ErrorCode::Network, "controller log stream ended", true),
                        );
                        break;
                    }
                    None => {}
                }
            }
            drop(stream);
            if permanent_failure {
                while self.session() == Some(session) {
                    self.runtime.sleep(POLL_INTERVAL).await;
                }
            } else {
                self.wait(session, Duration::from_millis(100)).await;
            }
        }
    }
}

pub struct LogPump {
    abort: AbortHandle,
    worker: Option<JoinHandle<()>>,
}
impl LogPump {
    pub fn spawn(
        logs: LogApplication,
        gateway: Arc<dyn RuntimeGateway>,
        runtime: Arc<dyn ApplicationRuntime>,
        observe: impl Fn() -> Option<CoreLifecycleSnapshot> + Send + Sync + 'static,
    ) -> Result<Self, Failure> {
        let (abort, registration) = AbortHandle::new_pair();
        let driver = LogDriver {
            logs,
            gateway,
            runtime: runtime.clone(),
            observe: Box::new(observe),
        };
        let worker = Builder::new()
            .name("musicfrog-logs".into())
            .spawn(move || {
                runtime.block_on(Box::pin(async move {
                    let _ = Abortable::new(driver.run(), registration).await;
                }));
            })
            .map_err(|error| Failure::new(ErrorCode::Internal, error.to_string(), true))?;
        Ok(Self {
            abort,
            worker: Some(worker),
        })
    }
}
impl Drop for LogPump {
    fn drop(&mut self) {
        self.abort.abort();
        if let Some(worker) = self.worker.take()
            && worker.thread().id() != current().id()
        {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
#[path = "log_stream_tests.rs"]
mod tests;
