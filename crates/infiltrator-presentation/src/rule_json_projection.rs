//! Shared status facets for the source document editor; input bytes stay separate.
use infiltrator_contract::rule_json_feedback::RuleJsonFeedback;
use lurker_ui_presentation::copy::TextCatalog;

pub fn json_editor_status(
    focused: bool,
    dirty: bool,
    published: bool,
    feedback: Option<RuleJsonFeedback>,
    catalog: &dyn TextCatalog,
) -> String {
    let focus = catalog.text(if focused {
        "rules_json_editing"
    } else {
        "rules_json_unfocused"
    });
    let saved = catalog.text(if dirty {
        "rules_json_dirty"
    } else if published {
        "rules_json_observed"
    } else {
        "rules_json_unobserved"
    });
    match feedback {
        Some(feedback) => format!(
            "{focus} · {saved} · {}",
            catalog.text(match feedback {
                RuleJsonFeedback::HostUnavailable => "rules_json_host_unavailable",
                RuleJsonFeedback::SourceUnavailable => "rules_json_source_unavailable",
                RuleJsonFeedback::Empty => "rules_json_empty",
                RuleJsonFeedback::InvalidDocument => "rules_json_invalid_document",
                RuleJsonFeedback::AwaitingReadback => "rules_json_waiting_readback",
            })
        ),
        None => format!("{focus} · {saved}"),
    }
}
