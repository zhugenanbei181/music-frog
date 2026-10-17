//! Multi-round outbound speedtests over the engine-neutral outbound owner.

use crate::outbound_application::OutboundApplication;
use crate::settings_application::SettingsApplication;
use futures_util::future::{AbortHandle, AbortRegistration};
use futures_util::stream::{self, StreamExt};
use infiltrator_contract::runtime_settings::RuntimeProbeSettings;
use infiltrator_contract::speedtest::{
    HistoricalSpeedtestRecord, SpeedtestCancelOutcome, SpeedtestConcurrency,
    SpeedtestOutboundResult, SpeedtestProgress, SpeedtestResultState, SpeedtestRunId,
    SpeedtestRunState, SpeedtestSampleOutcome, SpeedtestSnapshot, SpeedtestStartReceipt,
    SpeedtestTarget,
};
use infiltrator_ports::speedtest_history::SpeedtestHistoryStore;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::details::{
    OutboundDetailsState, OutboundObservation, OutboundProbeAvailability,
};
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_contract::outbound::inventory::OutboundObservationSnapshot;
use lurker_core_contract::outbound::probe::{OutboundProbeBatchScope, ProbeOutboundRequest};
use std::mem::replace;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_HISTORY_ENTRIES: usize = 3;

#[derive(Clone)]
pub(crate) struct PreparedSpeedtestRun {
    target: SpeedtestTarget,
    settings: RuntimeProbeSettings,
    outbound: OutboundApplication,
    candidates: Vec<ProbeCandidate>,
    target_order: Vec<OutboundId>,
}

pub(crate) struct SpeedtestRunAdmission {
    pub receipt: SpeedtestStartReceipt,
    pub registration: AbortRegistration,
    pub prepared: PreparedSpeedtestRun,
    pub concurrency: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpeedtestTaskOutcome {
    Completed,
    Cancelled,
    Failed(Failure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpeedtestCompletionAdmission {
    Applied,
    IgnoredStaleRun {
        completed: SpeedtestRunId,
        current: Option<SpeedtestRunId>,
    },
    IgnoredStaleScope {
        completed: SpeedtestRunId,
    },
}

enum SpeedtestExecution {
    Idle,
    Probing {
        run_id: SpeedtestRunId,
        target: SpeedtestTarget,
        progress: SpeedtestProgress,
        abort: AbortHandle,
    },
    CancelRequested {
        run_id: SpeedtestRunId,
        target: SpeedtestTarget,
        progress: SpeedtestProgress,
    },
    Completed {
        run_id: SpeedtestRunId,
        target: SpeedtestTarget,
        progress: SpeedtestProgress,
    },
    Cancelled {
        run_id: SpeedtestRunId,
        target: SpeedtestTarget,
        progress: SpeedtestProgress,
    },
    Failed {
        run_id: SpeedtestRunId,
        target: SpeedtestTarget,
        failure: Failure,
    },
}

struct SpeedtestOwnerState {
    execution: SpeedtestExecution,
    next_run_id: u64,
    revision: u64,
    concurrency: SpeedtestConcurrency,
    results: Vec<SpeedtestOutboundResult>,
    recent_history: Vec<HistoricalSpeedtestRecord>,
}

impl Default for SpeedtestOwnerState {
    fn default() -> Self {
        Self {
            execution: SpeedtestExecution::Idle,
            next_run_id: 1,
            revision: 0,
            concurrency: SpeedtestConcurrency::default(),
            results: Vec::new(),
            recent_history: Vec::new(),
        }
    }
}

#[path = "speedtest_application/owner_state.rs"]
mod owner_state;

/// Shared application service that validates one accepted inventory before
/// probing, then records multiple samples through the typed outbound port.
#[derive(Clone)]
pub struct SpeedtestApplication {
    outbound: Arc<RwLock<Option<OutboundApplication>>>,
    probe_settings: SettingsApplication,
    state: Arc<Mutex<SpeedtestOwnerState>>,
    history_store: Option<Arc<dyn SpeedtestHistoryStore>>,
}

impl SpeedtestApplication {
    pub fn new(probe_settings: SettingsApplication) -> Self {
        Self {
            outbound: Arc::new(RwLock::new(None)),
            probe_settings,
            state: Arc::new(Mutex::new(SpeedtestOwnerState::default())),
            history_store: None,
        }
    }

    /// Install the same outbound application that Core commands and the
    /// surface reader use. The owner is write-once to prevent revision forks.
    pub fn install_outbound_application(
        &self,
        application: OutboundApplication,
    ) -> Result<(), Failure> {
        let mut slot = self.outbound.write().expect("speedtest outbound lock");
        if slot.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "speedtest outbound application is already installed",
                false,
            ));
        }
        *slot = Some(application);
        Ok(())
    }

