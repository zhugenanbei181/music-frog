//! Commands own simulation; readers replay the last result without executing it.
use super::RuleTracerApplication;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::rule_condition::{ConditionIssue, RuleTraceIssue};
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::rule_trace_run::{
    RuleTraceExecution, RuleTraceOperation, RuleTraceOperationId, RuleTraceOutboundFacts,
    RuleTraceOwnerObservation, ScopedRuleTraceRequest,
};
use infiltrator_contract::rule_tracer::{RuleTracerSnapshot, RuleTracerStatus};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use std::sync::{Arc, Mutex};

impl RuleTracerApplication {
    pub fn execution(&self) -> RuleTraceExecution {
        self.execution
            .lock()
            .expect("rule trace execution lock")
            .clone()
    }

    pub async fn simulate_scoped(
        &self,
        observed_scope: &CoreScope,
        observed_workspace: &ProfileWorkspaceIdentity,
        operation: RuleTraceOperationId,
        request: ScopedRuleTraceRequest,
        outbound_facts: &RuleTraceOutboundFacts,
    ) -> Result<(), Failure> {
        request.validate_observation(observed_scope, observed_workspace)?;
        let session_mode = match observed_scope.session_token.as_ref() {
            Some(_) => match outbound_facts {
                RuleTraceOutboundFacts::Observed(outbound_observation) => {
                    if outbound_observation.inventory().scope() != observed_scope
                        || outbound_observation.details().scope() != observed_scope
                    {
                        return Err(Failure::new(
                            ErrorCode::InvalidState,
                            "Rule trace outbound facts belong to another Core scope",
                            false,
                        ));
                    }
                    super::RuleTraceSessionMode::Live
                }
                RuleTraceOutboundFacts::Unavailable(failure) => return Err(failure.clone()),
            },
            None => match outbound_facts {
                RuleTraceOutboundFacts::Unavailable(_) => super::RuleTraceSessionMode::Offline,
                RuleTraceOutboundFacts::Observed(_) => {
                    return Err(Failure::new(
                        ErrorCode::NotReady,
                        "A live outbound observation cannot be attached to a Core scope without a session",
                        true,
                    ));
                }
            },
        };
        let owner = request.owner.clone();
        let _admission = self.admission.try_lock().ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Another rule simulation is already running",
                true,
            )
        })?;
        let report_revision = {
            let mut state = self.execution.lock().expect("rule trace execution lock");
            if operation.0 == 0
                || state
                    .operation_id
                    .is_some_and(|previous| previous.0 >= operation.0)
            {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "Rule simulation identity is stale",
                    false,
                ));
            }
            let report_revision = state.revision.checked_add(2).ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidState,
                    "Rule simulation revision exhausted",
                    false,
                )
            })?;
            state.revision += 1;
            state.operation_id = Some(operation);
            state.operation = RuleTraceOperation::Running;
            state.failure = None;
            state.issue = None;
            state.owner = RuleTraceOwnerObservation::observed(owner.clone());
            report_revision
        };
        let mut run = SimulationRun {
            state: self.execution.clone(),
            finished: false,
        };
        let snapshot_owner = match session_mode {
            super::RuleTraceSessionMode::Live => super::RuleTraceSnapshotOwner::Live {
                generation: observed_scope.generation.value(),
                revision: report_revision,
            },
            super::RuleTraceSessionMode::Offline => super::RuleTraceSnapshotOwner::Offline {
                generation: observed_scope.generation.value(),
                revision: report_revision,
            },
        };
        let workspace = match self.override_port.get() {
            Some(port) => port
                .load_rule_workspace_for_trace(&owner)
                .await
                .map_err(Failure::from),
            None => Err(Failure::unsupported("No profile rule source is composed")),
        };
        let result = match workspace {
            Ok(workspace) => {
                if workspace.profile_identity != owner.workspace {
                    Err(Failure::new(
                        ErrorCode::NotReady,
                        "Profile workspace changed; refresh before simulating rules",
                        true,
                    ))
                } else {
                    self.statistics
                        .lock()
                        .expect("rule statistics lock")
                        .bind(&workspace.source);
                    self.set_context(&request.request.context);
                    self.set_query(&request.request.query);
                    match self.simulate_workspace_snapshot(
                        snapshot_owner,
                        &workspace.rules,
                        Some(&workspace.sub_rules),
                        outbound_facts,
                    ) {
                        Ok(mut report) => {
                            report.source = Some(workspace.source);
                            report.targets = workspace.targets;
                            report.can_reverse_apply &= report.source.is_some();
                            Ok(report)
                        }
                        Err(issue) => {
                            self.execution
                                .lock()
                                .expect("rule trace execution lock")
                                .issue = Some(issue.clone());
                            Err(issue_failure(&issue))
                        }
                    }
                }
            }
            Err(failure) => Err(failure),
        };
        let mut state = self.execution.lock().expect("rule trace execution lock");
        state.revision += 1;
        let terminal = match result {
            Ok(report) => {
                state.report = Some(report);
                state.report_id = Some(operation);
                state.operation = RuleTraceOperation::Completed;
                Ok(())
            }
            Err(failure) => {
                state.operation = if failure.code == ErrorCode::Unsupported {
                    RuleTraceOperation::Unsupported
                } else {
                    RuleTraceOperation::Failed
                };
                state.failure = Some(failure.clone());
                Err(failure)
            }
        };
        run.finished = true;
        terminal
    }

    pub fn replay(&self, source: &RuleSourceIdentity) -> RuleTracerSnapshot {
        let state = self.execution();
        let mut report = state
            .report
            .unwrap_or_else(|| RuleTracerSnapshot::empty(0, state.revision));
        report.revision = state.revision;
        report.can_reverse_apply &= report.source.as_ref() == Some(source)
            && state.operation == RuleTraceOperation::Completed
            && state.failure.is_none();
        if let Some(failure) = state.failure {
            report.status = if failure.code == ErrorCode::Unsupported {
                RuleTracerStatus::Unsupported
            } else {
                RuleTracerStatus::Failed
            };
            report.failure = Some(failure.message);
        }
        report
    }
}
struct SimulationRun {
    state: Arc<Mutex<RuleTraceExecution>>,
    finished: bool,
}
impl Drop for SimulationRun {
    fn drop(&mut self) {
        if !self.finished {
            let mut state = self.state.lock().expect("rule trace execution lock");
            state.revision += 1;
            state.operation = RuleTraceOperation::Failed;
            state.failure = Some(Failure::new(
                ErrorCode::Canceled,
                "Rule simulation canceled before completion",
                true,
            ));
        }
    }
}

fn issue_failure(issue: &RuleTraceIssue) -> Failure {
    let (code, reason) = match &issue.issue {
        ConditionIssue::MissingInput(field) => (
            ErrorCode::NotReady,
            format!("Sandbox input is missing: {field:?}"),
        ),
        ConditionIssue::ExternalData { rule_type, name } => (
            ErrorCode::Unsupported,
            format!("No {rule_type} evaluation data is available for {name}"),
        ),
        ConditionIssue::InvalidRule { reason } => (ErrorCode::Configuration, reason.clone()),
    };
    Failure::new(code, format!("Rule #{}: {reason}", issue.index + 1), false)
}
