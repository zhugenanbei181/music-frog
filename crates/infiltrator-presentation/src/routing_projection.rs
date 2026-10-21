//! User-visible policy follows the actual domain decision, independent of surface vocabulary.
use infiltrator_contract::uwp::UwpLoopbackAvailability;
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use lurker_ui_presentation::copy::TextCatalog;
use lurker_ui_presentation::copy::interpolate;

pub const fn mode_key(mode: AppRoutingMode) -> &'static str {
    match mode {
        AppRoutingMode::ProxyAll => "app_routing_mode_global",
        AppRoutingMode::ProxySelected => "app_routing_mode_selected_proxy",
        AppRoutingMode::BypassSelected => "app_routing_mode_selected_direct",
    }
}

/// The copy key of the routing mode, including the unobserved case.
///
/// A page that has not observed the routing mode has no mode to render, so
/// it renders the unobserved copy instead of falling back to a default
/// mode that the owner never reported.
pub const fn mode_option_key(mode: Option<AppRoutingMode>) -> &'static str {
    match mode {
        Some(mode) => mode_key(mode),
        None => "app_routing_mode_unobserved",
    }
}
pub const fn rule_key(rule: AppRoutingRule) -> &'static str {
    match rule {
        AppRoutingRule::Proxy => "app_routing_rule_proxy",
        AppRoutingRule::Direct => "app_routing_rule_direct",
        AppRoutingRule::Block => "app_routing_rule_block",
    }
}

/// The summary of the routing mode and the configured applications.
///
/// The mode is optional because the owner may not have published one yet;
/// the summary then says so instead of naming a mode the owner never
/// reported.
pub fn routing_summary(
    mode: Option<AppRoutingMode>,
    count: usize,
    catalog: &dyn TextCatalog,
) -> String {
    match mode {
        Some(mode) => interpolate(
            catalog.text("app_routing_observed_summary").as_ref(),
            &[
                ("mode", catalog.text(mode_key(mode)).as_ref()),
                ("count", &count.to_string()),
            ],
        ),
        None => interpolate(
            catalog.text("app_routing_unobserved_summary").as_ref(),
            &[("count", &count.to_string())],
        ),
    }
}
pub fn process_copy(process: &str, catalog: &dyn TextCatalog) -> String {
    interpolate(
        catalog.text("app_routing_process_copy").as_ref(),
        &[("process", process)],
    )
}

pub fn uwp_summary(
    availability: &UwpLoopbackAvailability,
    exemptions: impl IntoIterator<Item = bool>,
    catalog: &dyn TextCatalog,
) -> String {
    match availability {
        UwpLoopbackAvailability::Supported => {
            let (total, exempt) = exemptions
                .into_iter()
                .fold((0_usize, 0_usize), |(total, exempt), enabled| {
                    (total + 1, exempt + usize::from(enabled))
                });
            interpolate(
                catalog.text("uwp_observed_summary").as_ref(),
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
pub fn uwp_empty(availability: &UwpLoopbackAvailability, catalog: &dyn TextCatalog) -> String {
    match availability {
        UwpLoopbackAvailability::Supported => catalog.text("uwp_observed_empty").into_owned(),
        _ => uwp_summary(availability, [], catalog),
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