    fn outbound_application(&self) -> Result<OutboundApplication, Failure> {
        self.outbound
            .read()
            .expect("speedtest outbound lock")
            .clone()
            .ok_or_else(|| {
                Failure::unsupported("speedtest requires the active Core outbound owner")
            })
    }

    async fn accepted_probe_settings(&self) -> Result<RuntimeProbeSettings, Failure> {
        self.probe_settings.runtime_probe_settings().await
    }

    pub(crate) fn set_concurrency(&self, concurrency: SpeedtestConcurrency) {
        let mut state = self.state.lock().expect("speedtest state lock");
        state.concurrency = concurrency;
        state.revision = state.revision.saturating_add(1);
    }

    pub(crate) fn snapshot(&self) -> SpeedtestSnapshot {
        self.state.lock().expect("speedtest state lock").snapshot()
    }

    pub(crate) fn cancel_run(&self, requested: SpeedtestRunId) -> SpeedtestCancelOutcome {
        let mut state = self.state.lock().expect("speedtest state lock");
        let current_run_id = state.run_id();
        let outcome = match &state.execution {
            SpeedtestExecution::Probing {
                run_id,
                target,
                progress,
                abort,
            } if *run_id == requested => {
                let run_id = *run_id;
                let target = target.clone();
                let progress = progress.clone();
                let abort = abort.clone();
                state.execution = SpeedtestExecution::CancelRequested {
                    run_id,
                    target,
                    progress,
                };
                abort.abort();
                state.revision = state.revision.saturating_add(1);
                return SpeedtestCancelOutcome::Requested { run_id };
            }
            SpeedtestExecution::CancelRequested { run_id, .. } if *run_id == requested => {
                SpeedtestCancelOutcome::AlreadyRequested { run_id: *run_id }
            }
            SpeedtestExecution::Completed { run_id, .. }
            | SpeedtestExecution::Cancelled { run_id, .. }
            | SpeedtestExecution::Failed { run_id, .. }
                if *run_id == requested =>
            {
                SpeedtestCancelOutcome::RunAlreadyFinished { run_id: *run_id }
            }
            _ => SpeedtestCancelOutcome::StaleRun {
                requested_run_id: requested,
                current_run_id,
            },
        };
        outcome
    }

    pub fn with_history_store(mut self, store: Arc<dyn SpeedtestHistoryStore>) -> Self {
        if let Ok(records) = store.load() {
            let mut state = self.state.lock().expect("speedtest state lock");
            state.recent_history = bounded_history(records);
        }
        self.history_store = Some(store);
        self
    }

