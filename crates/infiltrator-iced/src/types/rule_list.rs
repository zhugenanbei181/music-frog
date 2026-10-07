//! Correlated TEA terminals for the shared staged rule-list owner.
use infiltrator_contract::error::Failure;
use infiltrator_contract::rule_document::RuleListOperationId;
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
