//! TEA terminal identity for an explicit shared simulation command.
use infiltrator_contract::error::Failure;
use infiltrator_contract::rule_condition::TrafficField;
use infiltrator_contract::rule_trace_run::RuleTraceOperationId;
#[derive(Clone, Debug)]
pub enum RuleTraceAction {
    ToggleSandbox,
    Sandbox(TrafficField, String),
    Settings,
    ConfirmOverride,
    OverrideFinished {
        operation: RuleTraceOperationId,
        result: Result<(), Failure>,
    },
    Finished {
        operation: RuleTraceOperationId,
        result: Result<(), Failure>,
    },
}
