//! Shared status facets for the source document editor; input bytes stay separate.
use infiltrator_contract::rule_json_feedback::RuleJsonFeedback;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn json_editor_status(
    focused: bool,
    dirty: bool,
    published: bool,
    feedback: Option<RuleJsonFeedback>,
    language: &str,
) -> String {
    let lang = Lang(language);
    let focus = lang.tr(if focused {
        "rules_json_editing"
    } else {
        "rules_json_unfocused"
    });
    let saved = lang.tr(if dirty {
        "rules_json_dirty"
    } else if published {
        "rules_json_observed"
    } else {
        "rules_json_unobserved"
    });
    match feedback {
        Some(feedback) => format!(
            "{focus} · {saved} · {}",
            lang.tr(match feedback {
                RuleJsonFeedback::HostUnavailable => "rules_json_host_unavailable",
                RuleJsonFeedback::Empty => "rules_json_empty",
                RuleJsonFeedback::AwaitingReadback => "rules_json_waiting_readback",
            })
        ),
        None => format!("{focus} · {saved}"),
    }
}
