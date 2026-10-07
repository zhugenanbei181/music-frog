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

#[derive(Clone, Debug, Default)]
pub struct RuleTraceActions {
    pub query: String,
    pub source_ip: String,
    pub sandbox: BTreeMap<TrafficField, String>,
    pub advanced_open: bool,
    pub pending: Option<RuleTraceOperationId>,
    pub requested: Option<RuleTraceOperationId>,
    pub failure: Option<Failure>,
    pub snapshot: RuleTraceExecution,
    pub confirmation: Option<TracerRuleOverride>,
    pub override_pending: Option<RuleTraceOperationId>,
    pub override_failure: Option<Failure>,
}
impl RuleTraceActions {
    pub fn begin(&mut self) -> Result<(RuleTraceOperationId, RuleTraceRequest), Failure> {
        if self.busy() || self.confirmation.is_some() {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "Rule simulation is already pending",
                true,
            ));
        }
        let request = match self.request() {
            Ok(request) => request,
            Err(failure) => {
                self.failure = Some(failure.clone());
                return Err(failure);
            }
        };
        if let Err(failure) = request.validate() {
            self.failure = Some(failure.clone());
            return Err(failure);
        }
        let operation = self.allocate()?;
        self.pending = Some(operation);
        self.requested = Some(operation);
        self.failure = None;
        Ok((operation, request))
    }
    pub fn finish(&mut self, operation: RuleTraceOperationId, result: Result<(), Failure>) -> bool {
        if self.pending != Some(operation) {
            return false;
        }
        self.pending = None;
        self.failure = result.err();
        true
    }
    pub fn observe(&mut self, snapshot: RuleTraceExecution) -> bool {
        if snapshot.revision < self.snapshot.revision {
            return false;
        }
        if let Some(requested) = self.requested
            && snapshot
                .operation_id
                .is_none_or(|operation| operation.0 < requested.0)
        {
            return false;
        }
        self.snapshot = snapshot;
        true
    }
    pub fn set_query(&mut self, query: String) {
        if !self.busy() && self.confirmation.is_none() {
            self.query = query;
        }
    }
    pub fn set_source_ip(&mut self, source_ip: String) {
        if !self.busy() && self.confirmation.is_none() {
            self.source_ip = source_ip;
        }
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some() || self.override_pending.is_some()
    }
    pub fn prepare_override(
        &mut self,
        report: &RuleTracerSnapshot,
        index: usize,
        target: String,
    ) -> bool {
        if self.busy()
            || self.confirmation.is_some()
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
        self.confirmation = Some(TracerRuleOverride {
            rule_index: index,
            rule_table: chain.hit_rule_table.clone(),
            new_target: target.trim().to_owned(),
            expected_source: source,
            expected_rule: chain.matched_rule_raw.clone(),
        });
        self.override_failure = None;
        true
    }
    pub fn cancel_override(&mut self) -> bool {
        if self.override_pending.is_some() {
            return false;
        }
        self.confirmation = None;
        self.override_failure = None;
        true
    }
    pub fn begin_override(&mut self) -> Option<(RuleTraceOperationId, TracerRuleOverride)> {
        if self.busy() {
            return None;
        }
        let request = self.confirmation.clone()?;
        let operation = match self.allocate() {
            Ok(operation) => operation,
            Err(failure) => {
                self.override_failure = Some(failure);
                return None;
            }
        };
        self.override_pending = Some(operation);
        self.override_failure = None;
        Some((operation, request))
    }
    pub fn finish_override(
        &mut self,
        operation: RuleTraceOperationId,
        result: Result<(), Failure>,
    ) -> bool {
        if self.override_pending != Some(operation) {
            return false;
        }
        self.override_pending = None;
        match result {
            Ok(()) => {
                self.confirmation = None;
                self.override_failure = None;
            }
            Err(failure) => self.override_failure = Some(failure),
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
        if !self.busy() && self.confirmation.is_none() {
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
        self.override_failure
            .as_ref()
            .or(self.failure.as_ref())
            .or_else(|| {
                self.snapshot
                    .failure
                    .as_ref()
                    .filter(|_| self.snapshot.operation_id == self.requested)
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