    /// Validate the scope, inventory revision, membership and probe support
    /// before the Core actor accepts a generation.
    pub(crate) async fn prepare(
        &self,
        target: SpeedtestTarget,
    ) -> Result<PreparedSpeedtestRun, Failure> {
        if self.state.lock().expect("speedtest state lock").is_active() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "a speedtest run is already active",
                true,
            ));
        }
        let settings = self.accepted_probe_settings().await?;
        let outbound = self.outbound_application()?;
        target
            .validate()
            .map_err(|error| invalid_target(error.to_string()))?;
        let observation = outbound.observation(target.core_scope().clone()).await?;
        if observation.inventory().scope() != target.core_scope()
            || observation.details().scope() != target.core_scope()
        {
            return Err(invalid_target(
                "speedtest observation belongs to another Core activation",
            ));
        }
        if observation.inventory().revision() != target.expected_inventory_revision() {
            return Err(invalid_target(
                "speedtest target uses a stale outbound inventory revision",
            ));
        }
        let candidates = resolve_candidates(&observation, target.outbound_scope())?;
        let target_order = candidates
            .iter()
            .map(|candidate| candidate.outbound_id.clone())
            .collect();
        Ok(PreparedSpeedtestRun {
            target,
            settings,
            outbound,
            candidates,
            target_order,
        })
    }

    pub(crate) fn admit(
        &self,
        prepared: PreparedSpeedtestRun,
    ) -> Result<SpeedtestRunAdmission, Failure> {
        let mut state = self.state.lock().expect("speedtest state lock");
        if state.is_active() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "a speedtest run is already active",
                true,
            ));
        }
        let run_id = SpeedtestRunId::new(state.next_run_id).map_err(|_| {
            Failure::new(
                ErrorCode::Internal,
                "speedtest run identity space is exhausted",
                false,
            )
        })?;
        state.next_run_id = state.next_run_id.checked_add(1).ok_or_else(|| {
            Failure::new(
                ErrorCode::Internal,
                "speedtest run identity space is exhausted",
                false,
            )
        })?;
        let (abort, registration) = AbortHandle::new_pair();
        let progress = SpeedtestProgress {
            completed_targets: 0,
            total_targets: prepared.candidates.len(),
        };
        state.execution = SpeedtestExecution::Probing {
            run_id,
            target: prepared.target.clone(),
            progress,
            abort,
        };
        state.results.clear();
        state.revision = state.revision.saturating_add(1);
        Ok(SpeedtestRunAdmission {
            receipt: SpeedtestStartReceipt { run_id },
            registration,
            concurrency: usize::from(state.concurrency.get()),
            prepared,
        })
    }

    pub(crate) async fn execute_prepared(
        &self,
        run_id: SpeedtestRunId,
        prepared: PreparedSpeedtestRun,
        concurrency: usize,
    ) {
        let _completed = stream::iter(prepared.candidates.into_iter().map(|candidate| {
            let application = self.clone();
            let outbound = prepared.outbound.clone();
            let target = prepared.target.clone();
            let settings = prepared.settings.clone();
            let target_order = prepared.target_order.clone();
            async move {
                let result = application
                    .probe_candidate(&outbound, &target, &settings, candidate)
                    .await;
                let result = match result {
                    Ok(result) => result,
                    Err((outbound_id, failure)) => SpeedtestOutboundResult {
                        outbound_id,
                        display_name: None,
                        protocol_display_name: None,
                        samples: Vec::new(),
                        state: SpeedtestResultState::Failed { failure },
                        tested_at_epoch_ms: current_epoch_ms(),
                    },
                };
                application.record_completed_target(run_id, &target, result, &target_order);
            }
        }))
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;
    }

    fn record_completed_target(
        &self,
        run_id: SpeedtestRunId,
        target: &SpeedtestTarget,
        result: SpeedtestOutboundResult,
        target_order: &[OutboundId],
    ) {
        let mut state = self.state.lock().expect("speedtest state lock");
        if let SpeedtestExecution::Probing {
            run_id: active_run,
            target: active_target,
            progress,
            ..
        } = &mut state.execution
            && *active_run == run_id
            && active_target.core_scope() == target.core_scope()
        {
            progress.completed_targets = progress.completed_targets.saturating_add(1);
            state.results.push(result);
            state.results.sort_by_key(|result| {
                target_order
                    .iter()
                    .position(|id| id == &result.outbound_id)
                    .unwrap_or(usize::MAX)
            });
            state.revision = state.revision.saturating_add(1);
        }
    }

    pub(crate) fn finish(
        &self,
        run_id: SpeedtestRunId,
        request_scope: &CoreScope,
        current_scope: Option<&CoreScope>,
        outcome: SpeedtestTaskOutcome,
    ) -> SpeedtestCompletionAdmission {
        let mut state = self.state.lock().expect("speedtest state lock");
        let current_run = state.run_id();
        let active_target = match &state.execution {
            SpeedtestExecution::Probing {
                run_id: active,
                target,
                ..
            }
            | SpeedtestExecution::CancelRequested {
                run_id: active,
                target,
                ..
            } if *active == run_id => Some(target.clone()),
            _ => None,
        };
        let Some(target) = active_target else {
            return SpeedtestCompletionAdmission::IgnoredStaleRun {
                completed: run_id,
                current: current_run,
            };
        };
        if target.core_scope() != request_scope || current_scope != Some(request_scope) {
            return SpeedtestCompletionAdmission::IgnoredStaleScope { completed: run_id };
        }
        let progress = match &state.execution {
            SpeedtestExecution::Probing { progress, .. }
            | SpeedtestExecution::CancelRequested { progress, .. } => progress.clone(),
            _ => unreachable!("active target matched an active execution variant"),
        };
        let cancelled = matches!(&state.execution, SpeedtestExecution::CancelRequested { .. })
            || matches!(outcome, SpeedtestTaskOutcome::Cancelled);
        state.execution = match outcome {
            SpeedtestTaskOutcome::Failed(failure) if !cancelled => SpeedtestExecution::Failed {
                run_id,
                target,
                failure,
            },
            SpeedtestTaskOutcome::Completed if !cancelled => {
                let record = build_history_record(run_id, &state.results, &target);
                push_history_record(&mut state.recent_history, record);
                if let Some(store) = &self.history_store {
                    let _ = store.save(&state.recent_history);
                }
                SpeedtestExecution::Completed {
                    run_id,
                    target,
                    progress,
                }
            }
            SpeedtestTaskOutcome::Cancelled
            | SpeedtestTaskOutcome::Completed
            | SpeedtestTaskOutcome::Failed(_) => SpeedtestExecution::Cancelled {
                run_id,
                target,
                progress,
            },
        };
        state.revision = state.revision.saturating_add(1);
        SpeedtestCompletionAdmission::Applied
    }

    pub(crate) fn fail_admission(
        &self,
        run_id: SpeedtestRunId,
        scope: CoreScope,
        failure: Failure,
    ) {
        let mut state = self.state.lock().expect("speedtest state lock");
        let target = match &state.execution {
            SpeedtestExecution::Probing {
                run_id: active,
                target,
                ..
            } if *active == run_id && target.core_scope() == &scope => Some(target.clone()),
            _ => None,
        };
        if let Some(target) = target {
            state.execution = SpeedtestExecution::Failed {
                run_id,
                target,
                failure,
            };
            state.revision = state.revision.saturating_add(1);
        }
    }

    pub(crate) fn invalidate_scope(&self, current_scope: Option<&CoreScope>) {
        let mut state = self.state.lock().expect("speedtest state lock");
        let execution = replace(&mut state.execution, SpeedtestExecution::Idle);
        match execution {
            SpeedtestExecution::Probing {
                run_id,
                target,
                progress,
                abort,
            } if current_scope != Some(target.core_scope()) => {
                abort.abort();
                state.execution = SpeedtestExecution::Cancelled {
                    run_id,
                    target,
                    progress,
                };
                state.revision = state.revision.saturating_add(1);
            }
            SpeedtestExecution::CancelRequested {
                run_id,
                target,
                progress,
            } if current_scope != Some(target.core_scope()) => {
                state.execution = SpeedtestExecution::Cancelled {
                    run_id,
                    target,
                    progress,
                };
                state.revision = state.revision.saturating_add(1);
            }
            other => state.execution = other,
        }
    }

    pub(crate) fn cancel_for_shutdown(&self) {
        let mut state = self.state.lock().expect("speedtest state lock");
        let execution = replace(&mut state.execution, SpeedtestExecution::Idle);
        match execution {
            SpeedtestExecution::Probing {
                run_id,
                target,
                progress,
                abort,
            } => {
                abort.abort();
                state.execution = SpeedtestExecution::Cancelled {
                    run_id,
                    target,
                    progress,
                };
                state.revision = state.revision.saturating_add(1);
            }
            SpeedtestExecution::CancelRequested {
                run_id,
                target,
                progress,
            } => {
                state.execution = SpeedtestExecution::Cancelled {
                    run_id,
                    target,
                    progress,
                };
                state.revision = state.revision.saturating_add(1);
            }
            other => state.execution = other,
        }
    }

    async fn probe_candidate(
        &self,
        outbound: &OutboundApplication,
        target: &SpeedtestTarget,
        settings: &RuntimeProbeSettings,
        candidate: ProbeCandidate,
    ) -> Result<SpeedtestOutboundResult, (OutboundId, Failure)> {
        let outbound_id = candidate.outbound_id.clone();
        if candidate.probe_availability == OutboundProbeAvailability::Unsupported {
            return Ok(candidate.result(Vec::new(), SpeedtestResultState::Unsupported));
        }
        if candidate.probe_availability == OutboundProbeAvailability::Unknown {
            return Ok(candidate.result(
                Vec::new(),
                SpeedtestResultState::Failed {
                    failure: Failure::new(
                        ErrorCode::NotReady,
                        "outbound probe support has not been observed",
                        true,
                    ),
                },
            ));
        }

        let mut samples = Vec::with_capacity(usize::from(target.rounds().get()));
        for _ in 0..target.rounds().get() {
            let request = ProbeOutboundRequest::new(
                outbound_id.clone(),
                target.expected_inventory_revision(),
                settings.url(),
                u64::from(settings.timeout_ms()),
            )
            .map_err(|error| {
                (
                    outbound_id.clone(),
                    Failure::new(ErrorCode::InvalidInput, error.to_string(), false),
                )
            })?;
            let snapshot = match outbound.probe_request(target.core_scope(), &request).await {
                Ok(snapshot) => snapshot,
                Err(failure) => {
                    let state = SpeedtestResultState::Failed {
                        failure: failure.clone(),
                    };
                    return Ok(candidate.result(samples, state));
                }
            };
            let sample = SpeedtestSampleOutcome::from(snapshot.state());
            let terminal = match &sample {
                SpeedtestSampleOutcome::Unsupported => Some(SpeedtestResultState::Unsupported),
                SpeedtestSampleOutcome::Failed { failure } => Some(SpeedtestResultState::Failed {
                    failure: failure.clone(),
                }),
                SpeedtestSampleOutcome::Cancelled => Some(SpeedtestResultState::Cancelled),
                SpeedtestSampleOutcome::Measured { .. } | SpeedtestSampleOutcome::TimedOut => None,
            };
            samples.push(sample);
            if let Some(terminal) = terminal {
                return Ok(candidate.result(samples, terminal));
            }
        }

        let state = if samples
            .iter()
            .all(|sample| matches!(sample, SpeedtestSampleOutcome::TimedOut))
        {
            SpeedtestResultState::TimedOut
        } else {
            SpeedtestResultState::Completed
        };
        Ok(candidate.result(samples, state))
    }
}

