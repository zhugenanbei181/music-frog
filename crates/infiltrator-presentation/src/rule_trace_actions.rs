//! Shared draft and terminal correlation for the two rule simulation surfaces.
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_condition::TrafficField;
use infiltrator_contract::rule_trace_run::{
    RuleTraceExecution, RuleTraceOperationId, RuleTraceRequest,
};
use infiltrator_contract::rule_tracer::{
    RuleTracerSnapshot, TracerRuleOverride, TrafficContextSnapshot,
};
use infiltrator_domain::rules::tracer::TrafficContext;
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RuleTraceReadbackPhase {
    #[default]
    Waiting,
    Observed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RuleTraceRunPhase {
    #[default]
    Idle,
    ValidationFailed(Failure),
    Submitting {
        operation: RuleTraceOperationId,
        readback: RuleTraceReadbackPhase,
    },
    AwaitingReadback {
        operation: RuleTraceOperationId,
    },
    Observed {
        operation: RuleTraceOperationId,
    },
    Failed {
        operation: RuleTraceOperationId,
        failure: Failure,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RuleOverridePhase {
    #[default]
    Idle,
    Confirming(TracerRuleOverride),
    Submitting {
        operation: RuleTraceOperationId,
        request: TracerRuleOverride,
    },
    Failed {
        request: TracerRuleOverride,
        failure: Failure,
    },
}

#[derive(Clone, Debug, Default)]
pub struct RuleTraceActions {
    pub query: String,
    pub source_ip: String,
    pub sandbox: BTreeMap<TrafficField, String>,
    pub advanced_open: bool,
    run: RuleTraceRunPhase,
    pub snapshot: RuleTraceExecution,
    override_phase: RuleOverridePhase,
}
impl RuleTraceActions {
    pub fn run_phase(&self) -> &RuleTraceRunPhase {
        &self.run
    }
    pub fn override_phase(&self) -> &RuleOverridePhase {
        &self.override_phase
    }
    pub fn requested_operation(&self) -> Option<RuleTraceOperationId> {
        match &self.run {
            RuleTraceRunPhase::Submitting { operation, .. }
            | RuleTraceRunPhase::AwaitingReadback { operation }
            | RuleTraceRunPhase::Observed { operation }
            | RuleTraceRunPhase::Failed { operation, .. } => Some(*operation),
            RuleTraceRunPhase::Idle | RuleTraceRunPhase::ValidationFailed(_) => None,
        }
    }
    pub fn pending_operation(&self) -> Option<RuleTraceOperationId> {
        match &self.run {
            RuleTraceRunPhase::Submitting { operation, .. } => Some(*operation),
            RuleTraceRunPhase::Idle
            | RuleTraceRunPhase::ValidationFailed(_)
            | RuleTraceRunPhase::AwaitingReadback { .. }
            | RuleTraceRunPhase::Observed { .. }
            | RuleTraceRunPhase::Failed { .. } => None,
        }
    }
    pub fn confirmation(&self) -> Option<&TracerRuleOverride> {
        match &self.override_phase {
            RuleOverridePhase::Confirming(request)
            | RuleOverridePhase::Submitting { request, .. }
            | RuleOverridePhase::Failed { request, .. } => Some(request),
            RuleOverridePhase::Idle => None,
        }
    }
    pub fn override_pending_operation(&self) -> Option<RuleTraceOperationId> {
        match &self.override_phase {
            RuleOverridePhase::Submitting { operation, .. } => Some(*operation),
            RuleOverridePhase::Idle
            | RuleOverridePhase::Confirming(_)
            | RuleOverridePhase::Failed { .. } => None,
        }
    }
    pub fn override_failure(&self) -> Option<&Failure> {
        match &self.override_phase {
            RuleOverridePhase::Failed { failure, .. } => Some(failure),
            RuleOverridePhase::Idle
            | RuleOverridePhase::Confirming(_)
            | RuleOverridePhase::Submitting { .. } => None,
        }
    }
    pub fn failure(&self) -> Option<&Failure> {
        match &self.run {
            RuleTraceRunPhase::ValidationFailed(failure)
            | RuleTraceRunPhase::Failed { failure, .. } => Some(failure),
            RuleTraceRunPhase::Idle
            | RuleTraceRunPhase::Submitting { .. }
            | RuleTraceRunPhase::AwaitingReadback { .. }
            | RuleTraceRunPhase::Observed { .. } => None,
        }
    }
    pub fn begin(&mut self) -> Result<(RuleTraceOperationId, RuleTraceRequest), Failure> {
        if self.busy() || self.confirmation().is_some() {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Rule simulation is already pending",
                true,
            ));
        }
        let request = match self.request() {
            Ok(request) => request,
            Err(failure) => {
                self.run = RuleTraceRunPhase::ValidationFailed(failure.clone());
                return Err(failure);
            }
        };
        if let Err(failure) = request.validate() {
            self.run = RuleTraceRunPhase::ValidationFailed(failure.clone());
            return Err(failure);
        }
        let operation = match self.allocate() {
            Ok(operation) => operation,
            Err(failure) => {
                self.run = RuleTraceRunPhase::ValidationFailed(failure.clone());
                return Err(failure);
            }
        };
        self.run = RuleTraceRunPhase::Submitting {
            operation,
            readback: RuleTraceReadbackPhase::Waiting,
        };
        Ok((operation, request))
    }
    pub fn finish(&mut self, operation: RuleTraceOperationId, result: Result<(), Failure>) -> bool {
        let readback_observed = match &self.run {
            RuleTraceRunPhase::Submitting {
                operation: pending,
                readback,
            } if *pending == operation => *readback == RuleTraceReadbackPhase::Observed,
            _ => return false,
        };
        self.run = match result {
            Ok(()) if readback_observed => RuleTraceRunPhase::Observed { operation },
            Ok(()) => RuleTraceRunPhase::AwaitingReadback { operation },
            Err(failure) => RuleTraceRunPhase::Failed { operation, failure },
        };
        true
    }
    pub fn observe(&mut self, snapshot: RuleTraceExecution) -> bool {
        if snapshot.revision < self.snapshot.revision {
            return false;
        }
        if let Some(requested) = self.requested_operation()
            && snapshot
                .operation_id
                .is_none_or(|operation| operation.0 < requested.0)
        {
            return false;
        }
        let observed_operation = snapshot.operation_id;
        self.run = match &self.run {
            RuleTraceRunPhase::Submitting {
                operation,
                readback: RuleTraceReadbackPhase::Waiting,
            } if observed_operation == Some(*operation) => RuleTraceRunPhase::Submitting {
                operation: *operation,
                readback: RuleTraceReadbackPhase::Observed,
            },
            RuleTraceRunPhase::AwaitingReadback { operation }
                if observed_operation == Some(*operation) =>
            {
                RuleTraceRunPhase::Observed {
                    operation: *operation,
                }
            }
            phase => phase.clone(),
        };
        self.snapshot = snapshot;
        true
    }
    pub fn set_query(&mut self, query: String) {
        if !self.busy() && self.confirmation().is_none() {
            self.query = query;
        }
    }
    pub fn set_source_ip(&mut self, source_ip: String) {
        if !self.busy() && self.confirmation().is_none() {
            self.source_ip = source_ip;
        }
    }
    pub fn busy(&self) -> bool {
        matches!(
            &self.run,
            RuleTraceRunPhase::Submitting { .. } | RuleTraceRunPhase::AwaitingReadback { .. }
        ) || self.override_pending_operation().is_some()
    }
    pub fn prepare_override(
        &mut self,
        report: &RuleTracerSnapshot,
        index: usize,
        target: String,
    ) -> bool {
        if self.busy()
            || !matches!(&self.override_phase, RuleOverridePhase::Idle)
            || !report.can_reverse_apply
            || self.current_failure().is_some()
            || (self.snapshot.report_id.is_some() && !self.draft_matches(report))
        {
            return false;
        }
        let Some(source) = report.source.clone() else {
            return false;
        };
        let Some(chain) = &report.decision_chain else {
            return false;
        };
        if chain.hit_rule_index != Some(index) || chain.is_fallback {
            return false;
        }
        self.override_phase = RuleOverridePhase::Confirming(TracerRuleOverride {
            rule_index: index,
            rule_table: chain.hit_rule_table.clone(),
            new_target: target.trim().to_owned(),
            expected_source: source,
            expected_rule: chain.matched_rule_raw.clone(),
        });
        true
    }
    pub fn cancel_override(&mut self) -> bool {
        match &self.override_phase {
            RuleOverridePhase::Confirming(_) | RuleOverridePhase::Failed { .. } => {
                self.override_phase = RuleOverridePhase::Idle;
                true
            }
            RuleOverridePhase::Idle | RuleOverridePhase::Submitting { .. } => false,
        }
    }
    pub fn begin_override(&mut self) -> Option<(RuleTraceOperationId, TracerRuleOverride)> {
        if self.pending_operation().is_some() || self.override_pending_operation().is_some() {
            return None;
        }
        let request = match &self.override_phase {
            RuleOverridePhase::Confirming(request) | RuleOverridePhase::Failed { request, .. } => {
                request.clone()
            }
            RuleOverridePhase::Idle | RuleOverridePhase::Submitting { .. } => return None,
        };
        let operation = match self.allocate() {
            Ok(operation) => operation,
            Err(failure) => {
                self.override_phase = RuleOverridePhase::Failed { request, failure };
                return None;
            }
        };
        self.override_phase = RuleOverridePhase::Submitting {
            operation,
            request: request.clone(),
        };
        Some((operation, request))
    }
    pub fn finish_override(
        &mut self,
        operation: RuleTraceOperationId,
        result: Result<(), Failure>,
    ) -> bool {
        let request = match &self.override_phase {
            RuleOverridePhase::Submitting {
                operation: pending,
                request,
            } if *pending == operation => request.clone(),
            _ => return false,
        };
        match result {
            Ok(()) => self.override_phase = RuleOverridePhase::Idle,
            Err(failure) => self.override_phase = RuleOverridePhase::Failed { request, failure },
        }
        true
    }
    pub fn draft_matches(&self, report: &RuleTracerSnapshot) -> bool {
        self.request()
            .ok()
            .and_then(|request| TrafficContext::from_request(&request).ok())
            .is_some_and(|context| {
                self.query.trim() == report.active_query
                    && context.snapshot() == report.simulated_context
            })
    }
    fn allocate(&self) -> Result<RuleTraceOperationId, Failure> {
        let minimum = self
            .snapshot
            .operation_id
            .map_or(Some(1), |operation| operation.0.checked_add(1))
            .ok_or_else(exhausted)?;
        allocate(&NEXT_OPERATION, minimum)
    }
    pub fn set_sandbox(&mut self, field: TrafficField, value: String) {
        if !self.busy() && self.confirmation().is_none() {
            self.sandbox.insert(field, value);
        }
    }
    pub fn request(&self) -> Result<RuleTraceRequest, Failure> {
        let mut context = TrafficContextSnapshot {
            src_ip: (!self.source_ip.trim().is_empty()).then(|| self.source_ip.trim().to_owned()),
            ..TrafficContextSnapshot::default()
        };
        for (field, value) in &self.sandbox {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            let invalid = || {
                Failure::new(
                    ErrorCode::InvalidInput,
                    format!("Invalid sandbox {field:?}"),
                    false,
                )
            };
            match field {
                TrafficField::DestinationIp => {
                    value.parse::<IpAddr>().map_err(|_| invalid())?;
                    context.ip = Some(value.into());
                }
                TrafficField::DestinationPort
                | TrafficField::SourcePort
                | TrafficField::InboundPort => {
                    let port = value.parse::<u16>().map_err(|_| invalid())?;
                    if port == 0 {
                        return Err(invalid());
                    }
                    match field {
                        TrafficField::DestinationPort => context.port = Some(port),
                        TrafficField::SourcePort => context.src_port = Some(port),
                        _ => context.in_port = Some(port),
                    }
                }
                TrafficField::InboundType => context.in_type = Some(value.into()),
                TrafficField::InboundName => context.in_name = Some(value.into()),
                TrafficField::InboundUser => context.in_user = Some(value.into()),
                TrafficField::ProcessName => context.process_name = Some(value.into()),
                TrafficField::ProcessPath => context.process_path = Some(value.into()),
                TrafficField::Network => context.network = Some(value.into()),
                TrafficField::Dscp => {
                    context.dscp = Some(value.parse::<u8>().map_err(|_| invalid())?)
                }
                TrafficField::Uid => {
                    context.uid = Some(value.parse::<u32>().map_err(|_| invalid())?)
                }
                TrafficField::PackageName => context.package_name = Some(value.into()),
                TrafficField::SourceIp | TrafficField::Destination => return Err(invalid()),
            }
        }
        Ok(RuleTraceRequest {
            query: self.query.trim().into(),
            context,
        })
    }
    pub fn current_failure(&self) -> Option<&Failure> {
        self.override_failure()
            .or_else(|| self.failure())
            .or_else(|| {
                self.snapshot
                    .failure
                    .as_ref()
                    .filter(|_| self.snapshot.operation_id == self.requested_operation())
            })
    }
}

fn exhausted() -> Failure {
    Failure::new(
        ErrorCode::InvalidState,
        "Rule operation identity exhausted",
        false,
    )
}
fn allocate(counter: &AtomicU64, minimum: u64) -> Result<RuleTraceOperationId, Failure> {
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.max(minimum).max(1).checked_add(1)
        })
        .map(|previous| RuleTraceOperationId(previous.max(minimum).max(1)))
        .map_err(|_| exhausted())
}
#[cfg(test)]
#[path = "rule_trace_actions_test.rs"]
mod tests;
