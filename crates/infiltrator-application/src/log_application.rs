//! Bounded, session-fenced raw controller log facts. Readers never consume the stream.
use infiltrator_contract::logs::{
    LogBindingSnapshot, LogFactsSnapshot, LogLevel, LogObservationSnapshot, LogSession,
    LogStreamState, RawLogRecord,
};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_ports::runtime_stream::{RuntimeStreamEvent, RuntimeStreamTerminal};
use lurker_core_contract::error::{ErrorCode, Failure};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub const LOG_CAPACITY: usize = 500;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogBindingIntent {
    Bind(LogSession),
    Unbind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogBindingOutcome {
    Bound,
    CoreOwnerConfirmed,
    RejectedByCoreOwner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogIngestOutcome {
    Applied,
    RejectedForeignSession,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum UnobservedLogStream {
    #[default]
    Idle,
    Connecting,
    Reconnecting(Failure),
    Failed(Failure),
    Unsupported(Failure),
    Cancelled,
}

impl UnobservedLogStream {
    fn stream_state(&self) -> LogStreamState {
        match self {
            Self::Idle => LogStreamState::Idle,
            Self::Connecting => LogStreamState::Connecting,
            Self::Reconnecting(failure) => LogStreamState::Reconnecting(failure.clone()),
            Self::Failed(failure) => LogStreamState::Failed(failure.clone()),
            Self::Unsupported(failure) => LogStreamState::Unsupported(failure.clone()),
            Self::Cancelled => LogStreamState::Cancelled,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LogBuffer {
    Unobserved {
        stream: UnobservedLogStream,
    },
    Observed {
        session: LogSession,
        stream: LogStreamState,
        records: VecDeque<RawLogRecord>,
    },
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::Unobserved {
            stream: UnobservedLogStream::Idle,
        }
    }
}

impl LogBuffer {
    fn session(&self) -> Option<&LogSession> {
        match self {
            Self::Unobserved { .. } => None,
            Self::Observed { session, .. } => Some(session),
        }
    }

    fn set_non_live_stream(&mut self, next: UnobservedLogStream) {
        match self {
            Self::Unobserved { stream } => *stream = next,
            Self::Observed { stream, .. } => *stream = next.stream_state(),
        }
    }

    fn connect(&mut self, session: LogSession) {
        match self {
            Self::Observed {
                session: observed,
                stream,
                ..
            } if *observed == session => *stream = LogStreamState::Live,
            _ => {
                *self = Self::Observed {
                    session,
                    stream: LogStreamState::Live,
                    records: VecDeque::new(),
                };
            }
        }
    }

    fn push(&mut self, session: LogSession, record: RawLogRecord) {
        match self {
            Self::Observed {
                session: observed,
                stream,
                records,
            } if *observed == session => {
                *stream = LogStreamState::Live;
                records.push_back(record);
                while records.len() > LOG_CAPACITY {
                    records.pop_front();
                }
            }
            _ => {
                *self = Self::Observed {
                    session,
                    stream: LogStreamState::Live,
                    records: VecDeque::from([record]),
                };
            }
        }
    }

    fn clear_records(&mut self) {
        if let Self::Observed { records, .. } = self {
            records.clear();
        }
    }

    fn snapshot(&self) -> LogObservationSnapshot {
        match self {
            Self::Unobserved { stream } => match stream {
                UnobservedLogStream::Idle => LogObservationSnapshot::Unobserved,
                UnobservedLogStream::Connecting => LogObservationSnapshot::Connecting,
                UnobservedLogStream::Reconnecting(failure) => {
                    LogObservationSnapshot::Reconnecting(failure.clone())
                }
                UnobservedLogStream::Failed(failure) => {
                    LogObservationSnapshot::Failed(failure.clone())
                }
                UnobservedLogStream::Unsupported(failure) => {
                    LogObservationSnapshot::Unsupported(failure.clone())
                }
                UnobservedLogStream::Cancelled => LogObservationSnapshot::Cancelled,
            },
            Self::Observed {
                session,
                stream,
                records,
            } => LogObservationSnapshot::Observed {
                session: session.clone(),
                stream: stream.clone(),
                records: records.iter().cloned().collect(),
            },
        }
    }
}

#[derive(Default)]
struct LogState {
    binding: LogBindingSnapshot,
    buffer: LogBuffer,
    next_id: u64,
    filter: Option<LogLevel>,
}

#[derive(Clone, Default)]
pub struct LogApplication {
    state: Arc<Mutex<LogState>>,
}

impl LogApplication {
    pub(crate) fn same_owner(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }

    pub(crate) fn has_export_scope(&self, session: &LogSession) -> bool {
        self.state.lock().expect("log facts").buffer.session() == Some(session)
    }

    /// The lifecycle owner retires old facts synchronously, before publishing
    /// the new Core snapshot. A stream worker must not rebind an older scope.
    pub fn observe_core(&self, core: &CoreSnapshot) {
        let mut state = self.state.lock().expect("log facts");
        let scope = core.scope().and_then(LogSession::from_core_scope);
        let active = matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        )
        .then_some(scope.clone())
        .flatten();

        if state.buffer.session().is_some_and(|observed| {
            observed.generation.value() != core.generation
                || observed.identity() != core.identity.as_ref()
                || scope.as_ref().is_some_and(|scope| scope != observed)
        }) {
            state.buffer = LogBuffer::default();
        }

        let next_binding = active
            .clone()
            .map(LogBindingSnapshot::CoreBound)
            .unwrap_or(LogBindingSnapshot::CoreUnbound);
        if state.binding != next_binding {
            state.binding = next_binding;
            state.buffer.set_non_live_stream(if active.is_some() {
                UnobservedLogStream::Connecting
            } else {
                UnobservedLogStream::Idle
            });
        }

        if let Some(session) = active
            && state
                .buffer
                .session()
                .is_some_and(|observed| observed != &session)
        {
            state.buffer = LogBuffer::Unobserved {
                stream: UnobservedLogStream::Connecting,
            };
        }
    }

    /// Exports every observed original record, independently of display filtering.
    pub fn export_records(&self) -> Result<(LogSession, Vec<(u64, String)>), Failure> {
        let state = self.state.lock().expect("log facts");
        let (session, records) = match &state.buffer {
            LogBuffer::Observed {
                session, records, ..
            } => (session.clone(), records),
            LogBuffer::Unobserved { .. } => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "No observed controller log buffer to export",
                    true,
                ));
            }
        };
        Ok((
            session,
            records
                .iter()
                .map(|record| (record.id, record.raw.clone()))
                .collect(),
        ))
    }

    pub fn bind(&self, intent: LogBindingIntent) -> LogBindingOutcome {
        let mut state = self.state.lock().expect("log facts");
        if matches!(
            &state.binding,
            LogBindingSnapshot::CoreUnbound | LogBindingSnapshot::CoreBound(_)
        ) {
            return match (&state.binding, &intent) {
                (LogBindingSnapshot::CoreUnbound, LogBindingIntent::Unbind) => {
                    LogBindingOutcome::CoreOwnerConfirmed
                }
                (LogBindingSnapshot::CoreBound(current), LogBindingIntent::Bind(requested))
                    if current == requested =>
                {
                    LogBindingOutcome::CoreOwnerConfirmed
                }
                _ => LogBindingOutcome::RejectedByCoreOwner,
            };
        }

        let next_binding = match &intent {
            LogBindingIntent::Bind(session) => LogBindingSnapshot::StandaloneBound(session.clone()),
            LogBindingIntent::Unbind => LogBindingSnapshot::StandaloneUnbound,
        };
        if state.binding == next_binding {
            return LogBindingOutcome::Bound;
        }

        state.binding = next_binding;
        match intent {
            LogBindingIntent::Bind(session) => {
                state
                    .buffer
                    .set_non_live_stream(UnobservedLogStream::Connecting);
                if state
                    .buffer
                    .session()
                    .is_some_and(|observed| observed != &session)
                {
                    state.buffer = LogBuffer::Unobserved {
                        stream: UnobservedLogStream::Connecting,
                    };
                }
            }
            LogBindingIntent::Unbind => state.buffer.set_non_live_stream(UnobservedLogStream::Idle),
        }
        LogBindingOutcome::Bound
    }

    pub fn ingest(
        &self,
        session: LogSession,
        event: RuntimeStreamEvent<String>,
    ) -> LogIngestOutcome {
        let mut state = self.state.lock().expect("log facts");
        if state.binding.session() != Some(&session) {
            return LogIngestOutcome::RejectedForeignSession;
        }
        match event {
            RuntimeStreamEvent::Connecting => state
                .buffer
                .set_non_live_stream(UnobservedLogStream::Connecting),
            RuntimeStreamEvent::Connected => state.buffer.connect(session),
            RuntimeStreamEvent::Item(raw) => {
                state.next_id = state
                    .next_id
                    .checked_add(1)
                    .expect("log identity exhausted");
                let id = state.next_id;
                state.buffer.push(session, RawLogRecord { id, raw });
            }
            RuntimeStreamEvent::Reconnecting(failure) => state
                .buffer
                .set_non_live_stream(UnobservedLogStream::Reconnecting(failure)),
            RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Cancelled) => state
                .buffer
                .set_non_live_stream(UnobservedLogStream::Cancelled),
            RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Failed(failure)) => state
                .buffer
                .set_non_live_stream(UnobservedLogStream::Failed(failure)),
            RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Unsupported(failure)) => state
                .buffer
                .set_non_live_stream(UnobservedLogStream::Unsupported(failure)),
        }
        LogIngestOutcome::Applied
    }

    pub fn fail(&self, session: LogSession, failure: Failure) -> LogIngestOutcome {
        let mut state = self.state.lock().expect("log facts");
        if state.binding.session() == Some(&session) {
            state
                .buffer
                .set_non_live_stream(UnobservedLogStream::Failed(failure));
            LogIngestOutcome::Applied
        } else {
            LogIngestOutcome::RejectedForeignSession
        }
    }

    pub fn clear(&self) {
        self.state.lock().expect("log facts").buffer.clear_records();
    }

    pub fn set_filter(&self, filter: Option<String>) -> Result<(), Failure> {
        let level = filter
            .map(|value| {
                let level = LogLevel::from_identifier(&value);
                if level == LogLevel::Unknown && !value.trim().eq_ignore_ascii_case("unknown") {
                    Err(Failure::new(
                        ErrorCode::InvalidInput,
                        "unrecognized log severity",
                        false,
                    ))
                } else {
                    Ok(level)
                }
            })
            .transpose()?;
        self.state.lock().expect("log facts").filter = level;
        Ok(())
    }

    pub fn facts(&self) -> LogFactsSnapshot {
        let state = self.state.lock().expect("log facts");
        LogFactsSnapshot {
            binding: state.binding.clone(),
            observation: state.buffer.snapshot(),
            active_level: state.filter,
        }
    }
}