#[derive(Clone)]
struct ProbeCandidate {
    outbound_id: OutboundId,
    display_name: Option<String>,
    protocol_display_name: Option<String>,
    probe_availability: OutboundProbeAvailability,
}

impl ProbeCandidate {
    fn result(
        &self,
        samples: Vec<SpeedtestSampleOutcome>,
        state: SpeedtestResultState,
    ) -> SpeedtestOutboundResult {
        SpeedtestOutboundResult {
            outbound_id: self.outbound_id.clone(),
            display_name: self.display_name.clone(),
            protocol_display_name: self.protocol_display_name.clone(),
            samples,
            state,
            tested_at_epoch_ms: current_epoch_ms(),
        }
    }
}

fn resolve_candidates(
    observation: &OutboundObservationSnapshot,
    target_scope: &OutboundProbeBatchScope,
) -> Result<Vec<ProbeCandidate>, Failure> {
    let inventory = observation.inventory();
    let (entries, collections) = match inventory.inventory() {
        OutboundInventoryState::Ready {
            entries,
            collections,
        } => (entries, collections),
        OutboundInventoryState::Empty if matches!(target_scope, OutboundProbeBatchScope::All) => {
            return Ok(Vec::new());
        }
        OutboundInventoryState::Empty => {
            return Err(invalid_target(
                "speedtest target is absent from the empty accepted inventory",
            ));
        }
        OutboundInventoryState::Failed { failure } => return Err(failure.clone()),
        OutboundInventoryState::Unknown => {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "outbound inventory has not been observed",
                true,
            ));
        }
        OutboundInventoryState::Unsupported => {
            return Err(Failure::unsupported(
                "speedtest requires a typed outbound inventory",
            ));
        }
    };
    let details = match observation.details().state() {
        OutboundDetailsState::Ready { details } => details,
        OutboundDetailsState::Empty if entries.is_empty() => return Ok(Vec::new()),
        OutboundDetailsState::Empty => {
            return Err(invalid_target(
                "outbound details are empty for the observed inventory",
            ));
        }
        OutboundDetailsState::Failed { failure } => return Err(failure.clone()),
        OutboundDetailsState::Unsupported => {
            return Err(Failure::unsupported(
                "speedtest requires typed outbound probe capability details",
            ));
        }
        OutboundDetailsState::Unknown => {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "outbound probe details have not been observed",
                true,
            ));
        }
    };
    let collection_ids = collections
        .iter()
        .map(|collection| &collection.id)
        .collect::<Vec<_>>();
    let detail_for = |id: &OutboundId| details.iter().find(|detail| &detail.id == id);
    let is_leaf = |id: &OutboundId| {
        !collection_ids.contains(&id)
            && detail_for(id).is_some_and(|detail| {
                !matches!(&detail.collection_kind, OutboundObservation::Observed(_))
            })
    };
    let ids = match target_scope {
        OutboundProbeBatchScope::One(id) => {
            if !entries.iter().any(|entry| &entry.id == id) || !is_leaf(id) {
                return Err(invalid_target(
                    "speedtest outbound is not a leaf member of the accepted inventory",
                ));
            }
            vec![id.clone()]
        }
        OutboundProbeBatchScope::Collection(id) => {
            let collection = collections
                .iter()
                .find(|collection| &collection.id == id)
                .ok_or_else(|| invalid_target("speedtest collection is absent from inventory"))?;
            collection
                .member_ids
                .iter()
                .filter(|member_id| is_leaf(member_id))
                .cloned()
                .collect()
        }
        OutboundProbeBatchScope::Selected(ids) => {
            for id in ids {
                if !entries.iter().any(|entry| &entry.id == id) || !is_leaf(id) {
                    return Err(invalid_target(
                        "speedtest selection contains a non-member or collection ID",
                    ));
                }
            }
            ids.clone()
        }
        OutboundProbeBatchScope::All => entries
            .iter()
            .filter(|entry| is_leaf(&entry.id))
            .map(|entry| entry.id.clone())
            .collect(),
    };

    let mut candidates = Vec::with_capacity(ids.len());
    for id in ids {
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| invalid_target("speedtest target is absent from inventory"))?;
        let detail = detail_for(&id)
            .ok_or_else(|| invalid_target("speedtest target has no accepted detail record"))?;
        candidates.push(ProbeCandidate {
            outbound_id: id,
            display_name: entry.display_name.clone(),
            protocol_display_name: detail.protocol_display_name.clone(),
            probe_availability: detail.probe_availability,
        });
    }
    Ok(candidates)
}

#[path = "speedtest_application/history.rs"]
mod history;
use history::*;

#[cfg(test)]
#[path = "speedtest_application_tests.rs"]
pub(crate) mod tests;
