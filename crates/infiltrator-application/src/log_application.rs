//! Bounded, session-fenced controller log facts. Readers never consume the stream.
use crate::log_projection::project_log_record;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::{LogLevel, LogSession, LogStreamState};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_contract::surface_snapshot::{LogSnapshot, LogsPageSnapshot, PageData, PageStatus};
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub const LOG_CAPACITY: usize = 500;
#[derive(Default)]
struct LogState {
    core_owned: bool,
    session: Option<LogSession>,
    observed_session: Option<LogSession>,
    stream: LogStreamState,
    observed: bool,
    next_id: u64,
    entries: VecDeque<LogSnapshot>,
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
    pub(crate) fn has_export_scope(&self, session: LogSession) -> bool {
        self.state.lock().expect("log facts").observed_session == Some(session)
    }
    /// The lifecycle owner retires old facts synchronously, before publishing
    /// the new Core snapshot. A stream worker must not rebind an older scope.
    pub fn observe_core(&self, core: &CoreSnapshot) {
        let mut state = self.state.lock().expect("log facts");
        state.core_owned = true;
        let scope = core.session_token.map(|token| LogSession {
            generation: core.generation,
            token,
        });
        if state.observed_session.is_some_and(|observed| {
            observed.generation != core.generation || scope.is_some_and(|scope| scope != observed)
        }) {
            state.entries.clear();
            state.observed = false;
            state.observed_session = scope;
        }
        let session = matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        )
        .then_some(scope)
        .flatten();
        if state.session != session {
            state.session = session;
            state.stream = if session.is_some() {
                LogStreamState::Connecting
            } else {
                LogStreamState::Idle
            };
        }
        if let Some(session) = session
            && state.observed_session != Some(session)
        {
            state.entries.clear();
            state.observed = false;
            state.observed_session = Some(session);
        }
    }
    /// Export all observed records, independently of the page's severity filter.
    pub fn export_records(&self) -> Result<(LogSession, Vec<(u64, String)>), Failure> {
        let state = self.state.lock().expect("log facts");
        let session = state
            .observed_session
            .filter(|_| state.observed)
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "No observed controller log buffer to export",
                    true,
                )
            })?;
        Ok((
            session,
            state
                .entries
                .iter()
                .map(|entry| {
                    entry.raw.clone().map(|raw| (entry.id, raw)).ok_or_else(|| {
                        Failure::new(
                            ErrorCode::InvalidState,
                            "An observed log record has no raw source",
                            false,
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
    pub fn bind(&self, session: Option<LogSession>) -> bool {
        let mut state = self.state.lock().expect("log facts");
        if state.core_owned {
            return state.session == session;
        }
        if state.session == session {
            return true;
        }
        state.session = session;
        if let Some(session) = session
            && state.observed_session != Some(session)
        {
            state.observed_session = Some(session);
            state.observed = false;
            state.entries.clear();
        }
        state.stream = if session.is_some() {
            LogStreamState::Connecting
        } else {
            LogStreamState::Idle
        };
        true
    }
    pub fn ingest(&self, session: LogSession, event: RuntimeStreamEvent<String>) -> bool {
        let mut state = self.state.lock().expect("log facts");
        if state.session != Some(session) {
            return false;
        }
        match event {
            RuntimeStreamEvent::Connecting => state.stream = LogStreamState::Connecting,
            RuntimeStreamEvent::Connected => {
                state.stream = LogStreamState::Live;
                state.observed = true;
            }
            RuntimeStreamEvent::Item(raw) => {
                state.next_id = state
                    .next_id
                    .checked_add(1)
                    .expect("log identity exhausted");
                let id = state.next_id;
                state.entries.push_back(project_log_record(id, &raw));
                while state.entries.len() > LOG_CAPACITY {
                    state.entries.pop_front();
                }
                state.observed = true;
                state.stream = LogStreamState::Live;
            }
            RuntimeStreamEvent::Reconnecting(failure) => {
                state.stream = LogStreamState::Reconnecting(failure)
            }
            RuntimeStreamEvent::Failed(failure) => state.stream = LogStreamState::Failed(failure),
        }
        true
    }
    pub fn fail(&self, session: LogSession, failure: Failure) {
        let mut state = self.state.lock().expect("log facts");
        if state.session == Some(session) {
            state.stream = LogStreamState::Failed(failure);
        }
    }
    pub fn clear(&self) {
        self.state.lock().expect("log facts").entries.clear();
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
    pub fn page(&self, core: &CoreSnapshot) -> PageData<LogsPageSnapshot> {
        let state = self.state.lock().expect("log facts");
        let scope = core.session_token.map(|token| LogSession {
            generation: core.generation,
            token,
        });
        let running = matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        );
        let data = LogsPageSnapshot {
            total_entries: state.entries.len(),
            active_level: state.filter.map(|level| level.label().into()),
            entries: state
                .entries
                .iter()
                .filter(|entry| {
                    state
                        .filter
                        .is_none_or(|level| LogLevel::from_identifier(&entry.level) == level)
                })
                .cloned()
                .collect(),
            stream: state.stream.clone(),
        };
        if !running || state.session != scope || scope.is_none() {
            let failure = Failure::new(
                ErrorCode::NotReady,
                "no current controller log session",
                true,
            );
            return PageData {
                status: PageStatus::Unavailable { failure },
                data: (state.observed
                    && state.observed_session.is_some_and(|observed| {
                        observed.generation == core.generation
                            && core
                                .session_token
                                .is_none_or(|token| token == observed.token)
                    }))
                .then_some(data),
            };
        }
        let status = match &state.stream {
            LogStreamState::Live if state.observed => {
                if data.entries.is_empty() {
                    PageStatus::Empty
                } else {
                    PageStatus::Ready
                }
            }
            LogStreamState::Reconnecting(failure) | LogStreamState::Failed(failure) => {
                PageStatus::Failed {
                    failure: failure.clone(),
                }
            }
            _ => PageStatus::Loading,
        };
        PageData {
            status,
            data: state.observed.then_some(data),
        }
    }
}
