//! The vocabulary the imported Lumen widgets speak.
//!
//! The imported widgets name a sort dimension and a level with plain
//! strings. Those strings are presentation vocabulary, not business state:
//! the shared typed keys own the meaning, and this module is the one adapter
//! between the two. No other place in the surface may translate a widget
//! string on its own, so the widget vocabulary can never grow a second
//! meaning for one control.

use infiltrator_contract::logs::LogLevel;
use infiltrator_domain::connection_view::ConnectionSortKey;
use lurker_core_contract::outbound::preferences::OutboundSortOrder;

/// The shared sort key the widget's sort vocabulary names.
///
/// The widget offers four dimensions and every one of them names one shared
/// key, so the connection rows are ordered by the shared reduction and not
/// by a second surface-local order. A value the widget does not offer names
/// the shared default key, because a control that names no dimension has no
/// order of its own.
pub fn connection_sort_key(value: &str) -> ConnectionSortKey {
    match value {
        "upload" => ConnectionSortKey::UploadDesc,
        "download" => ConnectionSortKey::DownloadDesc,
        "host" => ConnectionSortKey::HostAsc,
        "time" => ConnectionSortKey::LatestDesc,
        _ => ConnectionSortKey::default(),
    }
}

/// The widget's level vocabulary for the shared level filter.
///
/// `None` is "every level". The widget has no button for the `unknown`
/// record level, so an owner filter on it reads as `unknown` and no level
/// button is selected; the surface never claims a level the owner did not
/// confirm.
pub fn log_level_value(level: Option<LogLevel>) -> &'static str {
    match level {
        None => "all",
        Some(LogLevel::Debug) => "debug",
        Some(LogLevel::Info) => "info",
        Some(LogLevel::Warn) => "warning",
        Some(LogLevel::Error) => "error",
        Some(LogLevel::Unknown) => "unknown",
    }
}

/// The shared sort order the widget's proxy sort vocabulary names.
///
/// The imported sort menu offers four dimensions and each of them names one
/// shared sort order, so the proxy rows are ordered by the shared reduction
/// and not by a second surface-local order. The widget's menu positions are
/// its own vocabulary — they are not the shared model's control positions —
/// so the adapter maps a dimension by its name.
///
/// A value the widget does not offer names no order at all, so the caller
/// can refuse the gesture instead of silently applying an order the user
/// never picked.
pub fn proxy_sort_order(value: &str) -> Option<OutboundSortOrder> {
    match value {
        "latency_asc" => Some(OutboundSortOrder::LatencyAscending),
        "latency_desc" => Some(OutboundSortOrder::LatencyDescending),
        "name_asc" => Some(OutboundSortOrder::NameAscending),
        "name_desc" => Some(OutboundSortOrder::NameDescending),
        _ => None,
    }
}

/// The widget's proxy sort vocabulary value for one shared sort order.
///
/// Every shared sort order the proxies page offers has exactly one widget
/// value, so the widget and the shared model can never disagree about which
/// dimension the user picked.
pub fn proxy_sort_value(order: OutboundSortOrder) -> &'static str {
    match order {
        OutboundSortOrder::LatencyAscending => "latency_asc",
        OutboundSortOrder::LatencyDescending => "latency_desc",
        OutboundSortOrder::NameAscending => "name_asc",
        OutboundSortOrder::NameDescending => "name_desc",
    }
}

#[cfg(test)]
mod tests {
    use super::{proxy_sort_order, proxy_sort_value};
    use infiltrator_presentation::proxies_page_model::PROXY_SORT_OPTIONS;

    #[test]
    fn the_widget_vocabulary_names_every_shared_proxy_sort_order() {
        for order in PROXY_SORT_OPTIONS {
            let value = proxy_sort_value(order);
            assert_eq!(
                proxy_sort_order(value),
                Some(order),
                "the widget value {value:?} must name the shared order {order:?}"
            );
        }
    }

    #[test]
    fn a_widget_value_the_menu_does_not_offer_names_no_order() {
        assert_eq!(proxy_sort_order("random"), None);
        assert_eq!(proxy_sort_order(""), None);
    }
}
