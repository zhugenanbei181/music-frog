//! User-visible policy follows the actual domain decision, independent of surface vocabulary.
use infiltrator_contract::uwp::UwpLoopbackAvailability;
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub const fn mode_key(mode: AppRoutingMode) -> &'static str {
    match mode {
        AppRoutingMode::ProxyAll => "app_routing_mode_global",
        AppRoutingMode::ProxySelected => "app_routing_mode_selected_proxy",
        AppRoutingMode::BypassSelected => "app_routing_mode_selected_direct",
    }
}
pub const fn rule_key(rule: AppRoutingRule) -> &'static str {
    match rule {
        AppRoutingRule::Proxy => "app_routing_rule_proxy",
        AppRoutingRule::Direct => "app_routing_rule_direct",
        AppRoutingRule::Block => "app_routing_rule_block",
    }
}
pub fn routing_summary(mode: AppRoutingMode, count: usize, language: &str) -> String {
    interpolate(
        Lang(language).tr("app_routing_observed_summary").as_ref(),
        &[
            ("mode", Lang(language).tr(mode_key(mode)).as_ref()),
            ("count", &count.to_string()),
        ],
    )
}
pub fn process_copy(process: &str, language: &str) -> String {
    interpolate(
        Lang(language).tr("app_routing_process_copy").as_ref(),
        &[("process", process)],
    )
}

pub fn uwp_summary(
    availability: &UwpLoopbackAvailability,
    exemptions: impl IntoIterator<Item = bool>,
    language: &str,
) -> String {
    match availability {
        UwpLoopbackAvailability::Supported => {
            let (total, exempt) = exemptions
                .into_iter()
                .fold((0_usize, 0_usize), |(total, exempt), enabled| {
                    (total + 1, exempt + usize::from(enabled))
                });
            interpolate(
                Lang(language).tr("uwp_observed_summary").as_ref(),
                &[
                    ("count", &total.to_string()),
                    ("exempt", &exempt.to_string()),
                ],
            )
        }
        UwpLoopbackAvailability::Unsupported { reason }
        | UwpLoopbackAvailability::Unavailable { reason } => reason.clone(),
    }
}
pub fn uwp_empty(availability: &UwpLoopbackAvailability, language: &str) -> String {
    match availability {
        UwpLoopbackAvailability::Supported => Lang(language).tr("uwp_observed_empty").into_owned(),
        _ => uwp_summary(availability, [], language),
    }
}
pub const fn uwp_state_key(exempt: bool) -> &'static str {
    if exempt {
        "uwp_state_exempted"
    } else {
        "uwp_state_isolated"
    }
}

#[cfg(test)]
#[path = "routing_projection_tests.rs"]
mod tests;
