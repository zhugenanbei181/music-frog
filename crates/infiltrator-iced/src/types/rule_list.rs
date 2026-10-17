//! Correlated TEA terminals for the shared staged rule-list owner.
use infiltrator_contract::rule_document::RuleListOperationId;
use lurker_core_contract::error::Failure;
#[derive(Clone, Debug)]
pub enum RuleListAction {
    Discard,
    DiscardForm,
    Settings,
    Finished {
        operation: RuleListOperationId,
        result: Result<(), Failure>,
    },
}
